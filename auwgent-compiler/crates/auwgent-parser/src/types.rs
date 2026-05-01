//! Type expression and type config declaration parsers.

use auwgent_ast::*;
use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

use crate::primitives::*;

pub(crate) fn type_expr_parser(
) -> impl Parser<TokenKind, TypeExpr, Error = Simple<TokenKind>> + Clone {
    recursive(|ty| {
        let string_t =
            tok(TokenKind::StringType).map_with_span(|_, span| TypeExpr::String(s(span)));
        let number_t =
            tok(TokenKind::NumberType).map_with_span(|_, span| TypeExpr::Number(s(span)));
        let bool_t =
            tok(TokenKind::BooleanType).map_with_span(|_, span| TypeExpr::Boolean(s(span)));
        let text_t = tok(TokenKind::TextType).map_with_span(|_, span| TypeExpr::Text(s(span)));

        let type_ref = ident().map(TypeExpr::TypeRef);

        // Object type: { name: type, ... }
        let prop_type = property_name()
            .then(tok(TokenKind::Question).or_not())
            .then_ignore(tok(TokenKind::Colon))
            .then(ty.clone())
            .then(tok(TokenKind::AtDesc).ignore_then(string_lit()).or_not())
            .map_with_span(|(((name, opt), t), desc), span| PropertyType {
                name,
                optional: opt.is_some(),
                ty: t,
                description: desc,
                span: s(span),
            });

        let object_type = prop_type
            .then_ignore(tok(TokenKind::Comma).or_not())
            .repeated()
            .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
            .map_with_span(|props, span| TypeExpr::Object {
                properties: props,
                span: s(span),
            });

        let base = choice((string_t, number_t, bool_t, text_t, object_type, type_ref));

        // Union: "optA" | "optB" | ...
        let union_option = string_lit();
        let union_type = union_option
            .clone()
            .then(tok(TokenKind::Pipe).ignore_then(union_option).repeated())
            .map_with_span(|(first, rest), span| {
                let mut options = vec![first];
                options.extend(rest);
                TypeExpr::Union {
                    options,
                    span: s(span),
                }
            });

        let base_or_union = union_type.or(base);

        // Array suffix: type[]
        base_or_union
            .then(
                tok(TokenKind::LBracket)
                    .then(tok(TokenKind::RBracket))
                    .or_not(),
            )
            .map_with_span(|(ty, arr), span| {
                if arr.is_some() {
                    TypeExpr::Array {
                        element: Box::new(ty),
                        span: s(span),
                    }
                } else {
                    ty
                }
            })
    })
    .boxed()
}

pub(crate) fn type_config_decl_parser(
) -> impl Parser<TokenKind, TypeConfigDecl, Error = Simple<TokenKind>> + Clone {
    property_name()
        .then(tok(TokenKind::Question).or_not())
        .then_ignore(tok(TokenKind::Colon))
        .then(type_expr_parser())
        .then(tok(TokenKind::AtDesc).ignore_then(string_lit()).or_not())
        .map_with_span(|(((name, opt), ty), desc), span| TypeConfigDecl {
            name,
            optional: opt.is_some(),
            ty,
            description: desc,
            span: s(span),
        })
}

pub(crate) fn type_config_decl_block_parser(
) -> impl Parser<TokenKind, Vec<TypeConfigDecl>, Error = Simple<TokenKind>> + Clone {
    type_config_decl_parser()
        .then_ignore(tok(TokenKind::Comma).or_not())
        .repeated()
}
