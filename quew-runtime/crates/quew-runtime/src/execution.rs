//! Deterministic graph executor.
//!
//! [`Execution`] walks an [`AgentGraph`] and evaluates each node, producing a
//! [`Value`] for the graph's [`Output`] node. It handles the deterministic
//! node kinds: [`Input`], [`Context`], [`Output`], [`LetBind`], [`Branch`],
//! and [`FuncCall`] (including recursive calls to user functions and extension
//! methods).
//!
//! Effectful nodes ([`HostToolCall`], [`Reply`], [`AgentCall`]) are **not**
//! supported in this plan â€” they will be added in Plans 17â€“19.
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
use quew_ir::QuewGraphIR;
use quew_ir::graph::{NodeId, NodeKind};

use crate::eval::EvalError;
use crate::native::{NativeHandler, NativeRegistry};
use crate::value::Value;

/// An error produced during graph execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// The requested graph does not exist in the IR.
    GraphNotFound { graph_id: String },
    /// A node referenced an output that was never produced.
    MissingOutput { node: NodeId, detail: String },
    /// A `while` loop exceeded the configured iteration limit.
    LoopLimitExceeded { node: NodeId, limit: u64 },
    /// Nested graph execution exceeded the configured call-depth limit.
    RecursionLimitExceeded { limit: u32 },
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
    /// `break` was encountered inside a loop body.
    Break,
    /// `continue` was encountered inside a loop body.
    Continue,
    /// A `return` node executed â€” internal sentinel; the recorded value
    /// short-circuits the graph result and never escapes `run()`.
    Returned,
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::GraphNotFound { graph_id } => {
                write!(f, "graph '{graph_id}' not found in IR")
            }
            ExecutionError::MissingOutput { node, detail } => {
                write!(
                    f,
                    "node {node} referenced an output that does not exist: {detail}"
                )
            }
            ExecutionError::LoopLimitExceeded { node, limit } => {
                write!(
                    f,
                    "while loop at node {node} exceeded the iteration limit of {limit}; \
                     if this loop is expected to run longer, raise `limits.max_loop_iterations`"
                )
            }
            ExecutionError::RecursionLimitExceeded { limit } => {
                write!(
                    f,
                    "nested execution exceeded the call-depth limit of {limit}; \
                     likely unbounded recursion"
                )
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
            ExecutionError::TypeMismatch {
                expected,
                found,
                node,
            } => {
                write!(
                    f,
                    "type mismatch at node {node}: expected {expected}, found {found}"
                )
            }
            ExecutionError::Break => write!(f, "break"),
            ExecutionError::Continue => write!(f, "continue"),
            ExecutionError::Returned => write!(f, "returned"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Safety limits applied during execution.
///
/// These exist so that runaway programs (infinite `while` loops, unbounded
/// recursion) fail with a clean error instead of hanging the host process or
/// overflowing the stack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionLimits {
    /// Maximum iterations for a single `while` loop. `for` loops are bounded
    /// by the array they iterate and need no cap.
    pub max_loop_iterations: u64,
    /// Maximum nested graph executions (function calls, loop bodies).
    pub max_call_depth: u32,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_loop_iterations: 100_000,
            // Conservative: each nested level consumes real stack frames
            // (run_graph + eval_expr), so the cap must fire well before the
            // host thread's stack (often 1 MB) is exhausted. Agent programs
            // legitimately nest far below this; deeper recursion is a bug.
            max_call_depth: 64,
        }
    }
}

/// Executes a deterministic (effect-free) agent or function graph.
pub struct Execution<'a> {
    /// The compiled program. Shared across all executions.
    pub ir: &'a QuewGraphIR,
    /// The interner for resolving string handles.
    pub interner: &'a Arc<Interner>,
    /// Registry of native functions (`@@rust` builtins).
    pub natives: &'a NativeRegistry,
    /// Safety limits for this execution.
    pub limits: ExecutionLimits,
    /// Current nested-graph depth. Shared across recursive calls through
    /// `&self`, hence the cell.
    depth: std::cell::Cell<u32>,
}

impl<'a> Execution<'a> {
    /// Create a new execution context bound to a compiled IR, with default
    /// [`ExecutionLimits`].
    pub fn new(
        ir: &'a QuewGraphIR,
        interner: &'a Arc<Interner>,
        natives: &'a NativeRegistry,
    ) -> Self {
        Self::with_limits(ir, interner, natives, ExecutionLimits::default())
    }

