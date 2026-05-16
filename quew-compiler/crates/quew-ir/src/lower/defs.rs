use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{Item, Module};
use quew_checker::CheckResult;
use quew_interner::Interner;

use crate::defs::Definitions;
use crate::graph::AgentGraph;

/// Walk every top-level `Item` and populate the `Definitions` registry.
/// Also emits sub-graphs for function bodies into `graphs`.
pub fn lower_definitions(
    module: &Module,
    check: &CheckResult,
    _interner: &Arc<Interner>,
    definitions: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
) {

    for item in &module.items {
        match item {
            Item::Type(decl)     => lower_type(decl, check, definitions),
            Item::Model(decl)    => lower_model(decl, check, definitions),
            Item::Tool(decl)     => lower_tool(decl, check, definitions),
            Item::Tools(decl)    => lower_tools_group(decl, check, definitions),
            Item::Function(decl) => lower_function(decl, check, definitions, graphs),
            Item::Agent(_)       => { /* agent bodies handled in lower::lower() */ }
            Item::Let(_)         => { /* top-level let: not yet supported */ }
        }
    }
}

fn lower_type(
    _decl: &quew_ast::TypeDecl,
    _check: &CheckResult,
    _defs: &mut Definitions,
) {
    // TODO: Step 3
}

fn lower_model(
    _decl: &quew_ast::ModelDecl,
    _check: &CheckResult,
    _defs: &mut Definitions,
) {
    // TODO: Step 3
}

fn lower_tool(
    _decl: &quew_ast::ToolDecl,
    _check: &CheckResult,
    _defs: &mut Definitions,
) {
    // TODO: Step 3
}

fn lower_tools_group(
    _decl: &quew_ast::ToolsDecl,
    _check: &CheckResult,
    _defs: &mut Definitions,
) {
    // TODO: Step 3
}

fn lower_function(
    _decl: &quew_ast::FunctionDecl,
    _check: &CheckResult,
    _defs: &mut Definitions,
    _graphs: &mut IndexMap<String, AgentGraph>,
) {
    // TODO: Step 3
}
