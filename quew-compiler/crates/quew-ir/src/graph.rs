//! Execution graph — nodes, edges, and data flow.
//!
//! Each `AgentGraph` is a directed acyclic graph (DAG) of `IrNode`s connected
//! by `Edge`s. The graph describes the execution order and data flow for one
//! agent or function body.
//!
//! ## Node categories (from discussion1.md §10)
//!
//! | Category      | Examples                                  | Checkpoint |
//! |---------------|-------------------------------------------|------------|
//! | Boundary      | `Input`, `Context`, `Output`              | Never      |
//! | Deterministic | `LetBind`, `Branch`, `FuncCall` (pure)   | Optional   |
//! | Effectful     | `HostToolCall`, `AgentCall`, `Reply`      | Required   |
//!
//! Deterministic nodes replay instantly from their inputs — they do not need a
//! checkpoint. Effectful nodes touch the outside world (LLM, host FFI, child
//! agents) and always checkpoint before and after.

use indexmap::IndexMap;
use quew_interner::InternedStr;

use crate::defs::{ModelDef, ProviderKind};
use crate::types::IrType;

// ── Graph ─────────────────────────────────────────────────────────────────────

/// An executable graph for one agent or function body.
///
/// The graph is **immutable** once compiled. The runtime reads it but never
/// mutates it. Execution progress is tracked in a separate journal object.
#[derive(Debug, Clone)]
pub struct AgentGraph {
    /// Globally unique graph identifier (e.g. `"agent:Main"`, `"function:sanitize"`).
    pub graph_id: String,
    /// The node id where execution starts (always an `Input` boundary node).
    pub entry_node: NodeId,
    /// The node id that produces the final output (always an `Output` boundary node).
    pub return_node: NodeId,
    /// All nodes in this graph, keyed by stable `NodeId`.
    ///
    /// `IndexMap` preserves the lowerer's insertion order for deterministic
    /// traversal while allowing direct lookup by id during runtime execution.
    pub nodes: IndexMap<NodeId, IrNode>,
    /// Data-flow edges: `from.output → to.slot`.
    pub edges: Vec<Edge>,
}

impl AgentGraph {
    /// Look up a node by id. Panics if the node does not exist (indicates a
    /// lowering bug — every id in an edge must reference a real node).
    pub fn node(&self, id: NodeId) -> &IrNode {
        self.nodes.get(&id).unwrap_or_else(|| {
            panic!(
                "IR integrity error: node {id:?} not found in graph {}",
                self.graph_id
            )
        })
    }
}

// ── Node identity ─────────────────────────────────────────────────────────────

/// A stable, deterministic node identifier within one graph.
///
/// Format: sequential integer assigned during lowering (`n0`, `n1`, …).
/// The journal uses `(graph_id, NodeId)` as its checkpoint key.
/// NodeIds are stable across recompilations of the same source — the lowerer
/// always assigns them in the same traversal order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "n{}", self.0)
    }
}

// ── Node ──────────────────────────────────────────────────────────────────────

/// A single execution unit in an agent graph.
#[derive(Debug, Clone)]
pub struct IrNode {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Whether the journal must checkpoint before/after this node.
    pub checkpoint: CheckpointPolicy,
}

/// The kind of an `IrNode` — determines how the runtime executes it.
#[derive(Debug, Clone)]
pub enum NodeKind {
    // ── Boundary (connects graph to the outside call) ─────────────────────────
    /// The agent's input value. Always `n0` in an agent graph.
    /// In a function graph this is the first parameter.
    Input { input_ty: IrType },

    /// The `@context(T)` context object injected by the runtime.
    /// Only present when the agent has a `@context(T)` annotation.
    Context { context_ty: InternedStr },

    /// The final output node. Produces the graph's return value.
    Output { value: DataRef },

    // ── Deterministic (replayable; no checkpoint required by default) ─────────
    /// `let x = expr` — pure expression evaluation.
    ///
    /// The bound name `x` becomes a data slot referenced by downstream nodes
    /// via `DataRef { node: this_id, slot: None }` (scalar) or
    /// `DataRef { node: this_id, slot: Some("field") }` (record field).
    LetBind { name: InternedStr, value: IrExpr },

    /// `if condition { … } [else { … }]`
    ///
    /// The executor evaluates `condition`, then follows `then_node` or
    /// `else_node`. The branch taken is recorded in the journal so that on
    /// resume the executor does not re-evaluate the condition.
    Branch {
        condition: DataRef,
        then_node: NodeId,
        else_node: Option<NodeId>,
    },

    /// A call to a pure or internal DSL function.
    /// If the function contains only pure expressions, the node is deterministic
    /// and does not require a checkpoint. If the function calls host tools or
    /// stdlib I/O (`fetch`), the `checkpoint` field on the node is set to
    /// `Required` by the lowerer.
    FuncCall {
        /// Name of the function — key in `definitions.functions`.
        function: InternedStr,
        args: IndexMap<InternedStr, DataRef>,
    },

