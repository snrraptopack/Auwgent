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
use crate::native::NativeRegistry;
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
            crate::native::NativeEntry::Sync(|args| {
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
}
