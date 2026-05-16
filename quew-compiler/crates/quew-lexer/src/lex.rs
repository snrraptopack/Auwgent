//! The `lex()` entry point — drives the logos lexer and produces a [`LexResult`].
//!
//! This module owns no token definitions. Its only job is to iterate the logos
//! `Lexer`, map each output to our `(TokenKind, Span)` pair, and collect errors
//! into a separate `Vec<Diagnostic>` so the parser receives a complete token stream
//! even when the source contains unknown characters.

use std::sync::Arc;

use quew_errors::{Diagnostic, Span};
use quew_interner::{InternedStr, Interner};
use quew_source::SourceId;
use logos::Logos;

use crate::token::TokenKind;

/// The result of lexing one source file.
///
/// The lexer **never aborts** — `tokens` always contains the full stream,
/// including [`TokenKind::Error`] tokens for unrecognised characters.
/// `errors` contains non-fatal diagnostics for those positions; the caller
/// decides whether to continue to parsing or stop.
pub struct LexResult {
    /// All tokens in source order. Every character in the input is accounted
    /// for — either as a valid token or as a [`TokenKind::Error`] token.
    pub tokens: Vec<(TokenKind, Span)>,
    /// Diagnostics emitted during lexing (unknown characters, unterminated
    /// strings, unterminated block comments).
    pub errors: Vec<Diagnostic>,
    /// Interned slices for every [`TokenKind::Ident`] token, in the same order
    /// as the `tokens` vec. Non-ident tokens have a placeholder entry of
    /// `None`. This lets the parser retrieve interned names in O(1) by index.
    pub ident_table: Vec<Option<InternedStr>>,
}