    /// Create a new execution context with explicit safety limits.
    pub fn with_limits(
        ir: &'a QuewGraphIR,
        interner: &'a Arc<Interner>,
        natives: &'a NativeRegistry,
        limits: ExecutionLimits,
    ) -> Self {
        Self {
            ir,
            interner,
            natives,
            limits,
            depth: std::cell::Cell::new(0),
        }
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
        self.run_graph(graph, input).0
    }

    /// Execute a single graph, returning both the result and the final node
    /// outputs map. The outputs map is used by loop handlers to extract
    /// mutated captured variables even when `break`/`continue` interrupts
    /// execution before the graph's `Output` node.
    ///
    /// Tracks nested-graph depth (function calls, loop bodies) against
    /// [`ExecutionLimits::max_call_depth`] so unbounded recursion fails with
    /// a clean error instead of a stack overflow.
    fn run_graph(
        &self,
        graph: &quew_ir::graph::AgentGraph,
        input: Value,
    ) -> (Result<Value, ExecutionError>, HashMap<NodeId, Value>) {
        let depth = self.depth.get() + 1;
        if depth > self.limits.max_call_depth {
            return (
                Err(ExecutionError::RecursionLimitExceeded {
                    limit: self.limits.max_call_depth,
                }),
                HashMap::new(),
            );
        }
        self.depth.set(depth);
        let result = self.run_graph_inner(graph, input);
        self.depth.set(depth - 1);
        result
    }

