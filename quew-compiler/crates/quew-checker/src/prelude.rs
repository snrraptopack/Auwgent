//! Loading and merging Quew-owned prelude source.
//!
//! The prelude is real `.quew` code. Keeping it here lets compiler semantics
//! move into Quew declarations instead of growing hardcoded type names.

use std::sync::Arc;

use quew_ast::Module;
use quew_errors::{Diagnostic, Span};
use quew_interner::Interner;
use quew_source::SourceMap;

const TOOLS_PRELUDE: &str = include_str!("../../../prelude/tools.quew");
const WITH_PRELUDE: &str = include_str!("../../../prelude/with.quew");
const MODELS_PRELUDE: &str = include_str!("../../../prelude/models.quew");
const STRING_PRELUDE: &str = include_str!("../../../prelude/string.quew");
const ARRAY_PRELUDE: &str = include_str!("../../../prelude/array.quew");
const NUMBER_PRELUDE: &str = include_str!("../../../prelude/number.quew");
const IO_PRELUDE: &str = include_str!("../../../prelude/io.quew");
const NET_PRELUDE: &str = include_str!("../../../prelude/net.quew");
const JSON_PRELUDE: &str = include_str!("../../../prelude/json.quew");

const PRELUDE_FILES: &[(&str, &str)] = &[
    ("<quew-prelude:tools.quew>", TOOLS_PRELUDE),
    ("<quew-prelude:with.quew>", WITH_PRELUDE),
    ("<quew-prelude:models.quew>", MODELS_PRELUDE),
    ("<quew-prelude:string.quew>", STRING_PRELUDE),
    ("<quew-prelude:array.quew>", ARRAY_PRELUDE),
    ("<quew-prelude:number.quew>", NUMBER_PRELUDE),
    ("<quew-prelude:io.quew>", IO_PRELUDE),
    ("<quew-prelude:net.quew>", NET_PRELUDE),
    ("<quew-prelude:json.quew>", JSON_PRELUDE),
];

#[derive(Debug)]
pub struct PreludeModule {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn module_with_prelude(user: &Module, interner: &Arc<Interner>) -> PreludeModule {
    let parsed = parse_prelude(interner);
    let mut items = parsed.module.items;
    items.extend(user.items.clone());

    PreludeModule {
        module: Module {
            items,
            span: parsed.module.span.cover(user.span),
        },
        diagnostics: parsed.diagnostics,
    }
}

fn parse_prelude(interner: &Arc<Interner>) -> PreludeModule {
    let source_map = SourceMap::new(Arc::clone(interner));
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    let mut span = Span::new(0, 0);

    for (path, source) in PRELUDE_FILES {
        let source_id = source_map.add(path, *source);
        let lex = quew_lexer::lex(source, source_id, interner);
        let parse = quew_parser::parse(&lex, source, interner);

        diagnostics.extend(lex.errors);
        diagnostics.extend(parse.errors);
        span = span.cover(parse.module.span);
        items.extend(parse.module.items);
    }

    PreludeModule {
        module: Module { items, span },
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quew_scope::{RoleKey, build_symbol_table};

    #[test]
    fn prelude_parses_without_diagnostics() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        assert!(
            parsed.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            parsed.diagnostics
        );
        // Prelude should have at least the core builtins (string, array, number, io).
        assert!(
            parsed.module.items.len() >= 10,
            "prelude seems unexpectedly small: {} items",
            parsed.module.items.len()
        );
    }

    #[test]
    fn prelude_registers_tool_value_role() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        let table = build_symbol_table(&parsed.module, &interner);
        let key = RoleKey {
            keyword: interner.intern("tool"),
            place: interner.intern("value"),
        };

        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert!(table.roles.bindings.contains_key(&key));
    }

    #[test]
    fn prelude_registers_with_body_role() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        let table = build_symbol_table(&parsed.module, &interner);
        let key = RoleKey {
            keyword: interner.intern("with"),
            place: interner.intern("body"),
        };

        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert!(table.roles.bindings.contains_key(&key));
    }

    #[test]
    fn prelude_registers_model_body_role() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        let table = build_symbol_table(&parsed.module, &interner);
        let key = RoleKey {
            keyword: interner.intern("model"),
            place: interner.intern("body"),
        };

        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert!(table.roles.bindings.contains_key(&key));
    }

    #[test]
    fn prelude_registers_model_builder_functions() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        let table = build_symbol_table(&parsed.module, &interner);

        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);
        assert!(table.globals.contains_key(&interner.intern("gemini")));
        assert!(table.globals.contains_key(&interner.intern("openai")));
        assert!(table.globals.contains_key(&interner.intern("groq")));
    }

    #[test]
    fn prelude_registers_native_builtin_functions() {
        let interner = Arc::new(Interner::new());
        let parsed = parse_prelude(&interner);
        let table = build_symbol_table(&parsed.module, &interner);

        assert!(table.diagnostics.is_empty(), "{:?}", table.diagnostics);

        // Helper: assert a builtin function has the expected native binding.
        let assert_native = |name: &str, expected_id: &str| {
            let sym = table
                .globals
                .get(&interner.intern(name))
                .unwrap_or_else(|| panic!("prelude missing builtin: {name}"));
            assert_eq!(
                sym.native,
                Some(interner.intern(expected_id)),
                "builtin '{name}' has wrong native binding"
            );
        };

        // String builtins
        assert_native("len", "std.string.len");
        assert_native("is_empty", "std.string.is_empty");
        assert_native("contains", "std.string.contains");
        assert_native("starts_with", "std.string.starts_with");

        // Number builtins
        assert_native("abs", "std.number.abs");
        assert_native("clamp", "std.number.clamp");

        // Array builtins
        assert_native("array_len", "std.array.len");
        assert_native("array_get", "std.array.get");
        assert_native("array_push", "std.array.push");
        assert_native("array_pop", "std.array.pop");

        // I/O builtins
        assert_native("print", "std.io.print");

        // Extension methods
        let has_ext = |name: &str| {
            table
                .extension_methods
                .iter()
                .any(|method| method.name == interner.intern(name))
        };
        assert!(has_ext("len"), "missing extension method: string.len");
        assert!(has_ext("isEmpty"), "missing extension method: string.isEmpty");
        assert!(has_ext("contains"), "missing extension method: string.contains");
        assert!(has_ext("startsWith"), "missing extension method: string.startsWith");
    }

    #[test]
    fn merge_keeps_user_module_items_after_prelude_items() {
        let interner = Arc::new(Interner::new());
        let user = Module {
            items: vec![],
            span: Span::new(0, 0),
        };
        let merged = module_with_prelude(&user, &interner);

        assert!(merged.diagnostics.is_empty(), "{:?}", merged.diagnostics);
        // Merged module should contain prelude items even when user is empty.
        assert!(
            merged.module.items.len() >= 10,
            "merged module seems unexpectedly small: {} items",
            merged.module.items.len()
        );
    }
}
