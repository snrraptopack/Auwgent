//! Integration tests for `quew-parser`.
//!
//! Each test:
//! 1. Reads a `.quew` fixture from `quew-lexer`'s fixture directory (shared fixtures).
//! 2. Lexes it with `quew_lexer::lex()`.
//! 3. Parses it with `quew_parser::parse()`.
//! 4. Asserts the expected outcome:
//!    - Valid fixtures → 0 parse errors, correct top-level item count.
//!    - Invalid fixtures → ≥1 parse error OR specific diagnostic kind.

use std::sync::Arc;

use quew_interner::Interner;
use quew_lexer::lex;
use quew_parser::parse;
use quew_source::SourceMap;
use quew_ast::{BuiltinTypeMeta, BuiltinVisibility, Item};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Lex + parse a source string. Uses a temporary SourceMap to produce a SourceId
/// for `lex()`, which needs it for multi-file diagnostic tracking.
fn lex_and_parse(source: &str) -> quew_parser::ParseResult {
    let interner = Arc::new(Interner::new());
    let map = SourceMap::new(Arc::clone(&interner));
    let sid = map.add("<test>", source);
    let lex_result = lex(source, sid, &interner);
    parse(&lex_result, source, &interner)
}

// ── valid fixture tests ───────────────────────────────────────────────────────

#[test]
fn basic_agent_parses_cleanly() {
    let src = include_str!("../../quew-lexer/tests/fixtures/valid/basic_agent.quew");
    let result = lex_and_parse(src);
    assert!(
        result.errors.is_empty(),
        "expected 0 parse errors, got: {:?}",
        result.errors
    );
    // basic_agent.quew declares one agent at the top level
    assert_eq!(result.module.items.len(), 1, "expected 1 top-level item");
}

#[test]
fn tool_function_parses_cleanly() {
    let src = include_str!("../../quew-lexer/tests/fixtures/valid/tool_function.quew");
    let result = lex_and_parse(src);
    assert!(
        result.errors.is_empty(),
        "expected 0 parse errors, got: {:?}",
        result.errors
    );
}

#[test]
fn tool_function_binding_parses_cleanly() {
    let src = include_str!("../../quew-lexer/tests/fixtures/valid/tool_function_binding.quew");
    let result = lex_and_parse(src);
    assert!(
        result.errors.is_empty(),
        "expected 0 parse errors, got: {:?}",
        result.errors
    );
}

// ── unit tests — individual grammar rules ─────────────────────────────────────

#[test]
fn empty_source_yields_empty_module() {
    let result = lex_and_parse("");
    assert!(
        result.errors.is_empty(),
        "empty source should have no errors"
    );
    assert_eq!(result.module.items.len(), 0);
}

#[test]
fn let_declaration_parses() {
    let result = lex_and_parse("let x = 42");
    assert!(
        result.errors.is_empty(),
        "let decl should parse cleanly, errors: {:?}",
        result.errors
    );
    assert_eq!(result.module.items.len(), 1);
}

#[test]
fn string_let_parses() {
    let result = lex_and_parse(r#"let name = "alice""#);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.module.items.len(), 1);
}

/// Correct model syntax: `model Name = { model: gemini("..."), config: { ... } }`
#[test]
fn model_declaration_block_syntax_parses() {
    let src = r#"model MyModel = {
    model: gemini("gemini-pro")
}"#;
    let result = lex_and_parse(src);
    assert!(
        result.errors.is_empty(),
        "model decl errors: {:?}",
        result.errors
    );
    assert_eq!(result.module.items.len(), 1);
}

/// Model with optional config block.
#[test]
fn model_with_config_parses() {
    let src = r#"model MyModel = {
    model: gemini("gemini-pro")
    config: { temperature: 0.7 }
}"#;
    let result = lex_and_parse(src);
    assert!(
        result.errors.is_empty(),
        "model+config errors: {:?}",
        result.errors
    );
}

/// type fields can use keywords as names AND comma or newline as separator.
/// `type Foo = { mode: string, agent: string }` — user's exact requirement.
#[test]
fn type_with_keyword_fields_and_comma_sep_parses() {
    let result = lex_and_parse("type Foo = { mode: string, agent: string }");
    assert!(
        result.errors.is_empty(),
        "keyword field names with comma sep: {:?}",
        result.errors
    );
    assert_eq!(result.module.items.len(), 1);
}

