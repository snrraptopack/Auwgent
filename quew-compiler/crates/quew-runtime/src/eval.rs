//! Expression evaluator.
//!
//! [`eval_expr`] takes an [`IrExpr`] and a map of previously-computed node
//! outputs, and produces a [`Value`]. It is a pure function — no side effects,
//! no async, no graph traversal. All external data comes from the `outputs` map.

use std::collections::HashMap;
use std::sync::Arc;

use quew_interner::Interner;
use quew_ir::graph::{BinaryOp, DataRef, IrExpr, IrLit, NodeId, UnaryOp};

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
        }
    }
}

impl std::error::Error for EvalError {}

/// Evaluate an `IrExpr` into a `Value`.
///
/// `outputs` must contain an entry for every `NodeId` referenced by
/// `DataRef`s inside the expression. If a referenced node is missing,
/// [`EvalError::MissingNodeOutput`] is returned.
///
/// The `interner` is used to resolve `InternedStr` field names into readable
/// strings for object field access and error messages.
pub fn eval_expr(
    expr: &IrExpr,
    outputs: &HashMap<NodeId, Value>,
    interner: &Arc<Interner>,
) -> Result<Value, EvalError> {
    match expr {
        IrExpr::Lit(lit) => Ok(eval_lit(lit, interner)),
        IrExpr::Ref(data_ref) => eval_data_ref(data_ref, outputs, interner),
        IrExpr::Binary { left, op, right } => {
            let l = eval_expr(left, outputs, interner)?;
            let r = eval_expr(right, outputs, interner)?;
            eval_binary_op(&l, *op, &r)
        }
        IrExpr::Unary { op, expr } => {
            let v = eval_expr(expr, outputs, interner)?;
            eval_unary_op(*op, &v)
        }
        IrExpr::Member { base, field } => {
            let base_val = eval_expr(base, outputs, interner)?;
            let field_name = interner.resolve(*field).to_string();
            match base_val {
                Value::Object(map) => map
                    .get(&field_name)
                    .cloned()
                    .ok_or_else(|| EvalError::MissingField {
                        node: NodeId(0),
                        field: field_name,
                    }),
                _ => Err(EvalError::MissingField {
                    node: NodeId(0),
                    field: field_name,
                }),
            }
        }
        IrExpr::Call { function, args } => {
            // For pure inline calls (e.g. inside a larger expression), we
            // evaluate each argument. The actual dispatch happens at the
            // Execution level for graph-level FuncCall nodes.
            //
            // TODO(Plan 16): When we add the native registry, inline calls
            // should dispatch here. For now, we return Null so that graphs
            // without inline function calls execute correctly.
            let _ = function;
            let _ = args;
            Ok(Value::Null)
        }
        IrExpr::Array(elements) => {
            let mut vals = Vec::with_capacity(elements.len());
            for elem in elements {
                vals.push(eval_expr(elem, outputs, interner)?);
            }
            Ok(Value::Array(vals))
        }
        IrExpr::Ternary { cond, then, else_ } => {
            let c = eval_expr(cond, outputs, interner)?;
            match c {
                Value::Bool(b) => {
                    if b {
                        eval_expr(then, outputs, interner)
                    } else {
                        eval_expr(else_, outputs, interner)
                    }
                }
                _ => Err(EvalError::NonBooleanCondition),
            }
        }
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
                Value::Object(map) => map
                    .get(&field_name)
                    .cloned()
                    .ok_or_else(|| EvalError::MissingField {
                        node: data_ref.node,
                        field: field_name,
                    }),
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

    #[test]
    fn eval_literal() {
        let outputs = HashMap::new();
        assert_eq!(
            eval_expr(&lit_int(42), &outputs, &interner()).unwrap(),
            Value::Number(42)
        );
    }

    #[test]
    fn eval_data_ref_scalar() {
        let mut outputs = HashMap::new();
        outputs.insert(NodeId(5), Value::Number(99));

        let expr = IrExpr::Ref(DataRef::scalar(NodeId(5)));
        assert_eq!(eval_expr(&expr, &outputs, &interner()).unwrap(), Value::Number(99));
    }

    #[test]
    fn eval_data_ref_missing() {
        let outputs = HashMap::new();
        let expr = IrExpr::Ref(DataRef::scalar(NodeId(99)));
        assert!(matches!(
            eval_expr(&expr, &outputs, &interner()),
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
        assert_eq!(
            eval_expr(&expr, &outputs, &interner()).unwrap(),
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
        assert_eq!(
            eval_expr(&expr, &outputs, &interner()).unwrap(),
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
        assert_eq!(
            eval_expr(&expr, &outputs, &interner()).unwrap(),
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
        assert_eq!(
            eval_expr(&expr, &outputs, &interner()).unwrap(),
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
        assert!(matches!(
            eval_expr(&expr, &outputs, &interner()),
            Err(EvalError::NonBooleanCondition)
        ));
    }

    #[test]
    fn eval_array() {
        let outputs = HashMap::new();
        let expr = IrExpr::Array(vec![lit_int(1), lit_int(2), lit_int(3)]);
        assert_eq!(
            eval_expr(&expr, &outputs, &interner()).unwrap(),
            Value::Array(vec![Value::Number(1), Value::Number(2), Value::Number(3)])
        );
    }
}