    // ── Effectful (checkpoint required) ──────────────────────────────────────
    /// A call to a host-backed tool from agent code (not from the model).
    ///
    /// This is distinct from tools *exposed to the model* inside a `Reply`
    /// node. `HostToolCall` is when the agent code itself calls a tool before
    /// or between model turns.
    HostToolCall {
        /// Name of the tool — key in `definitions.tools`.
        tool: InternedStr,
        args: IndexMap<InternedStr, DataRef>,
    },

    /// `reply(input) with { … }` — the LLM boundary.
    ///
    /// This node owns the full conversation loop: system prompt generation,
    /// streaming, tool dispatch, retry, and fallback. It is always checkpointed.
    ///
    /// The v1 runtime loop becomes the implementation of this node type.
    Reply {
        /// The value passed to `reply(…)` — the user's message.
        message: DataRef,
        config: ReplyConfig,
    },

    /// `return Agent(input)` or `return Agent(input) with turns`.
    ///
    /// Delegates execution to another agent. The `mode` controls whether
    /// the child's turn trace is merged into the parent's journal context.
    AgentCall {
        /// Name of the agent — key in `definitions.agents`.
        agent: InternedStr,
        args: IndexMap<InternedStr, DataRef>,
        mode: AgentCallMode,
    },
}

/// How an `AgentCall` node merges the child's execution context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCallMode {
    /// `return Agent(input)` — parent journal records only the child's
    /// final output. Child's internal turns are invisible to the parent.
    BlackBox,
    /// `return Agent(input) with turns` — child nodes are logically inlined
    /// into the parent graph. The parent journal carries the child's full
    /// turn trace with a cursor marking the start point.
    WithTurns,
}

/// Whether the journal must checkpoint around this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointPolicy {
    /// Journal must save before and after. Required for all effectful nodes:
    /// `HostToolCall`, `AgentCall`, `Reply`.
    Required,
    /// Journal may save (useful for debugging and observability).
    /// Default for deterministic nodes.
    Optional,
    /// Never save. Used for boundary nodes (`Input`, `Context`, `Output`)
    /// and trivial `LetBind` expressions over literals.
    Never,
}

// ── Reply configuration ───────────────────────────────────────────────────────

/// The lowered form of a `reply(…) with { … }` configuration block.
#[derive(Debug, Clone)]
pub struct ReplyConfig {
    pub prompt: IrPrompt,
    /// The model to use for this reply. Inline `gemini("…")` calls are
    /// interned in `definitions.models` and referenced here by name.
    pub model: ModelRef,
    /// Fallback model if the primary fails or is rate-limited.
    pub fallback: Option<ModelRef>,
    /// How many times to retry on failure before giving up (or using fallback).
    pub retry: Option<u32>,
    /// Maximum number of model turns before the reply node terminates.
    pub max_turn: Option<u32>,
    /// Tools exposed to the model inside this reply.
    pub tools: Vec<ToolRef>,
    /// Built-in provider tools (e.g. `web_search`, `code_execution`).
    pub builtin: Vec<InternedStr>,
    /// Sub-agents the model can hand off to inside this reply.
    pub agents: Vec<AgentRef>,
}

/// The prompt expression for a reply node.
#[derive(Debug, Clone)]
pub enum IrPrompt {
    /// A plain string literal prompt.
    Literal(InternedStr),
    // Future: template with interpolated data refs.
}

/// A reference to a model — either a named definition or an anonymous inline.
#[derive(Debug, Clone)]
pub enum ModelRef {
    /// `model: MyModel` — refers to a named entry in `definitions.models`.
    Named(InternedStr),
    /// `model: gemini("gemini-pro")` — anonymous inline, still interned in
    /// `definitions.models` under a generated key.
    Inline { key: InternedStr, def: ModelDef },
}

impl ModelRef {
    /// The interned key this model was stored under in `definitions.models`.
    pub fn key(&self) -> InternedStr {
        match self {
            Self::Named(k) | Self::Inline { key: k, .. } => *k,
        }
    }

    pub fn provider(&self) -> ProviderKind {
        match self {
            Self::Named(_) => panic!("ModelRef::Named cannot resolve provider without definitions"),
            Self::Inline { def, .. } => def.provider,
        }
    }
}

/// A tool reference inside a `ReplyConfig::tools` list.
#[derive(Debug, Clone)]
pub struct ToolRef {
    /// Name of the tool — key in `definitions.tools`.
    pub name: InternedStr,
    /// Pre-bound host arguments from call-site syntax:
    /// `delete_person(ctx.isAdmin)` → `host_args["isAdmin"] = DataRef { ctx_node, "isAdmin" }`
    ///
    /// Empty for tools with no host params (e.g. `tools: [getWeather]`).
    pub host_args: IndexMap<InternedStr, DataRef>,
}

