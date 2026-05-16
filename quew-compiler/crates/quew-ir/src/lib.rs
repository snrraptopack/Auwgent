//! # quew-ir
//!
//! **Single responsibility:** lower the quew AST (after type-checking) into an
//! in-memory `QuewGraphIR` — the v2 execution graph.
//!
//! ## Architecture
//!
//! The IR is a **native Rust struct tree**. It is never serialized to disk by
//! the compiler. The runtime holds it in memory and shares it (behind an `Arc`)
//! across all concurrent runs of the same program.
//!
//! Resumability comes from a *separate* execution state object owned by the
//! runtime — the **journal** — which is serialized after every external
//! interaction. See `not_graph.txt` and `RESUMABLE_GRAPH_IR_PROPOSAL.md` for
//! the two-layer separation.
//!
//! ## Modules
//!
//! | Module     | Contents                                              |
//! |------------|-------------------------------------------------------|
//! | `types`    | IR type representations (`IrType`, `IrField`)        |
//! | `defs`     | Definition-section structs (models, tools, agents…)  |
//! | `graph`    | `AgentGraph`, `IrNode`, `NodeKind`, `Edge`, `DataRef` |
//! | `lower`    | `lower()` — the AST → IR entry point                 |

pub mod defs;
pub mod graph;
pub mod lower;
pub mod types;

use indexmap::IndexMap;
use quew_interner::InternedStr;

pub use defs::Definitions;
pub use graph::AgentGraph;

// ── Top-level IR struct ────────────────────────────────────────────────────────

/// The complete compiled program for one `.quew` source file.
///
/// This is an in-memory native Rust struct. It is **never** serialized to disk
/// by the compiler. The quew runtime loads it directly, shares it across
/// concurrent runs (via `Arc<QuewGraphIR>`), and rebuilds it from source on
/// restart — making it always reproducible from the checked AST.
#[derive(Debug, Clone)]
pub struct QuewGraphIR {
    /// High-level program metadata.
    pub program: ProgramMeta,

    /// Static declarations: types, models, tools, functions, agents.
    /// These answer "what exists?" and are shared by all graphs.
    pub definitions: Definitions,

    /// Executable graphs — one per agent and one per function body.
    ///
    /// Keys use a namespaced format:
    /// - `"agent:Main"` — agent body graph
    /// - `"function:sanitize"` — function body graph
    ///
    /// Using `IndexMap` preserves insertion order for deterministic node IDs
    /// across compilations.
    pub graphs: IndexMap<String, AgentGraph>,
}

/// Top-level program metadata.
#[derive(Debug, Clone)]
pub struct ProgramMeta {
    /// Name of the program (typically the source file name without extension).
    pub name: InternedStr,
    /// The agent that the runtime invokes when `run()` is called.
    pub entry_agent: InternedStr,
}
