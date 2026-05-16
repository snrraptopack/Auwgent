//! Shared type aliases and primitive combinators for the quew parser.
//!
//! Every parser module imports from here. Nothing in this file depends on
//! any specific grammar rule — it only deals with the token stream shape
//! and common leaf parsers (identifier, string literal, integer, float).

use std::sync::Arc;

use chumsky::input::{Stream, ValueInput};
use chumsky::prelude::*;
use quew_errors::Span;
use quew_interner::{InternedStr, Interner};
use quew_lexer::{AnnotationKind, TokenKind};

// ── Type aliases ──────────────────────────────────────────────────────────────

/// The span type chumsky works with internally.
/// We convert to/from `quew_errors::Span` at the boundary.
pub type CSpan = SimpleSpan<usize>;

/// The error type produced by every combinator in this crate.
pub type ParseError<'tok> = extra::Err<Rich<'tok, TokenKind>>;

/// The input type every combinator is generic over.
///
/// This is a constraint alias — combinators are written:
/// ```ignore
/// fn my_parser<'tok, I>() -> impl Parser<'tok, I, Output, ParseError<'tok>>
/// where I: Input<'tok>
/// ```
pub trait Input<'tok>: ValueInput<'tok, Token = TokenKind, Span = CSpan> {}

impl<'tok, T> Input<'tok> for T where T: ValueInput<'tok, Token = TokenKind, Span = CSpan> {}

// ── Span conversion ───────────────────────────────────────────────────────────

/// Convert a chumsky `SimpleSpan` to a `quew_errors::Span`.
#[inline]
pub fn to_span(cs: CSpan) -> Span {
    Span::new(cs.start, cs.end)
}

// ── Leaf combinators ──────────────────────────────────────────────────────────

/// Parse an `Ident` token and intern its source text.
///
/// `source` must be the full source string that was originally lexed — the
/// span on the token is used to slice it and intern the result.
pub fn ident<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, InternedStr, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::Ident).map_with(move |_, extra| {
        let s: CSpan = extra.span();
        interner.intern(&source[s.start..s.end])
    })
}

/// Parse any keyword or identifier as a field name (used after `.`).
///
/// After a `.`, any keyword is a valid field name. `config.model`, `response.is`,
/// `obj.for` must all parse without error. This combinator accepts ANY token
/// whose slice can be a valid name and interns the slice.
pub fn field_name<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, InternedStr, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // Accept Ident OR any keyword/provider token by matching on the slice.
    // We use `select!` to enumerate every possible field-name token.
    select! {
        TokenKind::Ident         => (),
        // Statement keywords
        TokenKind::KwAgent       => (),
        TokenKind::KwFunction    => (),
        TokenKind::KwTool        => (),
        TokenKind::KwTools       => (),
        TokenKind::KwType        => (),
        TokenKind::KwModel       => (),
        TokenKind::KwLet         => (),
        TokenKind::KwIf          => (),
        TokenKind::KwElse        => (),
        TokenKind::KwReturn      => (),
        TokenKind::KwReply       => (),
        TokenKind::KwWith        => (),
        TokenKind::KwFor         => (),
        TokenKind::KwIn          => (),
        TokenKind::KwIs          => (),
        TokenKind::KwAnd         => (),
        TokenKind::KwOr          => (),
        TokenKind::KwNot         => (),
        // Type keywords
        TokenKind::TyString      => (),
        TokenKind::TyNumber      => (),
        TokenKind::TyFloat       => (),
        TokenKind::TyBool        => (),
        TokenKind::TyVoid        => (),
        // Provider keywords
        TokenKind::KwGemini      => (),
        TokenKind::KwOpenAi      => (),
        TokenKind::KwGroq        => (),
    }
    .map_with(move |_, extra| {
        let s: CSpan = extra.span();
        interner.intern(&source[s.start..s.end])
    })
}

/// Parse a type-name token: user-defined `Ident` OR any primitive type keyword
/// (`string`, `number`, `float`, `bool`, `void`).
///
/// This is used by `type_expr` so that `name: string` and `name: MyType` both
/// work without special-casing primitives in the type parser.
pub fn type_name<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, (InternedStr, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    select! {
        TokenKind::Ident    => (),
        TokenKind::TyString => (),
        TokenKind::TyNumber => (),
        TokenKind::TyFloat  => (),
        TokenKind::TyBool   => (),
        TokenKind::TyVoid   => (),
    }
    .map_with(move |_, extra| {
        let s: CSpan = extra.span();
        (interner.intern(&source[s.start..s.end]), s)
    })
}

/// Parse a single-quoted string literal and intern its content (strips quotes).
pub fn string_literal<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, (InternedStr, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::StringLiteral).map_with(move |_, extra| {
        let s: CSpan = extra.span();
        let raw = &source[s.start..s.end]; // includes surrounding `"`
        let content = &raw[1..raw.len() - 1]; // strip quotes
        (interner.intern(content), s)
    })
}

/// Parse a triple-quoted string literal and intern its content (strips `"""`).
pub fn triple_string<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, (InternedStr, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::TripleString).map_with(move |_, extra| {
        let s: CSpan = extra.span();
        let raw = &source[s.start..s.end]; // includes surrounding `"""`
        let content = &raw[3..raw.len() - 3]; // strip triple quotes
        (interner.intern(content), s)
    })
}

/// Parse an integer literal and return its value.
pub fn int_literal<'tok, I>(
    source: &'tok str,
) -> impl Parser<'tok, I, (i64, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::IntLiteral).map_with(move |_, extra| {
        let s: CSpan = extra.span();
        let val: i64 = source[s.start..s.end].parse().unwrap_or(0);
        (val, s)
    })
}

/// Parse a float literal and return its raw text interned (avoids f64 Hash).
/// The AST builder will parse to f64 via `str::parse`.
#[allow(dead_code)] // public helper; used by callers and future passes
pub fn float_literal<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, (InternedStr, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::FloatLiteral).map_with(move |_, extra| {
        let s: CSpan = extra.span();
        (interner.intern(&source[s.start..s.end]), s)
    })
}

/// Parse an annotation token and return its `AnnotationKind` plus span.
pub fn annotation<'tok, I>()
-> impl Parser<'tok, I, (AnnotationKind, CSpan), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    select! { TokenKind::Annotation(k) => k }.map_with(|k, extra| (k, extra.span()))
}

/// Skip any number of `Newline` tokens — useful between items in a block.
#[allow(dead_code)] // public helper; available for callers that need newline-awareness
pub fn newlines<'tok, I>() -> impl Parser<'tok, I, (), ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    just(TokenKind::Newline).repeated().ignored()
}

// ── Stream builder ────────────────────────────────────────────────────────────

/// Convert a `LexResult` token list into a chumsky `Stream`.
///
/// Newlines are kept in the stream; parsers that don't care skip them via
/// the `newlines()` combinator. Items that need newline significance (e.g.
/// statement separation) can consume them explicitly.
pub fn make_stream(
    tokens: &[(TokenKind, Span)],
    source_len: usize,
) -> impl ValueInput<'_, Token = TokenKind, Span = CSpan> {
    // Bring chumsky's Input trait into scope for Stream::map()
    // without conflicting with our own `Input` trait alias.
    use chumsky::input::Input as _;

    let iter = tokens
        .iter()
        .map(|(tok, span)| (tok.clone(), CSpan::from(span.start..span.end)));
    let eoi = CSpan::from(source_len..source_len);
    Stream::from_iter(iter).map(eoi, |(t, s)| (t, s))
}
