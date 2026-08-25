//! Expression evaluator.
//!
//! [`eval_expr`] takes an [`IrExpr`] and a map of previously-computed node
//! outputs, and produces a [`Value`]. It is a pure function — no side effects,
//! no async, no graph traversal. All external data comes from the `outputs` map.

use std::collections::HashMap;
use std::sync::Arc;

use quew_interner::Interner;
use quew_ir::QuewGraphIR;
use quew_ir::graph::{BinaryOp, DataRef, IrExpr, IrLit, NodeId, UnaryOp};

use crate::native::NativeHandler;
use crate::value::{Value, ValueError};

/// An error produced while evaluating an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    /// A [`DataRef`] pointed to a node that has not been executed yet.
    MissingNodeOutput { node: NodeId },
    /// A [`DataRef`] requested a field that does not exist on the value.
    MissingField { node: NodeId, field: String },
    /// A binary or unary operation failed at the value level.
    ValueError(ValueError),
    /// A ternary condition did not evaluate to a boolean.
    NonBooleanCondition,
    /// A native function call failed.
    NativeError { message: String },
    /// An inline graph call failed.
    GraphCallError { message: String },
}

impl From<ValueError> for EvalError {
    fn from(e: ValueError) -> Self {
        EvalError::ValueError(e)
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::MissingNodeOutput { node } => {
                write!(f, "node {node} has not produced an output yet")
            }
            EvalError::MissingField { node, field } => {
                write!(f, "node {node} has no field '{field}'")
            }
            EvalError::ValueError(e) => write!(f, "{e}"),
            EvalError::NonBooleanCondition => {
                write!(f, "ternary condition did not evaluate to a boolean")
            }
            EvalError::NativeError { message } => {
                write!(f, "native function error: {message}")
            }
            EvalError::GraphCallError { message } => {
                write!(f, "graph call error: {message}")
            }
        }
    }
}

/// Resolve a function interned name to its graph reference, if any.
fn resolve_function_graph(
    function: quew_interner::InternedStr,
    func_name: &str,
    ir: &QuewGraphIR,
) -> Option<String> {
    // Direct graph refs (already contain prefix).
    if func_name.starts_with("function:") || func_name.starts_with("extension:") {
        return Some(func_name.to_string());
    }

    // Look up in definitions.functions.
    if let Some(func_def) = ir.definitions.functions.get(&function) {
        return Some(func_def.graph_ref.clone());
    }

    // Look up in definitions.extensions.
    for ext in &ir.definitions.extensions {
        if ext.method_name == function {
            return Some(ext.graph_ref.clone());
        }
    }

    None
}

impl std::error::Error for EvalError {}

