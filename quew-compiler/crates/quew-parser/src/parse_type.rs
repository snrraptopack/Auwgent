//! Type expression parser.

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::TypeExpr;
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{Input, ParseError, to_span, type_name};

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
    recursive(|ty| {
        // A single named or generic type. Accepts both user-defined `Ident`
        // tokens and primitive type keywords (`string`, `number`, `bool`, ...).
        let generic_args = ty
            .clone()
            .separated_by(just(TokenKind::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(TokenKind::LAngle), just(TokenKind::RAngle));

        let base = type_name(source, interner.clone())
            .then(generic_args.or_not())
            .map_with(|((name, name_span), generic_args), extra| {
                let outer_span = to_span(extra.span());
                match generic_args {
                    None => TypeExpr::Named(name, to_span(name_span)),
                    Some(args) => TypeExpr::Generic(name, args, outer_span),
                }
            });

        let union_or_single = base
            .clone()
            .then(
                just(TokenKind::Pipe)
                    .ignore_then(base.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
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
    })
}
