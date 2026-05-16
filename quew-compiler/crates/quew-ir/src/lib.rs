//! # quew-ir
//!
//! Lowers the quew AST into an in-memory `ExecutionGraph`.
//!
//! The `ExecutionGraph` is the v2 replacement for the v1 JSON IR. It is consumed
//! directly by the quew runtime (no file serialization in the normal path).
//! For interop and persistence, the graph can be serialized via `serde`.
//!
//! See `RESUMABLE_GRAPH_IR_PROPOSAL.md` for the full design of the graph IR.
//!
//! ## Status: stub

// TODO: define ExecutionGraph, node types, and the lowering pass after quew-checker.
