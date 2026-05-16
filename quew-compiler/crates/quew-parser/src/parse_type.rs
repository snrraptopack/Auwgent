//! Type expression parser.

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::TypeExpr;
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{to_span, type_name, Input, ParseError};

/// Parse a type expression.
///
/// Handles `Named`, `Union` (`A | B | C`), `Optional` (`T?`),
/// and `Generic` (`T<A>`).
///
/// Grammar:
/// ```text
/// type_expr := base_type ( `|` base_type )* `?`?
/// base_type  := ident ( `<` type_expr (`,` type_expr)* `>` )?
/// ```
pub fn type_expr<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, TypeExpr, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // A single named or generic type. Accepts both user-defined `Ident` tokens
    // AND primitive type keywords (`string`, `number`, `bool`, `float`, `void`).
    let base = type_name(source, interner.clone())
        .then(
            type_expr_generic_args(source, interner.clone()).or_not()
        )
        .map_with(|((name, name_span), generic_args), extra| {
            let outer_span = to_span(extra.span());
            match generic_args {
                None => TypeExpr::Named(name, to_span(name_span)),
                Some(args) => TypeExpr::Generic(name, args, outer_span),
            }
        });

    // `base ( | base )*`
    let union_or_single = base.clone()
        .then(
            just(TokenKind::Pipe)
                .ignore_then(base.clone())
                .repeated()
                .collect::<Vec<_>>()
        )
        .map_with(|(first, mut rest), extra| {
            if rest.is_empty() {
                first
            } else {
                let span = to_span(extra.span());
                rest.insert(0, first);
                TypeExpr::Union(rest, span)
            }
        });

    // Optional trailing `?`
    union_or_single
        .then(just(TokenKind::Question).or_not())
        .map_with(|(ty, q), extra| {
            if q.is_some() {
                let span = to_span(extra.span());
                TypeExpr::Optional(Box::new(ty), span)
            } else {
                ty
            }
        })
}

/// Parse `< type_expr (, type_expr)* >` for generic type applications.
fn type_expr_generic_args<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Vec<TypeExpr>, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    // Use type_name() so primitive types work as generic args too (e.g. `List<string>`).
    let arg = type_name(source, interner.clone())
        .map_with(|(name, name_span), _| TypeExpr::Named(name, to_span(name_span)));

    arg.separated_by(just(TokenKind::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LAngle), just(TokenKind::RAngle))
}
