//! Lowering context — shared state threaded through the lowering pass.
//!
//! `LowerCtx` holds the node counter (for stable `NodeId` assignment),
//! the current data-slot map (name → NodeId), and any lowering diagnostics.

use indexmap::IndexMap;
use quew_interner::InternedStr;

use crate::graph::NodeId;

/// Mutable state for one graph lowering pass.
pub struct LowerCtx {
    /// Monotonically increasing counter for assigning `NodeId`s.
    next_id: u32,
    /// Maps in-scope name bindings to the node that produced them.
    /// Used to resolve `Expr::Ident` references during expression lowering.
    pub slots: IndexMap<InternedStr, NodeId>,
}

impl LowerCtx {
    pub fn new() -> Self {
        Self { next_id: 0, slots: IndexMap::new() }
    }

    /// Allocate the next stable `NodeId`.
    pub fn next_node(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Bind a name to the node that produced it.
    pub fn bind(&mut self, name: InternedStr, node: NodeId) {
        self.slots.insert(name, node);
    }

    /// Resolve a name to its producing node. Returns `None` for unknown names
    /// (the checker should have caught these; this signals a lowering bug).
    pub fn resolve(&self, name: InternedStr) -> Option<NodeId> {
        self.slots.get(&name).copied()
    }
}

impl Default for LowerCtx {
    fn default() -> Self {
        Self::new()
    }
}
