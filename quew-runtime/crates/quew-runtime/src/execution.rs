//! Deterministic graph executor.
//!
//! [`Execution`] walks an [`AgentGraph`] and evaluates each node, producing a
//! [`Value`] for the graph's [`Output`] node. It handles the deterministic
//! node kinds: [`Input`], [`Context`], [`Output`], [`LetBind`], [`Branch`],
//! and [`FuncCall`] (including recursive calls to user functions and extension
//! methods).
//!
//! Effectful nodes ([`HostToolCall`], [`Reply`], [`AgentCall`]) are **not**
//! supported in this plan — they will be added in Plans 17–19.
//!
//! # Execution model
//!
//! The executor uses a simple forward walk through the graph's nodes. Nodes
//! are stored in an [`IndexMap`] in insertion order, which is topologically
//! valid because the lowerer emits them in dependency order.
//!
//! For [`Branch`] nodes, the executor evaluates the condition and then **skips**
//! all nodes in the untaken branch. It does this by tracking a set of
//! "unreachable" node IDs.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use quew_interner::Interner;
use quew_ir::graph::{AgentGraph, NodeId, NodeKind};
use quew_ir::QuewGraphIR;

use crate::eval::{eval_expr, EvalError};
use crate::native::{NativeHandler, NativeRegistry};
use crate::value::Value;

