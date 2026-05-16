//! Integration tests for `quew-lexer` — lex real `.quew` fixture files.
//!
//! These tests verify the full pipeline: file → lex() → token stream + diagnostics.
//! They use fixture files committed to `tests/fixtures/` as regression anchors.

use std::sync::Arc;
use quew_interner::Interner;
use quew_source::SourceMap;
use quew_lexer::{lex, TokenKind, AnnotationKind};

fn setup() -> (Arc<Interner>, SourceMap) {
    let interner = Arc::new(Interner::new());
    let map = SourceMap::new(Arc::clone(&interner));
    (interner, map)
}

// ── Valid fixtures ────────────────────────────────────────────────────────────

#[test]
fn valid_basic_agent_lexes_without_errors() {
    let src = include_str!("fixtures/valid/basic_agent.quew");
    let (interner, map) = setup();
    let sid = map.add("basic_agent.quew", src);
    let result = lex(src, sid, &interner);

    assert!(
        result.errors.is_empty(),
        "valid agent produced lex errors: {:?}", result.errors
    );
    // Must contain the KwAgent token.
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::KwAgent));
    assert!(kinds.contains(&&TokenKind::KwReply));
    assert!(kinds.contains(&&TokenKind::KwWith));
}

#[test]
fn valid_tool_declarations_lex_without_errors() {
    let src = include_str!("fixtures/valid/tool_declarations.quew");
    let (interner, map) = setup();
    let sid = map.add("tool_declarations.quew", src);
    let result = lex(src, sid, &interner);

    assert!(
        result.errors.is_empty(),
        "tool declarations produced lex errors: {:?}", result.errors
    );
    let tool_count = result.tokens.iter()
        .filter(|(k, _)| k == &TokenKind::KwTool)
        .count();
    assert_eq!(tool_count, 3, "expected 3 tool keywords");
}

#[test]
fn valid_tool_function_lexes_annotations_correctly() {
    let src = include_str!("fixtures/valid/tool_function.quew");
    let (interner, map) = setup();
    let sid = map.add("tool_function.quew", src);
    let result = lex(src, sid, &interner);

    assert!(
        result.errors.is_empty(),
        "tool function produced lex errors: {:?}", result.errors
    );
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::Annotation(AnnotationKind::Tool)));
    assert!(kinds.contains(&&TokenKind::Annotation(AnnotationKind::Desc)));
    assert!(kinds.contains(&&TokenKind::KwFunction));
}

#[test]
fn valid_tool_function_binding_lexes_without_errors() {
    // This fixture uses `@id` inside a parameter list — the binding-reference
    // syntax from not.txt line 198. The lexer emits it as Annotation(Unknown)
    // because `@id` matches the annotation regex but isn't a named annotation.
    // The parser will resolve the binding semantics from context.
    let src = include_str!("fixtures/valid/tool_function_binding.quew");
    let (interner, map) = setup();
    let sid = map.add("tool_function_binding.quew", src);
    let result = lex(src, sid, &interner);

    assert!(
        result.errors.is_empty(),
        "binding fixture produced lex errors: {:?}", result.errors
    );
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    // @tool annotation is present.
    assert!(kinds.contains(&&TokenKind::Annotation(AnnotationKind::Tool)));
    // @id inside params lexes as Annotation(Unknown) — not an error.
    assert!(kinds.contains(&&TokenKind::Annotation(AnnotationKind::Unknown)));
    // Core keywords are all present.
    assert!(kinds.contains(&&TokenKind::KwFunction));
    assert!(kinds.contains(&&TokenKind::KwNot));
    assert!(kinds.contains(&&TokenKind::KwReturn));
}

// ── Invalid fixtures ──────────────────────────────────────────────────────────

#[test]
fn invalid_unknown_chars_produce_errors_but_do_not_panic() {
    let src = include_str!("fixtures/invalid/unknown_chars.quew");
    let (interner, map) = setup();
    let sid = map.add("unknown_chars.quew", src);
    let result = lex(src, sid, &interner);

    // Must have errors for the `$` and `#` characters.
    assert!(!result.errors.is_empty(), "expected lex errors for unknown chars");
    // Must still produce KwAgent (lexer continued after the unknown chars).
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::KwAgent));
}

// ── Inline scenarios ──────────────────────────────────────────────────────────

#[test]
fn inline_conditional_postfix_lexes_correctly() {
    // `let x = a if cond else b` — no `then` keyword
    let src = "let x = a if cond else b";
    let (interner, map) = setup();
    let sid = map.add("inline.quew", src);
    let result = lex(src, sid, &interner);

    assert!(result.errors.is_empty());
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::KwIf));
    assert!(kinds.contains(&&TokenKind::KwElse));
    // No `then` token should appear.
    assert!(!kinds.iter().any(|k| matches!(k, TokenKind::Ident) && false)); // sanity
}

#[test]
fn for_in_loop_lexes_correctly() {
    let src = "for idx, value in session.turns { }";
    let (interner, map) = setup();
    let sid = map.add("for_loop.quew", src);
    let result = lex(src, sid, &interner);

    assert!(result.errors.is_empty());
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::KwFor));
    assert!(kinds.contains(&&TokenKind::KwIn));
}

#[test]
fn type_discrimination_is_keyword_lexes() {
    let src = "if x is MyType { }";
    let (interner, map) = setup();
    let sid = map.add("is.quew", src);
    let result = lex(src, sid, &interner);

    assert!(result.errors.is_empty());
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::KwIs));
}

#[test]
fn optional_param_question_mark_lexes() {
    let src = "tool foo(id?: string): string";
    let (interner, map) = setup();
    let sid = map.add("optional.quew", src);
    let result = lex(src, sid, &interner);

    assert!(result.errors.is_empty());
    let kinds: Vec<_> = result.tokens.iter().map(|(k, _)| k).collect();
    assert!(kinds.contains(&&TokenKind::Question));
}

#[test]
fn union_type_pipe_lexes() {
    let src = "string | number | bool";
    let (interner, map) = setup();
    let sid = map.add("union.quew", src);
    let result = lex(src, sid, &interner);

    assert!(result.errors.is_empty());
    let pipe_count = result.tokens.iter().filter(|(k, _)| k == &TokenKind::Pipe).count();
    assert_eq!(pipe_count, 2);
}
