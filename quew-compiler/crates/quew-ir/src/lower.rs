//! AST → IR lowering pass.
//!
//! This module is the **only** entry point into `quew-ir`. Everything else
//! in this crate is type definitions. The actual lowering logic is split into
//! focused sub-modules:
//!
//! | Sub-module    | Responsibility                                        |
//! |---------------|-------------------------------------------------------|
//! | `defs`        | Lower top-level declarations into `Definitions`      |
//! | `graph`       | Lower an agent/function body into an `AgentGraph`    |
//! | `expr`        | Lower `Expr` nodes into `IrExpr` / `DataRef`         |
//! | `config`      | Lower a `WithBlock` into `ReplyConfig`                |
//! | `ctx`         | `LowerCtx` — node counter, slot map, diagnostics     |
//!
//! ## Invariants
//!
//! - `lower()` is called **only** on a module that passed the checker with zero
//!   errors. Do not call it on a module with diagnostics of severity `Error`.
//! - The lowerer never panics on valid input. It may panic with a clear
//!   "lowering bug" message on inputs that should have been rejected by the
//!   checker — these indicate a checker gap, not a user error.
//! - No I/O. No external calls. Pure transformation.

pub mod config;
pub mod ctx;
pub mod defs;
pub mod expr;
pub mod graph_lower;

use std::sync::Arc;

use quew_ast::Module;
use quew_checker::CheckResult;
use quew_interner::{InternedStr, Interner};

use crate::{Definitions, ProgramMeta, QuewGraphIR};


/// Lower a type-checked `Module` into a `QuewGraphIR`.
///
/// # Preconditions
///
/// - `check_result.diagnostics` must contain zero items with `Severity::Error`.
///   The lowerer trusts the checker's output and does not re-validate.
///
/// # Returns
///
/// The complete in-memory execution graph. The caller (typically `quew-cli`)
/// holds this in an `Arc<QuewGraphIR>` and passes it to the runtime.
pub fn lower(module: &Module, check: &CheckResult, interner: &Arc<Interner>) -> QuewGraphIR {
    let mut graphs = indexmap::IndexMap::new();

    // ── 1. Lower definitions ─────────────────────────────────────────────────
    let mut definitions = Definitions::default();
    defs::lower_definitions(module, check, interner, &mut definitions, &mut graphs);

    // ── 2. Lower agent bodies ─────────────────────────────────────────────────
    for item in &module.items {
        if let quew_ast::Item::Agent(agent) = item {
            let graph_key = format!("agent:{}", interner.resolve(agent.name));
            let graph = graph_lower::lower_agent(agent, check, interner, &definitions);
            graphs.insert(graph_key, graph);
        }
    }

    // ── 3. Determine entry agent ──────────────────────────────────────────────
    // The entry agent is the first agent declared in the module.
    // Future: an explicit `@entry` annotation will override this.
    let entry_agent: InternedStr = module
        .items
        .iter()
        .find_map(|i| {
            if let quew_ast::Item::Agent(a) = i { Some(a.name) } else { None }
        })
        .expect("lower() called on a module with no agent declarations");

    QuewGraphIR {
        program: ProgramMeta { name: entry_agent, entry_agent },
        definitions,
        graphs,
    }
}