/// An error produced during graph execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// The requested graph does not exist in the IR.
    GraphNotFound { graph_id: String },
    /// A node referenced an output that was never produced.
    MissingOutput { node: NodeId },
    /// Expression evaluation failed.
    EvalError { node: NodeId, source: EvalError },
    /// A Branch node was encountered but its condition did not evaluate to bool.
    InvalidBranchCondition { node: NodeId },
    /// The Output node could not be resolved.
    MissingReturnValue { graph_id: String },
    /// An unsupported node kind was encountered (HostToolCall, Reply, AgentCall)
    UnsupportedNode { node: NodeId, kind: String },
    /// A function call referenced a graph that does not exist.
    MissingFunctionGraph { function: String },
    /// A native builtin function returned an error.
    NativeError { message: String },
    /// A node received a value of the wrong type.
    TypeMismatch {
        expected: String,
        found: String,
        node: NodeId,
    },
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::GraphNotFound { graph_id } => {
                write!(f, "graph '{graph_id}' not found in IR")
            }
            ExecutionError::MissingOutput { node } => {
                write!(f, "node {node} referenced an output that does not exist")
            }
            ExecutionError::EvalError { node, source } => {
                write!(f, "expression evaluation failed at node {node}: {source}")
            }
            ExecutionError::InvalidBranchCondition { node } => {
                write!(f, "branch node {node} condition is not a boolean")
            }
            ExecutionError::MissingReturnValue { graph_id } => {
                write!(f, "output node of graph '{graph_id}' has no value")
            }
            ExecutionError::UnsupportedNode { node, kind } => {
                write!(f, "node {node} has unsupported kind '{kind}'")
            }
            ExecutionError::MissingFunctionGraph { function } => {
                write!(f, "function '{function}' has no compiled graph")
            }
            ExecutionError::NativeError { message } => {
                write!(f, "native function error: {message}")
            }
            ExecutionError::TypeMismatch { expected, found, node } => {
                write!(
                    f,
                    "type mismatch at node {node}: expected {expected}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Executes a deterministic (effect-free) agent or function graph.
pub struct Execution<'a> {
    /// The compiled program. Shared across all executions.
    pub ir: &'a QuewGraphIR,
    /// The interner for resolving string handles.
    pub interner: &'a Arc<Interner>,
    /// Registry of native functions (`@@rust` builtins).
    pub natives: &'a NativeRegistry,
}

impl<'a> Execution<'a> {
    /// Create a new execution context bound to a compiled IR.
    pub fn new(
        ir: &'a QuewGraphIR,
        interner: &'a Arc<Interner>,
        natives: &'a NativeRegistry,
    ) -> Self {
        Self { ir, interner, natives }
    }

    /// Run a single graph from its entry node to its output node.
    ///
    /// `graph_id` is the key in `ir.graphs` (e.g. `"agent:Main"` or
    /// `"function:double"`). `input` is the value fed into the graph's
    /// [`Input`] node.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::GraphNotFound`] if the graph ID does not
    /// exist. Returns [`ExecutionError::UnsupportedNode`] if the graph
    /// contains effectful nodes that this executor cannot handle.
    pub fn run(&self, graph_id: &str, input: Value) -> Result<Value, ExecutionError> {
        let graph = self
            .ir
            .graphs
            .get(graph_id)
            .ok_or_else(|| ExecutionError::GraphNotFound {
                graph_id: graph_id.to_string(),
            })?;

        let mut outputs: HashMap<NodeId, Value> = HashMap::new();
        let mut unreachable: HashSet<NodeId> = HashSet::new();

        // Seed the Input node.
        outputs.insert(graph.entry_node, input);

        // Walk nodes in insertion order (topologically valid).
        for (node_id, node) in &graph.nodes {
            // Skip unreachable nodes (branches not taken).
            if unreachable.contains(node_id) {
                continue;
            }

            // Skip nodes already seeded (Input, and potentially resumed nodes).
            if outputs.contains_key(node_id) {
                continue;
            }

            match &node.kind {
                NodeKind::Input { .. } => {
                    // Already seeded above.
                }
                NodeKind::Context { .. } => {
                    // Context injection is not supported in this plan.
                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::Output { value } => {
                    let output_value = self
                        .resolve_data_ref(value, &outputs)
                        .map_err(|_| ExecutionError::MissingOutput { node: value.node })?;
                    outputs.insert(*node_id, output_value);
                }
                NodeKind::LetBind { name: _, value } => {
                    let result = eval_expr(value, &outputs, self.interner, self.natives, self.ir).map_err(|e| {
                        ExecutionError::EvalError {
                            node: *node_id,
                            source: e,
                        }
                    })?;
                    outputs.insert(*node_id, result);
                }
                NodeKind::Branch {
                    condition,
                    then_node,
                    else_node,
                } => {
                    let cond_value = self
                        .resolve_data_ref(condition, &outputs)
                        .map_err(|_| ExecutionError::MissingOutput {
                            node: condition.node,
                        })?;

                    let taken = match cond_value {
                        Value::Bool(b) => {
                            if b {
                                *then_node
                            } else {
                                else_node.unwrap_or(*node_id)
                            }
                        }
                        _ => {
                            return Err(ExecutionError::InvalidBranchCondition {
                                node: *node_id,
                            })
                        }
                    };

                    // Mark all nodes in the untaken branch as unreachable.
                    // We do this by computing which branch was NOT taken and
                    // marking its entry plus all transitively reachable nodes.
                    let not_taken = if taken == *then_node {
                        *else_node
                    } else {
                        Some(*then_node)
                    };

                    if let Some(not_taken_id) = not_taken {
                        self.mark_unreachable(graph, not_taken_id, &mut unreachable);
                    }

                    // The branch node itself produces no value.
                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::FuncCall { function, args } => {
                    // Check if this function is a native builtin.
                    let is_native = self
                        .ir
                        .definitions
                        .functions
                        .get(function)
                        .and_then(|def| def.native)
                        .and_then(|native_id| self.natives.get(self.interner.resolve(native_id)));

                    if let Some(entry) = is_native {
                        let mut arg_values = Vec::with_capacity(args.len());
                        for (_slot, data_ref) in args {
                            let val = self
                                .resolve_data_ref(data_ref, &outputs)
                                .map_err(|_| ExecutionError::MissingOutput {
                                    node: data_ref.node,
                                })?;
                            arg_values.push(val);
                        }
                        let result = match &entry.handler {
                            NativeHandler::Sync(f) => {
                                f(&arg_values).map_err(|e| ExecutionError::NativeError {
                                    message: e.message,
                                })?
                            }
                        };
                        outputs.insert(*node_id, result);
                        continue;
                    }

                    let graph_ref = self.resolve_function_graph(*function);

                    // The compiler binds all parameters as `input.<name>`, so the
                    // runtime always packages arguments into an object keyed by
                    // parameter name, even for single-argument functions.
                    let mut obj = indexmap::IndexMap::new();
                    for (slot, data_ref) in args {
                        let val = self
                            .resolve_data_ref(data_ref, &outputs)
                            .map_err(|_| ExecutionError::MissingOutput {
                                node: data_ref.node,
                            })?;
                        let key = self.interner.resolve(*slot).to_string();
                        obj.insert(key, val);
                    }

                    let result = self.run(&graph_ref, Value::Object(obj))?;
                    outputs.insert(*node_id, result);
                }
                NodeKind::Loop {
                    iterable,
                    body_graph,
                    value_name,
                    index_name,
                    captured,
                } => {
                    let array = self
                        .resolve_data_ref(iterable, &outputs)
                        .map_err(|_| ExecutionError::MissingOutput {
                            node: iterable.node,
                        })?;
                    let array = array.as_array().ok_or_else(|| ExecutionError::TypeMismatch {
                        expected: "array".into(),
                        found: array.type_name().into(),
                        node: *node_id,
                    })?;

                    for (idx, item) in array.iter().enumerate() {
                        let mut obj = indexmap::IndexMap::new();
                        obj.insert(
                            self.interner.resolve(*value_name).to_string(),
                            item.clone(),
                        );
                        if let Some(idx_name) = index_name {
                            obj.insert(
                                self.interner.resolve(*idx_name).to_string(),
                                Value::Number(idx as i64),
                            );
                        }
                        for (name, data_ref) in captured {
                            let val = self.resolve_data_ref(data_ref, &outputs).map_err(|_| {
                                ExecutionError::MissingOutput {
                                    node: data_ref.node,
                                }
                            })?;
                            obj.insert(self.interner.resolve(*name).to_string(), val);
                        }

                        let _ = self.run(body_graph, Value::Object(obj))?;
                    }

                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::WhileLoop {
                    body_graph,
                    captured,
                } => {
                    loop {
                        let mut obj = indexmap::IndexMap::new();
                        for (name, data_ref) in captured {
                            let val = self
                                .resolve_data_ref(data_ref, &outputs)
                                .map_err(|_| ExecutionError::MissingOutput {
                                    node: data_ref.node,
                                })?;
                            obj.insert(self.interner.resolve(*name).to_string(), val);
                        }

                        let result = self.run(body_graph, Value::Object(obj))?;
                        let result_map = match &result {
                            Value::Object(map) => map,
                            _ => break,
                        };

                        let cond = result_map
                            .get("__cond")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // Update captured variables in parent outputs so the next
                        // iteration sees mutated state.
                        for (name, data_ref) in captured {
                            let key = self.interner.resolve(*name).to_string();
                            if let Some(new_val) = result_map.get(&key) {
                                outputs.insert(data_ref.node, new_val.clone());
                            }
                        }

                        if !cond {
                            break;
                        }
                    }

                    outputs.insert(*node_id, Value::Null);
                }
                other => {
                    return Err(ExecutionError::UnsupportedNode {
                        node: *node_id,
                        kind: format!("{other:?}"),
                    });
                }
            }
        }

        // Return the output node's value.
        outputs
            .get(&graph.return_node)
            .cloned()
            .ok_or_else(|| ExecutionError::MissingReturnValue {
                graph_id: graph_id.to_string(),
            })
    }

    /// Resolve a `DataRef` to a `Value` by looking up the source node's output.
    fn resolve_data_ref(
        &self,
        data_ref: &quew_ir::graph::DataRef,
        outputs: &HashMap<NodeId, Value>,
    ) -> Result<Value, ()> {
        let base = outputs.get(&data_ref.node).cloned().ok_or(())?;

        match &data_ref.slot {
            None => Ok(base),
            Some(slot) => {
                let field_name = self.interner.resolve(*slot).to_string();
                match base {
                    Value::Object(map) => map.get(&field_name).cloned().ok_or(()),
                    _ => Err(()),
                }
            }
        }
    }

    /// Resolve a function name to its graph reference.
    ///
    /// For extension methods and regular functions, the `function` field in
    /// `FuncCall` contains the interned name. We look up the corresponding
    /// `graph_ref` in `definitions.functions` or `definitions.extensions`.
    fn resolve_function_graph(&self, function: quew_interner::InternedStr) -> String {
        let name = self.interner.resolve(function);

        // Direct graph refs (already contain prefix).
        if name.starts_with("function:") || name.starts_with("extension:") {
            return name.to_string();
        }

        // Look up in definitions.functions.
        if let Some(func_def) = self.ir.definitions.functions.get(&function) {
            return func_def.graph_ref.clone();
        }

        // Fallback: construct the graph ref from the name.
        format!("function:{name}")
    }

    /// Mark a node and all nodes transitively reachable from it as unreachable.
    ///
    /// This is used after a Branch to skip the untaken arm. We only follow
    /// edges where this node is the *source* (i.e. edges from this node to
    /// downstream nodes).
    fn mark_unreachable(
        &self,
        graph: &AgentGraph,
        start: NodeId,
        unreachable: &mut HashSet<NodeId>,
    ) {
        let mut queue = vec![start];
        while let Some(current) = queue.pop() {
            if !unreachable.insert(current) {
                continue; // already visited
            }
            // Find all edges originating from this node.
            for edge in &graph.edges {
                if edge.from == current {
                    queue.push(edge.to);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quew_errors::Severity;
    use quew_interner::Interner;
    use quew_ir::lower::lower;
    use quew_source::SourceMap;

    fn compile_source(source: &str) -> (Arc<Interner>, QuewGraphIR) {
        let interner = Arc::new(Interner::new());
        let source_map = SourceMap::new(Arc::clone(&interner));
        let source_id = source_map.add("test.quew", source.to_string());
        let lex = quew_lexer::lex(source, source_id, &interner);
        assert!(lex.errors.is_empty(), "lex errors: {:?}", lex.errors);
        let parse = quew_parser::parse(&lex, source, &interner);
        assert!(parse.errors.is_empty(), "parse errors: {:?}", parse.errors);
        let check = quew_checker::check(&parse.module, &interner);
        assert!(
            !check
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "checker errors: {:?}",
            check.diagnostics
        );
        let ir = lower(&parse.module, &check, &interner);
        (interner, ir)
    }

    fn compile_source_with_prelude(source: &str) -> (Arc<Interner>, QuewGraphIR) {
        let interner = Arc::new(Interner::new());
        let source_map = SourceMap::new(Arc::clone(&interner));
        let source_id = source_map.add("test.quew", source.to_string());
        let lex = quew_lexer::lex(source, source_id, &interner);
        assert!(lex.errors.is_empty(), "lex errors: {:?}", lex.errors);
        let parse = quew_parser::parse(&lex, source, &interner);
        assert!(parse.errors.is_empty(), "parse errors: {:?}", parse.errors);
        let prelude = quew_checker::module_with_prelude(&parse.module, &interner);
        let check = quew_checker::check(&prelude.module, &interner);
        assert!(
            !check
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "checker errors: {:?}",
            check.diagnostics
        );
        let ir = lower(&prelude.module, &check, &interner);
        (interner, ir)
    }

    #[test]
    fn execute_literal_return_function() {
        let (interner, ir) = compile_source(
            r#"
function answer(): number { return 42 }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:answer", Value::Null).unwrap();
        assert_eq!(result, Value::Number(42));
    }

    #[test]
    fn execute_identity_function() {
        let (interner, ir) = compile_source(
            r#"
function identity(x: number): number { return x }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("x".to_string(), Value::Number(7));
        let result = exec.run("function:identity", Value::Object(input)).unwrap();
        assert_eq!(result, Value::Number(7));
    }

    #[test]
    fn execute_arithmetic_function() {
        let (interner, ir) = compile_source(
            r#"
function double(x: number): number { return x + x }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("x".to_string(), Value::Number(5));
        let result = exec.run("function:double", Value::Object(input)).unwrap();
        assert_eq!(result, Value::Number(10));
    }

    #[test]
    fn execute_branch_routing() {
        // Test branch execution with a hand-built graph so we can verify
        // the executor correctly routes to the taken arm and skips the
        // untaken arm, without relying on the compiler's branch lowering.
        let interner = Arc::new(Interner::new());

        let mut nodes = indexmap::IndexMap::new();
        let mut edges = Vec::new();

        // n0: Input (bool)
        nodes.insert(
            NodeId(0),
            quew_ir::graph::IrNode {
                id: NodeId(0),
                kind: NodeKind::Input {
                    input_ty: quew_ir::types::IrType::Bool,
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Never,
            },
        );

        // n1: Branch on input
        nodes.insert(
            NodeId(1),
            quew_ir::graph::IrNode {
                id: NodeId(1),
                kind: NodeKind::Branch {
                    condition: quew_ir::graph::DataRef::scalar(NodeId(0)),
                    then_node: NodeId(2),
                    else_node: Some(NodeId(3)),
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Optional,
            },
        );

        // n2: Then arm — bind 42
        nodes.insert(
            NodeId(2),
            quew_ir::graph::IrNode {
                id: NodeId(2),
                kind: NodeKind::LetBind {
                    name: interner.intern("then_val"),
                    value: quew_ir::graph::IrExpr::Lit(quew_ir::graph::IrLit::Int(42)),
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Optional,
            },
        );

        // n3: Else arm — bind 0
        nodes.insert(
            NodeId(3),
            quew_ir::graph::IrNode {
                id: NodeId(3),
                kind: NodeKind::LetBind {
                    name: interner.intern("else_val"),
                    value: quew_ir::graph::IrExpr::Lit(quew_ir::graph::IrLit::Int(0)),
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Optional,
            },
        );

        // n4: Output — returns the then_val (this is a simplification;
        // in a real graph the output would be connected to whichever arm ran)
        nodes.insert(
            NodeId(4),
            quew_ir::graph::IrNode {
                id: NodeId(4),
                kind: NodeKind::Output {
                    value: quew_ir::graph::DataRef::scalar(NodeId(2)),
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Never,
            },
        );

        edges.push(quew_ir::graph::Edge {
            from: NodeId(0),
            to: NodeId(1),
            slot: interner.intern("condition"),
        });

        let graph = quew_ir::graph::AgentGraph {
            graph_id: "agent:BranchTest".to_string(),
            entry_node: NodeId(0),
            return_node: NodeId(4),
            nodes,
            edges,
        };

        let mut graphs = indexmap::IndexMap::new();
        graphs.insert("agent:BranchTest".to_string(), graph);

        let ir = QuewGraphIR {
            program: quew_ir::ProgramMeta {
                name: interner.intern("BranchTest"),
                entry_agent: interner.intern("BranchTest"),
            },
            definitions: quew_ir::Definitions::default(),
            graphs,
        };

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);

        // When input is true, the then arm (n2) should execute.
        let result = exec.run("agent:BranchTest", Value::Bool(true)).unwrap();
        // Output is wired to n2, which should have executed.
        assert_eq!(result, Value::Number(42));

        // When input is false, the else arm (n3) should execute, but the
        // output is still wired to n2 which won't have run. This is expected
        // for this simplified graph — in a real program the output would be
        // wired to a phi-like merge node.
    }

    #[test]
    fn execute_function_calling_another_function() {
        let (interner, ir) = compile_source(
            r#"
function add(a: number, b: number): number { return a + b }
function add_three(x: number): number { return add(x, 3) }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("x".to_string(), Value::Number(4));
        let result = exec.run("function:add_three", Value::Object(input)).unwrap();
        assert_eq!(result, Value::Number(7));
    }

    #[test]
    fn execute_extension_method_call() {
        let (interner, ir) = compile_source(
            r#"
extend string {
    function withPrefix(prefix: string): string { return prefix + self }
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("self".to_string(), Value::String("world".into()));
        input.insert("prefix".to_string(), Value::String("Hello, ".into()));
        let result = exec.run("extension:string:withPrefix", Value::Object(input)).unwrap();
        assert_eq!(result, Value::String("Hello, world".into()));
    }

    #[test]
    fn execute_native_builtin_dispatch() {
        let interner = Arc::new(Interner::new());

        // Build a minimal graph that calls a native function inline.
        let mut nodes = indexmap::IndexMap::new();

        nodes.insert(
            NodeId(0),
            quew_ir::graph::IrNode {
                id: NodeId(0),
                kind: NodeKind::Input {
                    input_ty: quew_ir::types::IrType::String,
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Never,
            },
        );

        // n1: LetBind that calls the native len function inline.
        let func_name = interner.intern("std.string.len");
        nodes.insert(
            NodeId(1),
            quew_ir::graph::IrNode {
                id: NodeId(1),
                kind: NodeKind::LetBind {
                    name: interner.intern("len"),
                    value: quew_ir::graph::IrExpr::Call {
                        function: func_name,
                        args: {
                            let mut m = indexmap::IndexMap::new();
                            m.insert(
                                interner.intern("self"),
                                quew_ir::graph::IrExpr::Ref(quew_ir::graph::DataRef::scalar(NodeId(0))),
                            );
                            m
                        },
                    },
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Optional,
            },
        );

        nodes.insert(
            NodeId(2),
            quew_ir::graph::IrNode {
                id: NodeId(2),
                kind: NodeKind::Output {
                    value: quew_ir::graph::DataRef::scalar(NodeId(1)),
                },
                checkpoint: quew_ir::graph::CheckpointPolicy::Never,
            },
        );

        let graph = quew_ir::graph::AgentGraph {
            graph_id: "function:len_test".to_string(),
            entry_node: NodeId(0),
            return_node: NodeId(2),
            nodes,
            edges: Vec::new(),
        };

        let mut graphs = indexmap::IndexMap::new();
        graphs.insert("function:len_test".to_string(), graph);

        let ir = QuewGraphIR {
            program: quew_ir::ProgramMeta {
                name: interner.intern("LenTest"),
                entry_agent: interner.intern("LenTest"),
            },
            definitions: quew_ir::Definitions::default(),
            graphs,
        };

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.string.len",
            crate::native::NativeHandler::Sync(|args| {
                let s = args[0].as_str().ok_or("len: expected string")?;
                Ok(Value::Number(s.len() as i64))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:len_test", Value::String("hello".into())).unwrap();
        assert_eq!(result, Value::Number(5));
    }

    #[test]
    fn graph_not_found() {
        let interner = Arc::new(Interner::new());
        let ir = QuewGraphIR {
            program: quew_ir::ProgramMeta {
                name: interner.intern("Test"),
                entry_agent: interner.intern("Test"),
            },
            definitions: quew_ir::Definitions::default(),
            graphs: indexmap::IndexMap::new(),
        };

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let err = exec.run("function:missing", Value::Null).unwrap_err();
        assert!(matches!(err, ExecutionError::GraphNotFound { .. }));
    }

    #[test]
    fn execute_string_interpolation() {
        let (interner, ir) = compile_source(
            r#"
function greet(name: string): string {
    return "hello {name}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("name".to_string(), Value::String("world".into()));
        let result = exec.run("function:greet", Value::Object(input)).unwrap();
        assert_eq!(result, Value::String("hello world".into()));
    }

    #[test]
    fn execute_string_interpolation_multiple_segments() {
        let (interner, ir) = compile_source(
            r#"
function format(a: string, b: string): string {
    return "{a} and {b}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("a".to_string(), Value::String("hello".into()));
        input.insert("b".to_string(), Value::String("world".into()));
        let result = exec.run("function:format", Value::Object(input)).unwrap();
        assert_eq!(result, Value::String("hello and world".into()));
    }

    #[test]
    fn execute_string_interpolation_with_escaped_braces() {
        // {{ only escapes when the string also contains interpolation.
        let (interner, ir) = compile_source(
            r#"
function braces(x: string): string {
    return "{{literal}} {x}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let mut input = indexmap::IndexMap::new();
        input.insert("x".to_string(), Value::String("value".into()));
        let result = exec.run("function:braces", Value::Object(input)).unwrap();
        assert_eq!(result, Value::String("{literal} value".into()));
    }

    // ── Native builtin dispatch from compiled code ────────────────────────────

    #[test]
    fn execute_string_len_native_from_compiled_code() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    return len("hello")
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.string.len",
            crate::native::NativeHandler::Sync(|args| {
                let s = args[0].as_str().ok_or("expected string")?;
                Ok(Value::Number(s.len() as i64))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(5));
    }

    #[test]
    fn execute_array_len_native_from_compiled_code() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let arr = [1, 2, 3]
    return array_len(arr)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.array.len",
            crate::native::NativeHandler::Sync(|args| {
                let arr = args[0].as_array().ok_or("expected array")?;
                Ok(Value::Number(arr.len() as i64))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(3));
    }

    #[test]
    fn execute_array_get_native_from_compiled_code() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let arr = [10, 20, 30]
    return array_get(arr, 1)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.array.get",
            crate::native::NativeHandler::Sync(|args| {
                let arr = args[0].as_array().ok_or("expected array")?;
                let idx = args[1].as_number().ok_or("expected number")?;
                Ok(arr.get(idx as usize).cloned().unwrap_or(Value::Null))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(20));
    }

    #[test]
    fn execute_array_push_native_from_compiled_code() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number[] {
    let arr = [1, 2]
    return array_push(arr, 3)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.array.push",
            crate::native::NativeHandler::Sync(|args| {
                let mut arr = args[0].as_array().map(|a| a.to_vec()).unwrap_or_default();
                arr.push(args[1].clone());
                Ok(Value::Array(arr))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(
            result,
            Value::Array(vec![Value::Number(1), Value::Number(2), Value::Number(3)])
        );
    }

    #[test]
    fn execute_array_pop_native_from_compiled_code() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let arr = [1, 2, 3]
    return array_pop(arr)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let mut natives = crate::native::NativeRegistry::new();
        natives.register(
            "std.array.pop",
            crate::native::NativeHandler::Sync(|args| {
                let arr = args[0].as_array().ok_or("expected array")?;
                Ok(arr.last().cloned().unwrap_or(Value::Null))
            }),
        );
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(3));
    }

    // ── For loop execution ────────────────────────────────────────────────────

    #[test]
    fn execute_for_loop_over_literal_array() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    for x in [1, 2, 3] {
        let y = x
    }
    return 42
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(42));
    }

    #[test]
    fn execute_for_loop_with_index() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    for item, idx in [10, 20, 30] {
        let y = item
    }
    return 99
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(99));
    }

    // ── Mutable assignment ────────────────────────────────────────────────────

    #[test]
    fn execute_mutable_assignment() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let count = 0
    count = 5
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(5));
    }

    #[test]
    fn execute_mutable_assignment_with_expression() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let count = 10
    count = count + 1
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(11));
    }

    #[test]
    fn execute_assignment_inside_branch_then_taken() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let x = 0
    if true {
        x = 42
    }
    return x
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(42));
    }

    #[test]
    fn execute_assignment_inside_branch_else_taken() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let x = 0
    if false {
        x = 99
    } else {
        x = 77
    }
    return x
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(77));
    }

    // ── Object literals ───────────────────────────────────────────────────────

    #[test]
    fn execute_typed_object_literal() {
        let (interner, ir) = compile_source(
            r#"
type Person = {
    name: string
    age: number
}
function test(): Person {
    let obj: Person = { name: "Alice", age: 30 }
    return obj
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        let mut expected = indexmap::IndexMap::new();
        expected.insert("name".to_string(), Value::String("Alice".into()));
        expected.insert("age".to_string(), Value::Number(30));
        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn execute_object_literal_field_access() {
        let (interner, ir) = compile_source(
            r#"
type Person = {
    name: string
    age: number
}
function test(): string {
    let obj: Person = { name: "Bob", age: 25 }
    return obj.name
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::String("Bob".into()));
    }

    // ── While loop execution ──────────────────────────────────────────────────

    #[test]
    fn execute_while_loop_with_mutation() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let count = 0
    while count < 3 {
        count = count + 1
    }
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(3));
    }

    #[test]
    fn execute_while_loop_zero_iterations() {
        let (interner, ir) = compile_source_with_prelude(
            r#"
function test(): number {
    let count = 5
    while count < 3 {
        let count = count + 1
    }
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
        );

        let natives = crate::native::NativeRegistry::new();
        let exec = Execution::new(&ir, &interner, &natives);
        let result = exec.run("function:test", Value::Null).unwrap();
        assert_eq!(result, Value::Number(5));
    }
}
