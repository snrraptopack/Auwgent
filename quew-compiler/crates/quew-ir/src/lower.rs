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

    #[test]
    fn lowers_extension_method_call_into_func_call() {
        let (interner, ir) = lower_source(
            r#"
extend string {
    function isSuperEmpty(): bool { return true }
}
agent Main(input: string) {
    let result = input.isSuperEmpty()
}
"#,
        );

        // The extension method body should be lowered into a graph.
        let ext_graph = &ir.graphs["extension:string:isSuperEmpty"];
        assert_eq!(ext_graph.nodes.len(), 3); // Input, LetBind(true), Output

        // The agent body should contain a FuncCall to the extension method.
        let agent_graph = &ir.graphs["agent:Main"];
        let func_name = interner.intern("extension:string:isSuperEmpty");
        assert!(agent_graph.nodes.values().any(|node| {
            matches!(
                &node.kind,
                crate::graph::NodeKind::FuncCall { function, .. } if *function == func_name
            )
        }));
    }

    #[test]
    fn lowers_function_body_into_graph() {
        let (interner, ir) = lower_source(
            r#"
function custom_string_is_empty(value: string): bool { return true }
agent Main(input: string) {
    let result = custom_string_is_empty(input)
}
"#,
        );

        let func_graph = &ir.graphs["function:custom_string_is_empty"];
        assert_eq!(func_graph.nodes.len(), 3); // Input, LetBind(true), Output

        let agent_graph = &ir.graphs["agent:Main"];
        let func_name = interner.intern("custom_string_is_empty");
        assert!(agent_graph.nodes.values().any(|node| {
            matches!(
                &node.kind,
                crate::graph::NodeKind::FuncCall { function, .. } if *function == func_name
            )
        }));
    }

    #[test]
    fn lowers_extension_method_calling_another_function() {
        let (interner, ir) = lower_source(
            r#"
function custom_string_is_empty(value: string): bool { return true }
extend string {
    function isSuperEmpty(): bool { return custom_string_is_empty(self) }
}
agent Main(input: string) {
    let result = input.isSuperEmpty()
}
"#,
        );

        // Extension method body should call the function (inlined as IrExpr::Call).
        let ext_graph = &ir.graphs["extension:string:isSuperEmpty"];
        let func_name = interner.intern("custom_string_is_empty");
        assert!(ext_graph.nodes.values().any(|node| {
            matches!(
                &node.kind,
                crate::graph::NodeKind::LetBind { value: crate::graph::IrExpr::Call { function, .. }, .. }
                    if *function == func_name
            )
        }));

        // Agent body should call the extension method.
        let agent_graph = &ir.graphs["agent:Main"];
        let ext_name = interner.intern("extension:string:isSuperEmpty");
        assert!(agent_graph.nodes.values().any(|node| {
            matches!(
                &node.kind,
                crate::graph::NodeKind::FuncCall { function, .. } if *function == ext_name
            )
        }));
    }

    #[test]
    fn lowers_function_call_args_inside_expression() {
        let (interner, ir) = lower_source(
            r#"
function add(a: number, b: number): number { return a }
agent Main(input: number) {
    let result = add(input, 1) + 0
}
"#,
        );

        let agent_graph = &ir.graphs["agent:Main"];
        let func_name = interner.intern("add");
        let a = interner.intern("a");
        let b = interner.intern("b");

        // The binary expression `add(input, 1) + 0` falls through to LetBind
        // with lower_expr, which must preserve the function call arguments.
        let let_bind = agent_graph
            .nodes
            .values()
            .find(|node| matches!(&node.kind, crate::graph::NodeKind::LetBind { .. }))
            .expect("expected a LetBind node");

        if let crate::graph::NodeKind::LetBind { value, .. } = &let_bind.kind {
            if let crate::graph::IrExpr::Binary { left, .. } = value {
                if let crate::graph::IrExpr::Call { function, args } = left.as_ref() {
                    assert_eq!(*function, func_name);
                    assert_eq!(args.len(), 2, "function call should have 2 arguments");
                    assert!(args.contains_key(&a), "args should contain 'a'");
                    assert!(args.contains_key(&b), "args should contain 'b'");
                } else {
                    panic!("expected Call expression, got {:?}", left);
                }
            } else {
                panic!("expected Binary expression, got {:?}", value);
            }
        } else {
            panic!("expected LetBind node");
        }
    }
}
