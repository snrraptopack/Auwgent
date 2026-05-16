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
            let graph = graph_lower::lower_agent(agent, check, interner, &mut definitions);
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
            if let quew_ast::Item::Agent(a) = i {
                Some(a.name)
            } else {
                None
            }
        })
        .expect("lower() called on a module with no agent declarations");

    QuewGraphIR {
        program: ProgramMeta {
            name: entry_agent,
            entry_agent,
        },
        definitions,
        graphs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quew_errors::Severity;
    use quew_source::SourceMap;

    fn lower_source(source: &str) -> (Arc<Interner>, QuewGraphIR) {
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
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "checker errors: {:?}",
            check.diagnostics
        );
        let ir = lower(&parse.module, &check, &interner);
        (interner, ir)
    }

    #[test]
    fn lowers_basic_reply_graph() {
        let (_interner, ir) = lower_source(
            r#"
agent Hello(input: string) {
    reply(input) with {
        prompt: "Say hi"
        model: gemini("gemini-pro")
    }
}
"#,
        );

        let graph = &ir.graphs["agent:Hello"];
        assert_eq!(graph.nodes.len(), 3);
        assert!(matches!(
            graph.node(graph.entry_node).kind,
            crate::graph::NodeKind::Input { .. }
        ));
        assert!(
            graph
                .nodes
                .values()
                .any(|node| matches!(node.kind, crate::graph::NodeKind::Reply { .. }))
        );
        assert!(matches!(
            graph.node(graph.return_node).kind,
            crate::graph::NodeKind::Output { .. }
        ));
    }

    #[test]
    fn lowers_with_turns_to_agent_call_mode() {
        let (interner, ir) = lower_source(
            r#"
agent Child(input: string) {
    reply(input) with { prompt: "child", model: gemini("gemini-pro") }
}

agent Main(input: string) {
    return Child(input) with turns
}
"#,
        );

        let graph = &ir.graphs["agent:Main"];
        let child = interner.intern("Child");
        assert!(graph.nodes.values().any(|node| {
            matches!(
                &node.kind,
                crate::graph::NodeKind::AgentCall { agent, mode: crate::graph::AgentCallMode::WithTurns, .. }
                    if *agent == child
            )
        }));
    }

    #[test]
    fn lowers_native_annotation_into_agent_protocol() {
        let (interner, ir) = lower_source(
            r#"
@native
agent Vision(input: string) {
    reply(input) with { prompt: "look", model: gemini("gemini-pro") }
}
"#,
        );

        let agent = &ir.definitions.agents[&interner.intern("Vision")];
        assert_eq!(agent.protocol, crate::defs::ProtocolMode::Native);
    }
}