#[test]
fn type_declaration_newline_sep_parses() {
    let src = "type User = {\n  name: string\n  age: number\n}";
    let result = lex_and_parse(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.module.items.len(), 1);
}

#[test]
fn type_with_optional_field_parses() {
    let result = lex_and_parse("type Resp = { name: string, score?: number }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn generic_type_declaration_parses() {
    let result = lex_and_parse("type Box<T> = { value: T }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn nested_generic_type_usage_parses() {
    let result = lex_and_parse("let nested: Box<Pair<string, number>> = value");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn generic_function_declaration_parses() {
    let result = lex_and_parse(
        r#"
function identity<T>(value: T): T {
    return value
}
"#,
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn public_builtin_type_declaration_parses() {
    let result = lex_and_parse("@@type Text = { value: string }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    match &result.module.items[0] {
        Item::Type(decl) => assert_eq!(decl.builtin, BuiltinTypeMeta::public()),
        other => panic!("expected type declaration, got {other:?}"),
    }
}

#[test]
fn internal_builtin_type_declaration_parses() {
    let result = lex_and_parse("!@@type InternalText = { value: string }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    match &result.module.items[0] {
        Item::Type(decl) => assert_eq!(decl.builtin, BuiltinTypeMeta::internal()),
        other => panic!("expected type declaration, got {other:?}"),
    }
}

#[test]
fn role_bound_generic_type_declaration_parses() {
    let result = lex_and_parse("@@(tool, value) type ToolResult<T> = { data: T, error: string }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    match &result.module.items[0] {
        Item::Type(decl) => match &decl.builtin {
            BuiltinTypeMeta::Builtin {
                visibility,
                role: Some(role),
            } => {
                assert_eq!(*visibility, BuiltinVisibility::Public);
                assert_eq!(decl.type_params.len(), 1);
                assert!(role.span.end > role.span.start);
            }
            other => panic!("expected role-bound builtin type, got {other:?}"),
        },
        other => panic!("expected type declaration, got {other:?}"),
    }
}

#[test]
fn multiple_top_level_items_parse() {
    let src = concat!("let x = 1\n", "let y = 2\n", "let z = 3\n",);
    let result = lex_and_parse(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.module.items.len(), 3);
}

#[test]
fn binary_expressions_parse() {
    let result = lex_and_parse("let v = 1 + 2 * 3");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn boolean_literals_parse() {
    let result = lex_and_parse("let t = true\nlet f = false");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.module.items.len(), 2);
}

#[test]
fn null_literal_parses() {
    let result = lex_and_parse("let n = null");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn array_literal_parses() {
    let result = lex_and_parse(r#"let items = ["a", "b", "c"]"#);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

/// `.model` — keywords must be valid field names after a dot.
#[test]
fn keyword_as_field_name_parses() {
    let result = lex_and_parse("let v = config.model");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

/// `result.error.isEmpty()` — member chain then call.
#[test]
fn member_chain_with_call_parses() {
    let result = lex_and_parse("let v = result.error.isEmpty()");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn provider_call_gemini_parses() {
    let result = lex_and_parse(r#"let m = gemini("gemini-pro")"#);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn provider_call_openai_parses() {
    let result = lex_and_parse(r#"let m = openai("gpt-4")"#);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn provider_call_groq_parses() {
    let result = lex_and_parse(r#"let m = groq("llama-3")"#);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

// ── error recovery: parser must not panic on invalid input ────────────────────

#[test]
fn unknown_chars_do_not_panic() {
    let src = include_str!("../../quew-lexer/tests/fixtures/invalid/unknown_chars.quew");
    let result = lex_and_parse(src);
    let _ = result; // must not panic
}

#[test]
fn incomplete_let_produces_error_not_panic() {
    let result = lex_and_parse("let x =");
    let _ = result; // must not panic; errors expected
}

#[test]
fn unclosed_brace_recovers() {
    let result = lex_and_parse("type Foo = {\n  name: string\n");
    let _ = result; // must not panic; errors expected
}
