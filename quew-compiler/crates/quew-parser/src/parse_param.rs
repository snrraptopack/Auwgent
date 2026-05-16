//! Parameter list parser — `(name: Type, @bound: Type, name?: Type)`.

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::{Param, ParamBinding};
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{annotation, ident, to_span, Input, ParseError};
use crate::parse_type::type_expr;

/// Parse a single parameter.
///
/// Three forms:
/// - `name: Type`        — normal
/// - `name?: Type`       — optional normal
/// - `@name: Type`       — binding reference (ties to `@tool` arg)
pub fn param<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Param, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // Clone upfront so each sub-parser gets its own handle.
    let int_bound = interner.clone();
    let int_normal = interner.clone();
    let int_ty_bound = interner.clone();

    // BoundRef: `@name: Type`
    let bound_ref = annotation()
        .then_ignore(just(TokenKind::Colon))
        .then(type_expr(source, int_ty_bound))
        .map_with(move |((_kind, ann_span), ty), extra| {
            let s = ann_span;
            let raw = &source[s.start..s.end]; // e.g. "@id"
            let name = int_bound.intern(&raw[1..]); // strip `@`
            Param {
                binding: ParamBinding::BoundRef,
                name,
                ty,
                optional: false,
                span: to_span(extra.span()),
            }
        });

    // Normal: `name[?]: Type`
    let int_ty_normal = int_normal.clone();
    let normal = ident(source, int_normal)
        .then(just(TokenKind::Question).or_not())
        .then_ignore(just(TokenKind::Colon))
        .then(type_expr(source, int_ty_normal))
        .map_with(|((name, question), ty), extra| Param {
            binding: ParamBinding::Normal,
            name,
            ty,
            optional: question.is_some(),
            span: to_span(extra.span()),
        });

    bound_ref.or(normal)
}

/// Parse a comma-separated parameter list inside `( )`.
pub fn param_list<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Vec<Param>, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    param(source, interner)
        .separated_by(just(TokenKind::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen))
}