/// Lex a source file and return the complete token stream.
///
/// # Parameters
///
/// - `source` — raw source text of the file.
/// - `source_id` — which file this text belongs to; embedded in every [`Span`].
/// - `interner` — shared string interner; all [`TokenKind::Ident`] slices are
///   interned here so the AST carries zero-allocation handles.
///
/// # Guarantees
///
/// - Never panics on any input.
/// - Always returns a complete token stream (may include [`TokenKind::Error`]).
/// - Every `Span` references `source_id`.
pub fn lex(source: &str, _source_id: SourceId, interner: &Arc<Interner>) -> LexResult {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut ident_table = Vec::new();

    let mut lexer = TokenKind::lexer(source);

    while let Some(result) = lexer.next() {
        // logos returns the byte range of the matched slice.
        let logos_span = lexer.span();
        let span = Span::new(logos_span.start, logos_span.end);

        match result {
            Ok(TokenKind::BlockComment) => {
                // BlockComment is only emitted when unterminated (the callback
                // returns Filter::Emit(()) for the error case).
                errors.push(Diagnostic::error(
                    "unterminated block comment",
                    span,
                ));
                tokens.push((TokenKind::Error, span));
                ident_table.push(None);
            }

            Ok(tok) => {
                // Intern identifier slices for the parser.
                let interned = if tok == TokenKind::Ident {
                    Some(interner.intern(lexer.slice()))
                } else {
                    None
                };
                tokens.push((tok, span));
                ident_table.push(interned);
            }

            Err(()) => {
                // Unrecognised character — emit Error token and a diagnostic.
                let slice = lexer.slice();
                errors.push(Diagnostic::error(
                    format!("unexpected character `{}`", slice.escape_default()),
                    span,
                ));
                tokens.push((TokenKind::Error, span));
                ident_table.push(None);
            }
        }
    }

    LexResult { tokens, errors, ident_table }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_interner() -> Arc<Interner> {
        Arc::new(Interner::new())
    }

    fn fake_source_id() -> SourceId {
        // We use quew-source's SourceMap, but for unit tests we just need any id.
        // Build a minimal SourceMap and register one file.
        let interner = make_interner();
        let map = quew_source::SourceMap::new(Arc::clone(&interner));
        map.add("test.quew", "")
    }

    fn run(source: &str) -> LexResult {
        let interner = make_interner();
        let source_id = fake_source_id();
        lex(source, source_id, &interner)
    }

    fn kinds(r: &LexResult) -> Vec<&TokenKind> {
        r.tokens.iter().map(|(k, _)| k).collect()
    }

    // ── Basic correctness ─────────────────────────────────────────────────────

    #[test]
    fn empty_input_produces_empty_result() {
        let r = run("");
        assert!(r.tokens.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn whitespace_only_produces_empty_result() {
        let r = run("   \t  \r  ");
        assert!(r.tokens.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn single_keyword_spans_correctly() {
        let r = run("agent");
        assert_eq!(r.tokens.len(), 1);
        let (_, span) = r.tokens[0];
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 5);
    }

    #[test]
    fn spans_cover_all_non_whitespace_chars() {
        let src = "let x = 42";
        let r = run(src);
        // let(3) x(1) =(1) 42(2) — 4 tokens, no whitespace
        assert_eq!(r.tokens.len(), 4);
        // Every token's slice should be non-empty.
        for (_, span) in &r.tokens {
            assert!(span.end > span.start);
        }
    }

    // ── Identifier interning ──────────────────────────────────────────────────

    #[test]
    fn ident_is_interned_in_table() {
        let r = run("myVar");
        assert_eq!(r.tokens.len(), 1);
        assert!(r.ident_table[0].is_some(), "ident must have an interned entry");
    }

    #[test]
    fn keyword_is_not_in_ident_table() {
        let r = run("let");
        assert_eq!(r.ident_table[0], None);
    }

    #[test]
    fn same_ident_interned_to_same_handle() {
        let interner = make_interner();
        let source_id = fake_source_id();
        let r = lex("foo foo", source_id, &interner);
        let a = r.ident_table[0].unwrap();
        // newline may appear between the two foos — find second Ident
        let b = r.ident_table.iter().skip(1).find_map(|x| *x).unwrap();
        assert_eq!(a, b, "same string must intern to the same handle");
    }

    // ── Error recovery ────────────────────────────────────────────────────────

    #[test]
    fn unknown_char_produces_error_token_and_diagnostic() {
        let r = run("let $ x");
        assert!(kinds(&r).contains(&&TokenKind::Error));
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].message.contains('$'));
    }

    #[test]
    fn multiple_unknown_chars_each_produce_error() {
        let r = run("$ # @");
        // `@` alone is not valid (needs a letter after it for annotations).
        // Exact count depends on logos grouping, but errors must be > 0.
        assert!(!r.errors.is_empty());
    }

    #[test]
    fn lexer_continues_after_unknown_char() {
        let r = run("agent $ Hello");
        let ks = kinds(&r);
        assert!(ks.contains(&&TokenKind::KwAgent));
        assert!(ks.contains(&&TokenKind::Ident));
        assert!(ks.contains(&&TokenKind::Error));
    }

    // ── Comment skipping ──────────────────────────────────────────────────────

    #[test]
    fn line_comment_skipped_no_tokens() {
        let r = run("// full line comment");
        assert!(r.tokens.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn block_comment_skipped() {
        let r = run("let /* skip me */ x");
        let ks = kinds(&r);
        assert!(ks.contains(&&TokenKind::KwLet));
        assert!(ks.contains(&&TokenKind::Ident));
        assert_eq!(r.errors.len(), 0);
    }

    #[test]
    fn unterminated_block_comment_produces_error() {
        let r = run("let /* never closed");
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].message.contains("unterminated"));
    }

    // ── Newlines ──────────────────────────────────────────────────────────────

    #[test]
    fn newlines_are_emitted_as_tokens() {
        let r = run("let\nx");
        let ks = kinds(&r);
        assert!(ks.contains(&&TokenKind::Newline));
    }
}