    fn run_graph_inner(
        &self,
        graph: &quew_ir::graph::AgentGraph,
        input: Value,
    ) -> (Result<Value, ExecutionError>, HashMap<NodeId, Value>) {
        let mut outputs: HashMap<NodeId, Value> = HashMap::new();
        let mut unreachable: HashSet<NodeId> = HashSet::new();
        let mut control: Option<ExecutionError> = None;
        // Value recorded by a `Return` node, if one executed. Takes priority
        // over the graph's `Output` node.
        let mut early_return: Option<Value> = None;

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

            // After break/continue was triggered, only execute merge nodes
            // (LetBind with Ternary), temporary bindings (name "_"), and the
            // Output node so that the graph can still produce a return value
            // with the latest variable bindings. All other nodes are dead code.
            if control.is_some() {
                match &node.kind {
                    NodeKind::LetBind {
                        value: quew_ir::graph::IrExpr::Ternary { .. },
                        ..
                    } => {
                        // merge node â€” execute below
                    }
                    NodeKind::LetBind { name, .. } if self.interner.resolve(*name) == "_" => {
                        // temporary binding (e.g. return object) â€” execute below
                    }
                    NodeKind::Output { .. } => {
                        // output node â€” execute below
                    }
                    _ => continue,
                }
            }

            match &node.kind {
                NodeKind::Input { .. } => {
                    // Already seeded above.
                }
                NodeKind::Context { .. } => {
                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::Output { value } => {
                    let output_value = match self.resolve_data_ref(value, &outputs) {
                        Ok(v) => v,
                        Err(detail) => {
                            return (
                                Err(ExecutionError::MissingOutput {
                                    node: value.node,
                                    detail,
                                }),
                                outputs,
                            );
                        }
                    };
                    outputs.insert(*node_id, output_value);
                }
                NodeKind::LetBind { name: _, value } => {
                    let result = match self.eval_expr(value, &outputs) {
                            Ok(v) => v,
                            Err(e) => {
                                return (
                                    Err(ExecutionError::EvalError {
                                        node: *node_id,
                                        source: e,
                                    }),
                                    outputs,
                                );
                            }
                        };
                    outputs.insert(*node_id, result);
                }
                NodeKind::Branch {
                    condition,
                    then_node: _,
                    else_node: _,
                    then_span,
                    else_span,
                } => {
                    let cond_value = match self.resolve_data_ref(condition, &outputs) {
                        Ok(v) => v,
                        Err(detail) => {
                            return (
                                Err(ExecutionError::MissingOutput {
                                    node: condition.node,
                                    detail,
                                }),
                                outputs,
                            );
                        }
                    };

                    let cond = match cond_value {
                        Value::Bool(b) => b,
                        _ => {
                            return (
                                Err(ExecutionError::InvalidBranchCondition { node: *node_id }),
                                outputs,
                            );
                        }
                    };

                    // Structurally mark every node in the untaken arm's span
                    // unreachable. Spans are recorded by the lowerer and cover
                    // the full arm, so multi-statement bodies are skipped
                    // correctly regardless of data dependencies between them.
                    let untaken_span = if cond { *else_span } else { *then_span };
                    if let Some((start, end)) = untaken_span {
                        for id in start.0..=end.0 {
                            unreachable.insert(NodeId(id));
                        }
                    }

                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::FuncCall { function, args } => {
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
                            let val = match self.resolve_data_ref(data_ref, &outputs) {
                                Ok(v) => v,
                                Err(detail) => {
                                    return (
                                        Err(ExecutionError::MissingOutput {
                                            node: data_ref.node,
                                            detail,
                                        }),
                                        outputs,
                                    );
                                }
                            };
                            arg_values.push(val);
                        }
                        let result = match &entry.handler {
                            NativeHandler::Sync(f) => match f(&arg_values) {
                                Ok(v) => v,
                                Err(e) => {
                                    return (
                                        Err(ExecutionError::NativeError { message: e.message }),
                                        outputs,
                                    );
                                }
                            },
                        };
                        outputs.insert(*node_id, result);
                        continue;
                    }

                    let graph_ref = self.resolve_function_graph(*function);

                    let mut obj = indexmap::IndexMap::new();
                    for (slot, data_ref) in args {
                        let val = match self.resolve_data_ref(data_ref, &outputs) {
                            Ok(v) => v,
                            Err(detail) => {
                                return (
                                    Err(ExecutionError::MissingOutput {
                                        node: data_ref.node,
                                        detail,
                                    }),
                                    outputs,
                                );
                            }
                        };
                        let key = self.interner.resolve(*slot).to_string();
                        obj.insert(key, val);
                    }

                    let result = match self.run(&graph_ref, Value::Object(obj)) {
                        Ok(v) => v,
                        Err(e) => return (Err(e), outputs),
                    };
                    outputs.insert(*node_id, result);
                }
                NodeKind::Loop {
                    iterable,
                    body_graph,
                    value_name,
                    index_name,
                    captured,
                } => {
                    let array = match self.resolve_data_ref(iterable, &outputs) {
                        Ok(v) => v,
                        Err(detail) => {
                            return (
                                Err(ExecutionError::MissingOutput {
                                    node: iterable.node,
                                    detail,
                                }),
                                outputs,
                            );
                        }
                    };
                    let array = match array.as_array() {
                        Some(a) => a,
                        None => {
                            return (
                                Err(ExecutionError::TypeMismatch {
                                    expected: "array".into(),
                                    found: array.type_name().into(),
                                    node: *node_id,
                                }),
                                outputs,
                            );
                        }
                    };

                    for (idx, item) in array.iter().enumerate() {
                        let mut obj = indexmap::IndexMap::new();
                        obj.insert(self.interner.resolve(*value_name).to_string(), item.clone());
                        if let Some(idx_name) = index_name {
                            obj.insert(
                                self.interner.resolve(*idx_name).to_string(),
                                Value::Number(idx as i64),
                            );
                        }
                        for (name, data_ref) in captured {
                            let val = match self.resolve_data_ref(data_ref, &outputs) {
                                Ok(v) => v,
                                Err(detail) => {
                                    return (
                                        Err(ExecutionError::MissingOutput {
                                            node: data_ref.node,
                                            detail,
                                        }),
                                        outputs,
                                    );
                                }
                            };
                            obj.insert(self.interner.resolve(*name).to_string(), val);
                        }

                        let body_graph_ref = match self.ir.graphs.get(body_graph) {
                            Some(g) => g,
                            None => {
                                return (
                                    Err(ExecutionError::GraphNotFound {
                                        graph_id: body_graph.clone(),
                                    }),
                                    outputs,
                                );
                            }
                        };
                        let (body_result, body_outputs) =
                            self.run_graph(body_graph_ref, Value::Object(obj));

                        // Propagate mutated captured variables back to parent outputs.
                        for (name, parent_data_ref) in captured {
                            if let Some(body_data_ref) = body_graph_ref.bindings.get(name) {
                                if let Ok(new_val) =
                                    self.resolve_data_ref(body_data_ref, &body_outputs)
                                {
                                    outputs.insert(parent_data_ref.node, new_val.clone());
                                }
                            }
                        }

                        match body_result {
                            Ok(_) => {}
                            Err(ExecutionError::Break) => break,
                            Err(ExecutionError::Continue) => continue,
                            Err(e) => return (Err(e), outputs),
                        }
                    }

                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::WhileLoop {
                    body_graph,
                    captured,
                } => {
                    let mut iterations: u64 = 0;
                    loop {
                        let mut obj = indexmap::IndexMap::new();
                        for (name, data_ref) in captured {
                            let val = match self.resolve_data_ref(data_ref, &outputs) {
                                Ok(v) => v,
                                Err(detail) => {
                                    return (
                                        Err(ExecutionError::MissingOutput {
                                            node: data_ref.node,
                                            detail,
                                        }),
                                        outputs,
                                    );
                                }
                            };
                            obj.insert(self.interner.resolve(*name).to_string(), val);
                        }

                        let body_graph_ref = match self.ir.graphs.get(body_graph) {
                            Some(g) => g,
                            None => {
                                return (
                                    Err(ExecutionError::GraphNotFound {
                                        graph_id: body_graph.clone(),
                                    }),
                                    outputs,
                                );
                            }
                        };
                        let (body_result, body_outputs) =
                            self.run_graph(body_graph_ref, Value::Object(obj));

                        // Propagate mutated captured variables back to parent outputs.
                        for (name, parent_data_ref) in captured {
                            if let Some(body_data_ref) = body_graph_ref.bindings.get(name) {
                                if let Ok(new_val) =
                                    self.resolve_data_ref(body_data_ref, &body_outputs)
                                {
                                    outputs.insert(parent_data_ref.node, new_val.clone());
                                }
                            }
                        }

                        match body_result {
                            Ok(result) => {
                                let result_map = match &result {
                                    Value::Object(map) => map,
                                    _ => break,
                                };

                                let cond = result_map
                                    .get("__cond")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);

                                if !cond {
                                    break;
                                }

                                // The condition held â€” this iteration will
                                // execute the body. Count it against the cap
                                // (condition-only entries don't count).
                                iterations += 1;
                                if iterations > self.limits.max_loop_iterations {
                                    return (
                                        Err(ExecutionError::LoopLimitExceeded {
                                            node: *node_id,
                                            limit: self.limits.max_loop_iterations,
                                        }),
                                        outputs,
                                    );
                                }
                            }
                            Err(ExecutionError::Break) => break,
                            Err(ExecutionError::Continue) => continue,
                            Err(e) => return (Err(e), outputs),
                        }
                    }

                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::Break => {
                    control = Some(ExecutionError::Break);
                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::Continue => {
                    control = Some(ExecutionError::Continue);
                    outputs.insert(*node_id, Value::Null);
                }
                NodeKind::Return { value } => {
                    let v = match self.resolve_data_ref(value, &outputs) {
                        Ok(v) => v,
                        Err(detail) => {
                            return (
                                Err(ExecutionError::MissingOutput {
                                    node: value.node,
                                    detail,
                                }),
                                outputs,
                            );
                        }
                    };
                    early_return = Some(v);
                    control = Some(ExecutionError::Returned);
                    outputs.insert(*node_id, Value::Null);
                }
                other => {
                    return (
                        Err(ExecutionError::UnsupportedNode {
                            node: *node_id,
                            kind: format!("{other:?}"),
                        }),
                        outputs,
                    );
                }
            }
        }

        // A `return` node short-circuits everything, including the Output
        // node's value and loop control sentinels.
        let result = if let Some(v) = early_return {
            Ok(v)
        } else {
            match control {
                Some(err) => Err(err),
                None => outputs.get(&graph.return_node).cloned().ok_or_else(|| {
                    ExecutionError::MissingReturnValue {
                        graph_id: graph.graph_id.clone(),
                    }
                }),
            }
        };
        (result, outputs)
    }

    /// Resolve a `DataRef` to a `Value` by looking up the source node's output.
    ///
    /// The `Err` variant carries a human-readable detail string describing
    /// exactly what was missing, so failures name their cause instead of a
    /// bare node id.
    fn resolve_data_ref(
        &self,
        data_ref: &quew_ir::graph::DataRef,
        outputs: &HashMap<NodeId, Value>,
    ) -> Result<Value, String> {
        let base = match outputs.get(&data_ref.node) {
            Some(v) => v.clone(),
            None => {
                return Err(format!(
                    "node {} never produced an output",
                    data_ref.node.0
                ));
            }
        };

        match &data_ref.slot {
            None => Ok(base),
            Some(slot) => {
                let field_name = self.interner.resolve(*slot).to_string();
                match base {
                    Value::Object(map) => map.get(&field_name).cloned().ok_or_else(|| {
                        format!(
                            "output of node {} has no field '{field_name}'",
                            data_ref.node.0
                        )
                    }),
                    other => Err(format!(
                        "output of node {} is {}, expected an object to read \
                         field '{field_name}' from",
                        data_ref.node.0,
                        other.type_name()
                    )),
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
}

#[cfg(test)]
#[path = "execution_tests/mod.rs"]
mod execution_tests;
