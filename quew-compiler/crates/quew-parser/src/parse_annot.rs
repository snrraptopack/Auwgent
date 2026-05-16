//! Annotation parser — `@tool`, `@desc "..."`, `@context(Type)`, `@tool(id: string)`, etc.
//!
//! In chumsky 0.13 there is no `then_with`. We instead try each possible arg
//! form as an `Option`, then select the right one in `map_with` based on `kind`.
//!
//! Arg-form priority (tried via `.or()` — first successful match wins):
//! 1. Param list `(name: Type, ...)` — for `@tool`
//! 2. Type in parens `(TypeName)`    — for `@context`
//! 3. String literal `"..."`         — for `@desc`
//! 4. Nothing                        — all other annotations

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::lit::{StringKind, StringLit};
use quew_ast::{Annotation, AnnotationArgs};
use quew_interner::Interner;
use quew_lexer::{AnnotationKind, TokenKind};

use crate::common::{Input, ParseError, annotation, string_literal, to_span};
use crate::parse_param::param;
use crate::parse_type::type_expr;

/// Parse a single `@annotation` with its arguments.
pub fn annotation_parser<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Annotation, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // ── Candidate arg forms ───────────────────────────────────────────────────

    // 1. Param list `(name: Type, ...)` — for `@tool(id: string)`
    let param_list = param(source, interner.clone())
        .separated_by(just(TokenKind::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen))
        .map(AnnotationArgs::Params);

    // 2. Type in parens `(TypeName)` — for `@context(MyContext)`
    let type_in_parens = type_expr(source, interner.clone())
        .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen))
        .map(AnnotationArgs::Type);

    // 3. String literal — for `@desc "..."`
    let string_arg = string_literal(source, interner.clone()).map(|(val, s)| {
        AnnotationArgs::String(StringLit {
            value: val,
            kind: StringKind::Regular,
            span: to_span(s),
        })
    });

    // Try each in priority order; default to None if nothing matches.
    //
    // `param_list` must come before `type_in_parens` — both start with `(`,
    // and a param list is more specific (requires `name: Type` inside).
    let opt_args = param_list
        .or(type_in_parens)
        .or(string_arg)
        .or(empty().to(AnnotationArgs::None));

    annotation()
        .then(opt_args)
        .map_with(|((kind, ann_span), args), _extra| {
            // For annotations that don't accept args, discard any spurious match.
            let args = match kind {
                AnnotationKind::Tool => args,    // keeps Params
                AnnotationKind::Context => args, // keeps Type
                AnnotationKind::Desc => args,    // keeps String
                _ => AnnotationArgs::None,
            };
            Annotation {
                kind,
                args,
                span: to_span(ann_span),
            }
        })
}

/// Parse zero or more annotations that precede a declaration.
/// Each annotation may be followed by newlines (multi-line annotation blocks are valid).
pub fn annotations<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Vec<Annotation>, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // Consume optional trailing newlines after each annotation so that:
    //   @tool(id: string)
    //   @desc "..."
    //   function Foo(...) { ... }
    // all parse as one item with two annotations.
    annotation_parser(source, interner)
        .then_ignore(just(TokenKind::Newline).repeated().ignored())
        .repeated()
        .collect()
}