/// Evaluate an `IrExpr` into a `Value`.
///
/// This is a method on [`Execution`](crate::execution::Execution) so that
/// expression-level function calls recurse through the *same* executor
/// instance, sharing its safety limits and nested-call-depth tracking.
/// (Previously each graph call from an expression constructed a fresh
/// executor, which made depth limits unenforceable.)
///
/// `outputs` must contain an entry for every `NodeId` referenced by
/// `DataRef`s inside the expression. If a referenced node is missing,
/// [`EvalError::MissingNodeOutput`] is returned.
impl<'a> crate::execution::Execution<'a> {
    pub fn eval_expr(
        &self,
        expr: &IrExpr,
        outputs: &HashMap<NodeId, Value>,
    ) -> Result<Value, EvalError> {
        let ir = self.ir;
        let interner = self.interner;
        let natives = self.natives;
        match expr {
        IrExpr::Lit(lit) => Ok(eval_lit(lit, interner)),
        IrExpr::Ref(data_ref) => eval_data_ref(data_ref, outputs, interner),
        IrExpr::Binary { left, op, right } => {
            let l = self.eval_expr(left, outputs)?;
            let r = self.eval_expr(right, outputs)?;
            eval_binary_op(&l, *op, &r)
        }
        IrExpr::Unary { op, expr } => {
            let v = self.eval_expr(expr, outputs)?;
            eval_unary_op(*op, &v)
        }
        IrExpr::Member { base, field } => {
            let base_val = self.eval_expr(base, outputs)?;
            let field_name = interner.resolve(*field).to_string();
            match base_val {
                Value::Object(map) => {
                    map.get(&field_name)
                        .cloned()
                        .ok_or_else(|| EvalError::MissingField {
                            node: NodeId(0),
                            field: field_name,
                        })
                }
                _ => Err(EvalError::MissingField {
                    node: NodeId(0),
                    field: field_name,
                }),
            }
        }
        IrExpr::Call { function, args } => {
            let func_name = interner.resolve(*function);

            // Look up in native registry first.
            // Try direct lookup by function name (for manually constructed IR),
            // then via definitions.native mapping (for compiled prelude builtins).
            let native_entry = natives.get(func_name).or_else(|| {
                ir.definitions
                    .functions
                    .get(function)
                    .and_then(|def| def.native)
                    .and_then(|native_id| natives.get(interner.resolve(native_id)))
            });

            if let Some(entry) = native_entry {
                let mut arg_values = Vec::with_capacity(args.len());
                for (_name, arg_expr) in args {
                    arg_values.push(self.eval_expr(arg_expr, outputs)?);
                }
                match &entry.handler {
                    NativeHandler::Sync(f) => {
                        f(&arg_values).map_err(|e| EvalError::NativeError { message: e.message })
                    }
                }
            } else {
                // Not a native function — try to recurse into a graph.
                let graph_ref = resolve_function_graph(*function, func_name, ir);

                if let Some(graph_id) = graph_ref {
                    // The compiler binds all parameters as `input.<name>`, so the
                    // runtime always packages arguments into an object keyed by
                    // parameter name, even for single-argument functions.
                    //
                    // IR argument names are not always trustworthy: the lowerer
                    // falls back to positional `arg0, arg1, ...` when the callee
                    // was not yet registered while its own body was lowered
                    // (self-recursion). Re-key positionally from the callee's
                    // real signature whenever we have one.
                    let param_keys: Vec<String> = ir
                        .definitions
                        .functions
                        .get(function)
                        .map(|func| {
                            func.params
                                .keys()
                                .map(|k| interner.resolve(*k).to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    let mut obj = indexmap::IndexMap::new();
                    for (idx, (_name, arg_expr)) in args.iter().enumerate() {
                        let val = self.eval_expr(arg_expr, outputs)?;
                        let key = match param_keys.get(idx) {
                            Some(k) => k.clone(),
                            None => interner.resolve(*_name).to_string(),
                        };
                        obj.insert(key, val);
                    }

                    self.run(&graph_id, Value::Object(obj)).map_err(|e| {
                        EvalError::GraphCallError {
                            message: e.to_string(),
                        }
                    })
                } else {
                    Ok(Value::Null)
                }
            }
        }
        IrExpr::Array(elements) => {
            let mut vals = Vec::with_capacity(elements.len());
            for elem in elements {
                vals.push(self.eval_expr(elem, outputs)?);
            }
            Ok(Value::Array(vals))
        }
        IrExpr::Object(fields) => {
            let mut map = indexmap::IndexMap::new();
            for (name, value_expr) in fields {
                let val = self.eval_expr(value_expr, outputs)?;
                map.insert(interner.resolve(*name).to_string(), val);
            }
            Ok(Value::Object(map))
        }
        IrExpr::Ternary { cond, then, else_ } => {
            let c = self.eval_expr(cond, outputs)?;
            match c {
                Value::Bool(b) => {
                    if b {
                        self.eval_expr(then, outputs)
                    } else {
                        self.eval_expr(else_, outputs)
                    }
                }
                _ => Err(EvalError::NonBooleanCondition),
            }
        }
        IrExpr::Is { value, ty } => {
            let val = self.eval_expr(value, outputs)?;
            let ty_name = interner.resolve(*ty);
            let result = match ty_name {
                "string" => matches!(val, Value::String(_)),
                "number" => matches!(val, Value::Number(_)),
                "float" => matches!(val, Value::Float(_)),
                "bool" => matches!(val, Value::Bool(_)),
                "null" => matches!(val, Value::Null),
                "void" => matches!(val, Value::Null),
                "array" => matches!(val, Value::Array(_)),
                "any" => true,
                _ => value_matches_named_type(&val, *ty, interner, ir),
            };
            Ok(Value::Bool(result))
        }
        }
    }
}

fn value_matches_named_type(
    value: &Value,
    ty: quew_interner::InternedStr,
    interner: &Arc<Interner>,
    ir: &QuewGraphIR,
) -> bool {
    let Some(type_def) = ir.definitions.types.get(&ty) else {
        return matches!(value, Value::Object(_));
    };
    let Value::Object(fields) = value else {
        return false;
    };

    type_def.fields.iter().all(|(field_name, field)| {
        let key = interner.resolve(*field_name);
        match fields.get(key) {
            Some(field_value) => value_matches_ir_type(field_value, &field.ty, interner, ir),
            None => field.optional,
        }
    })
}

fn value_matches_ir_type(
    value: &Value,
    ty: &quew_ir::types::IrType,
    interner: &Arc<Interner>,
    ir: &QuewGraphIR,
) -> bool {
    match ty {
        quew_ir::types::IrType::String | quew_ir::types::IrType::Text => {
            matches!(value, Value::String(_))
        }
        quew_ir::types::IrType::Any => true,
        quew_ir::types::IrType::Number => matches!(value, Value::Number(_)),
        quew_ir::types::IrType::Float => matches!(value, Value::Float(_)),
        quew_ir::types::IrType::Bool => matches!(value, Value::Bool(_)),
        quew_ir::types::IrType::Null | quew_ir::types::IrType::Void => {
            matches!(value, Value::Null)
        }
        quew_ir::types::IrType::Array(_) => matches!(value, Value::Array(_)),
        quew_ir::types::IrType::Object(fields) => {
            let Value::Object(value_fields) = value else {
                return false;
            };
            fields.iter().all(|(field_name, field)| {
                let key = interner.resolve(*field_name);
                match value_fields.get(key) {
                    Some(field_value) => {
                        value_matches_ir_type(field_value, &field.ty, interner, ir)
                    }
                    None => field.optional,
                }
            })
        }
        quew_ir::types::IrType::Named(name) => value_matches_named_type(value, *name, interner, ir),
        quew_ir::types::IrType::Union(types) => types
            .iter()
            .any(|member| value_matches_ir_type(value, member, interner, ir)),
        quew_ir::types::IrType::GenericInstance { name, .. } => {
            value_matches_named_type(value, *name, interner, ir)
        }
        quew_ir::types::IrType::GenericParam(_) | quew_ir::types::IrType::AgentOutput(_) => true,
    }
}

fn eval_lit(lit: &IrLit, interner: &Arc<Interner>) -> Value {
    match lit {
        IrLit::String(s) => Value::String(interner.resolve(*s).to_string()),
        IrLit::Int(n) => Value::Number(*n),
        IrLit::Float(f) => Value::Float(*f),
        IrLit::Bool(b) => Value::Bool(*b),
        IrLit::Null => Value::Null,
    }
}

fn eval_data_ref(
    data_ref: &DataRef,
    outputs: &HashMap<NodeId, Value>,
    interner: &Arc<Interner>,
) -> Result<Value, EvalError> {
    let base = outputs
        .get(&data_ref.node)
        .cloned()
        .ok_or(EvalError::MissingNodeOutput {
            node: data_ref.node,
        })?;

    match &data_ref.slot {
        None => Ok(base),
        Some(slot) => {
            let field_name = interner.resolve(*slot).to_string();
            match base {
                Value::Object(map) => {
                    map.get(&field_name)
                        .cloned()
                        .ok_or_else(|| EvalError::MissingField {
                            node: data_ref.node,
                            field: field_name,
                        })
                }
                _ => Err(EvalError::MissingField {
                    node: data_ref.node,
                    field: field_name,
                }),
            }
        }
    }
}

fn eval_binary_op(left: &Value, op: BinaryOp, right: &Value) -> Result<Value, EvalError> {
    match op {
        BinaryOp::Add => left.add(right).map_err(EvalError::from),
        BinaryOp::Sub => left.sub(right).map_err(EvalError::from),
        BinaryOp::Mul => left.mul(right).map_err(EvalError::from),
        BinaryOp::Div => left.div(right).map_err(EvalError::from),
        BinaryOp::Rem => left.rem(right).map_err(EvalError::from),
        BinaryOp::Eq => left.eq_val(right).map_err(EvalError::from),
        BinaryOp::NotEq => left.not_eq_val(right).map_err(EvalError::from),
        BinaryOp::Lt => left.lt(right).map_err(EvalError::from),
        BinaryOp::Lte => left.lte(right).map_err(EvalError::from),
        BinaryOp::Gt => left.gt(right).map_err(EvalError::from),
        BinaryOp::Gte => left.gte(right).map_err(EvalError::from),
        BinaryOp::And => left.and(right).map_err(EvalError::from),
        BinaryOp::Or => left.or(right).map_err(EvalError::from),
    }
}

fn eval_unary_op(op: UnaryOp, value: &Value) -> Result<Value, EvalError> {
    match op {
        UnaryOp::Not => value.not().map_err(EvalError::from),
        UnaryOp::Neg => value.neg().map_err(EvalError::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::NativeRegistry;

    /// Test shim preserving the old free-function call shape; evaluation now
    /// lives on `Execution` so graph calls share depth tracking.
    fn eval_expr(
        expr: &IrExpr,
        outputs: &HashMap<NodeId, Value>,
        interner: &Arc<Interner>,
        natives: &NativeRegistry,
        ir: &QuewGraphIR,
    ) -> Result<Value, EvalError> {
        crate::execution::Execution::new(ir, interner, natives).eval_expr(expr, outputs)
    }

    use quew_ir::graph::{IrLit, NodeId};

    fn lit_int(n: i64) -> IrExpr {
        IrExpr::Lit(IrLit::Int(n))
    }

    fn lit_bool(b: bool) -> IrExpr {
        IrExpr::Lit(IrLit::Bool(b))
    }

    fn interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }

    fn empty_ir(interner: &Arc<Interner>) -> QuewGraphIR {
        QuewGraphIR {
            program: quew_ir::ProgramMeta {
                name: interner.intern(""),
                entry_agent: interner.intern(""),
            },
            definitions: quew_ir::Definitions::default(),
            graphs: indexmap::IndexMap::new(),
        }
    }

    #[test]
    fn eval_literal() {
        let outputs = HashMap::new();
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&lit_int(42), &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Number(42)
        );
    }

    #[test]
    fn eval_data_ref_scalar() {
        let mut outputs = HashMap::new();
        outputs.insert(NodeId(5), Value::Number(99));

        let expr = IrExpr::Ref(DataRef::scalar(NodeId(5)));
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Number(99)
        );
    }

    #[test]
    fn eval_data_ref_missing() {
        let outputs = HashMap::new();
        let expr = IrExpr::Ref(DataRef::scalar(NodeId(99)));
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert!(matches!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir),
            Err(EvalError::MissingNodeOutput { node: NodeId(99) })
        ));
    }

