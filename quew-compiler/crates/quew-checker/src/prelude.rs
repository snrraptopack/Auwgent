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
const NATIVE_PRELUDE: &str = include_str!("../../../prelude/native.quew");

const PRELUDE_FILES: &[(&str, &str)] = &[
    ("<quew-prelude:tools.quew>", TOOLS_PRELUDE),
    ("<quew-prelude:with.quew>", WITH_PRELUDE),
    ("<quew-prelude:models.quew>", MODELS_PRELUDE),
    ("<quew-prelude:native.quew>", NATIVE_PRELUDE),
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
        assert_eq!(parsed.module.items.len(), 19);
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

        // String builtins
        let len = interner.intern("len");
        let is_empty = interner.intern("is_empty");
        let contains = interner.intern("contains");
        assert_eq!(
            table.globals[&len].native,
            Some(interner.intern("std.string.len"))
        );
        assert_eq!(
            table.globals[&is_empty].native,
            Some(interner.intern("std.string.is_empty"))
        );
        assert_eq!(
            table.globals[&contains].native,
            Some(interner.intern("std.string.contains"))
        );

        // Number builtins
        let abs = interner.intern("abs");
        let clamp = interner.intern("clamp");
        assert_eq!(
            table.globals[&abs].native,
            Some(interner.intern("std.number.abs"))
        );
        assert_eq!(
            table.globals[&clamp].native,
            Some(interner.intern("std.number.clamp"))
        );

        // Array builtins
        let array_len = interner.intern("array_len");
        let array_get = interner.intern("array_get");
        let array_push = interner.intern("array_push");
        let array_pop = interner.intern("array_pop");
        assert_eq!(
            table.globals[&array_len].native,
            Some(interner.intern("std.array.len"))
        );
        assert_eq!(
            table.globals[&array_get].native,
            Some(interner.intern("std.array.get"))
        );
        assert_eq!(
            table.globals[&array_push].native,
            Some(interner.intern("std.array.push"))
        );
        assert_eq!(
            table.globals[&array_pop].native,
            Some(interner.intern("std.array.pop"))
        );

        // Extension methods
        assert!(
            table
                .extension_methods
                .iter()
                .any(|method| method.name == interner.intern("len"))
        );
        assert!(
            table
                .extension_methods
                .iter()
                .any(|method| method.name == interner.intern("isEmpty"))
        );
        assert!(
            table
                .extension_methods
                .iter()
                .any(|method| method.name == interner.intern("contains"))
        );
        assert!(
            table
                .extension_methods
                .iter()
                .any(|method| method.name == interner.intern("startsWith"))
        );
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
        assert_eq!(merged.module.items.len(), 19);
    }
}