/// A sub-agent reference inside a `ReplyConfig::agents` list.
#[derive(Debug, Clone)]
pub struct AgentRef {
    /// Name of the agent — key in `definitions.agents`.
    pub name: InternedStr,
    /// How the agent handles its output relative to the parent.
    pub handoff: AgentCallMode,
}

// ── Expressions ───────────────────────────────────────────────────────────────

/// An inline expression stored inside a `LetBind` node.
///
/// These cover pure, deterministic computations. Complex expressions (agent
/// calls, tool calls) are always their own nodes, not embedded `IrExpr`s.
#[derive(Debug, Clone)]
pub enum IrExpr {
    /// A literal value.
    Lit(IrLit),
    /// A reference to another node's output (or a field of it).
    Ref(DataRef),
    /// A binary operation: `left op right`.
    Binary {
        left: Box<IrExpr>,
        op: BinaryOp,
        right: Box<IrExpr>,
    },
    /// A unary operation: `op expr`.
    Unary { op: UnaryOp, expr: Box<IrExpr> },
    /// A member access: `base.field`.
    Member {
        base: Box<IrExpr>,
        field: InternedStr,
    },
    /// A function call that evaluates to a value (pure functions only).
    Call {
        function: InternedStr,
        args: IndexMap<InternedStr, IrExpr>,
    },
    /// An array literal: `[elem, …]`.
    Array(Vec<IrExpr>),
    /// A ternary/inline conditional: `expr if cond else expr`.
    Ternary {
        cond: Box<IrExpr>,
        then: Box<IrExpr>,
        else_: Box<IrExpr>,
    },
}

/// A literal value in the IR.
#[derive(Debug, Clone, PartialEq)]
pub enum IrLit {
    String(InternedStr),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

/// Binary operators — a strict subset of what the AST supports.
/// Only operators that can appear in `IrExpr::Binary`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

// ── Data flow ─────────────────────────────────────────────────────────────────

/// A reference to data produced by a prior node in the same graph.
///
/// When `slot` is `None`, the reference is to the node's scalar output.
/// When `slot` is `Some("field")`, the reference is to a named field of the
/// node's record output (e.g. `ctx.userId` → `DataRef { node: ctx_id, slot: Some("userId") }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRef {
    /// The `NodeId` that produced this value.
    pub node: NodeId,
    /// Optional field name if the node output is a record.
    pub slot: Option<InternedStr>,
}

impl DataRef {
    /// A reference to the whole output of a node (no field selection).
    pub fn scalar(node: NodeId) -> Self {
        Self { node, slot: None }
    }

    /// A reference to a specific field of a node's record output.
    pub fn field(node: NodeId, slot: InternedStr) -> Self {
        Self {
            node,
            slot: Some(slot),
        }
    }
}

/// A directed data-flow edge from one node's output to another node's input slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// The node that produces the value.
    pub from: NodeId,
    /// The node that consumes the value.
    pub to: NodeId,
    /// Which input slot of `to` receives this value.
    pub slot: InternedStr,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u32) -> IrNode {
        IrNode {
            id: NodeId(id),
            kind: NodeKind::Input {
                input_ty: IrType::Text,
            },
            checkpoint: CheckpointPolicy::Never,
        }
    }

    fn graph_with_nodes(ids: &[u32]) -> AgentGraph {
        let mut nodes = IndexMap::new();
        for id in ids {
            nodes.insert(NodeId(*id), node(*id));
        }

        AgentGraph {
            graph_id: "agent:Test".to_string(),
            entry_node: NodeId(ids.first().copied().unwrap_or(0)),
            return_node: NodeId(ids.last().copied().unwrap_or(0)),
            nodes,
            edges: Vec::new(),
        }
    }

    #[test]
    fn node_returns_the_node_for_a_valid_id() {
        let graph = graph_with_nodes(&[0, 1, 2]);

        assert_eq!(graph.node(NodeId(2)).id, NodeId(2));
    }

    #[test]
    #[should_panic(expected = "IR integrity error: node NodeId(7) not found in graph agent:Test")]
    fn node_panics_for_a_missing_id() {
        let graph = graph_with_nodes(&[0, 1, 2]);

        let _ = graph.node(NodeId(7));
    }

    #[test]
    fn nodes_keep_lowerer_insertion_order() {
        let graph = graph_with_nodes(&[2, 0, 1]);
        let ids: Vec<_> = graph.nodes.keys().copied().collect();

        assert_eq!(ids, vec![NodeId(2), NodeId(0), NodeId(1)]);
    }
}