    #[test]
    fn eval_binary_add() {
        let outputs = HashMap::new();
        let expr = IrExpr::Binary {
            left: Box::new(lit_int(3)),
            op: BinaryOp::Add,
            right: Box::new(lit_int(4)),
        };
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Number(7)
        );
    }

    #[test]
    fn eval_unary_not() {
        let outputs = HashMap::new();
        let expr = IrExpr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(lit_bool(false)),
        };
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eval_ternary_true_branch() {
        let outputs = HashMap::new();
        let expr = IrExpr::Ternary {
            cond: Box::new(lit_bool(true)),
            then: Box::new(lit_int(1)),
            else_: Box::new(lit_int(2)),
        };
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Number(1)
        );
    }

    #[test]
    fn eval_ternary_false_branch() {
        let outputs = HashMap::new();
        let expr = IrExpr::Ternary {
            cond: Box::new(lit_bool(false)),
            then: Box::new(lit_int(1)),
            else_: Box::new(lit_int(2)),
        };
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Number(2)
        );
    }

    #[test]
    fn eval_ternary_non_bool_condition() {
        let outputs = HashMap::new();
        let expr = IrExpr::Ternary {
            cond: Box::new(lit_int(1)),
            then: Box::new(lit_int(1)),
            else_: Box::new(lit_int(2)),
        };
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert!(matches!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir),
            Err(EvalError::NonBooleanCondition)
        ));
    }

    #[test]
    fn eval_array() {
        let outputs = HashMap::new();
        let expr = IrExpr::Array(vec![lit_int(1), lit_int(2), lit_int(3)]);
        let natives = crate::native::NativeRegistry::new();
        let ir = empty_ir(&interner());
        assert_eq!(
            eval_expr(&expr, &outputs, &interner(), &natives, &ir).unwrap(),
            Value::Array(vec![Value::Number(1), Value::Number(2), Value::Number(3)])
        );
    }
}
