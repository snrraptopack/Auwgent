//! Primitive token-matching combinators used across all parser modules.

use auwgent_ast::Spanned;
use auwgent_errors::Span;
use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

pub(crate) type Spn = std::ops::Range<usize>;

pub(crate) fn s(r: Spn) -> Span {
    Span::new(r.start, r.end)
}

pub(crate) fn sp<T>(value: T, span: Spn) -> Spanned<T> {
    Spanned::new(value, s(span))
}

pub(crate) fn tok(
    kind: TokenKind,
) -> impl Parser<TokenKind, TokenKind, Error = Simple<TokenKind>> + Clone {
    just(kind)
}

pub(crate) fn ident() -> impl Parser<TokenKind, Spanned<String>, Error = Simple<TokenKind>> + Clone
{
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::Ident(s) => Ok(sp(s, span)),
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
    .labelled("identifier")
}

pub(crate) fn string_lit(
) -> impl Parser<TokenKind, Spanned<String>, Error = Simple<TokenKind>> + Clone {
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::DoubleString(s) | TokenKind::SingleString(s) => Ok(sp(s, span)),
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
    .labelled("string")
}

pub(crate) fn any_string(
) -> impl Parser<TokenKind, Spanned<String>, Error = Simple<TokenKind>> + Clone {
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::DoubleString(s) | TokenKind::SingleString(s) | TokenKind::MultilineString(s) => {
            Ok(sp(s, span))
        }
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
    .labelled("string")
}

pub(crate) fn multiline_string(
) -> impl Parser<TokenKind, Spanned<String>, Error = Simple<TokenKind>> + Clone {
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::MultilineString(s) => Ok(sp(s, span)),
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
}

pub(crate) fn number_lit() -> impl Parser<TokenKind, Spanned<f64>, Error = Simple<TokenKind>> + Clone
{
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::Number(s) => Ok(sp(s.parse::<f64>().unwrap_or(0.0), span)),
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
    .labelled("number")
}

pub(crate) fn number_int() -> impl Parser<TokenKind, Spanned<i64>, Error = Simple<TokenKind>> + Clone
{
    filter_map(|span: Spn, tok: TokenKind| match tok {
        TokenKind::Number(s) => Ok(sp(s.parse::<i64>().unwrap_or(0), span)),
        _ => Err(Simple::expected_input_found(span, vec![], Some(tok))),
    })
}

/// Matches ident or keyword that can serve as a property name.
pub(crate) fn property_name(
) -> impl Parser<TokenKind, Spanned<String>, Error = Simple<TokenKind>> + Clone {
    ident().or(filter_map(|span: Spn, tok: TokenKind| {
        let name = match &tok {
            TokenKind::Model => "model",
            TokenKind::Prompt => "prompt",
            TokenKind::Config => "config",
            TokenKind::Input => "input",
            TokenKind::Output => "output",
            TokenKind::Context => "context",
            TokenKind::Description => "description",
            TokenKind::Error => "error",
            TokenKind::Provider => "provider",
            TokenKind::MaxTokens => "maxTokens",
            TokenKind::MaxMessages => "maxMessages",
            _ => return Err(Simple::expected_input_found(span, vec![], Some(tok))),
        };
        Ok(sp(name.to_string(), span))
    }))
}
