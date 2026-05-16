//! Expression parser using chumsky 0.13's `.pratt()` combinator.

use std::sync::Arc;

use chumsky::prelude::*;
use chumsky::pratt::{infix, left, postfix, prefix, right};
use quew_ast::{
    expr::{
        ArrayExpr, BinaryExpr, BinaryOp, CallExpr, IdentExpr, IsExpr, MemberExpr,
        PostfixIfExpr, Provider, ProviderCall, UnaryExpr, UnaryOp,
    },
    lit::{Lit, StringKind, StringLit},
    Expr,
};
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{
    field_name, ident, int_literal, string_literal, to_span, triple_string,
    CSpan, Input, ParseError,
};
use crate::parse_type::type_expr;

/// Parse any expression.
///
/// Operator precedence (low → high):
/// 1. `and` / `or`                  — left, prec 2
/// 2. `==` / `!=`                   — left, prec 3
/// 3. `+` / `-`                     — left, prec 4
/// 4. `*` / `/` / `%`               — left, prec 5
/// 5. Unary `not`                   — prefix, prec 6
/// 6. Postfix-if `v if c else e`    — postfix, prec 7
/// 7. `is Type`                     — postfix, prec 8
/// 8. Call `()`, Member `.field`    — postfix, prec 9
pub fn expr<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Expr, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    recursive(|expr_rec| {
        // ── Literals ─────────────────────────────────────────────────────────

        let int_lit = int_literal(source)
            .map(|(val, s)| Expr::Lit(Lit::Int(val, to_span(s))));

        let float_lit = just(TokenKind::FloatLiteral)
            .map_with(move |_, extra: &mut _| {
                let s: CSpan = extra.span();
                let f: f64 = source[s.start..s.end].parse().unwrap_or(0.0);
                Expr::Lit(Lit::Float(f, to_span(s)))
            });

        let triple = triple_string(source, interner.clone())
            .map(|(val, s)| Expr::Lit(Lit::String(StringLit {
                value: val,
                kind: StringKind::Triple,
                span: to_span(s),
            })));

        let str_lit = string_literal(source, interner.clone())
            .map(|(val, s)| Expr::Lit(Lit::String(StringLit {
                value: val,
                kind: StringKind::Regular,
                span: to_span(s),
            })));

        let bool_lit = select! {
            TokenKind::True  => true,
            TokenKind::False => false,
        }
        .map_with(|b, extra: &mut _| Expr::Lit(Lit::Bool(b, to_span(extra.span()))));

        let null_lit = just(TokenKind::NullLiteral)
            .map_with(|_, extra: &mut _| Expr::Lit(Lit::Null(to_span(extra.span()))));

        // ── Provider call ─────────────────────────────────────────────────────

        let int_prov = interner.clone();
        let provider_call = select! {
            TokenKind::KwGemini => Provider::Gemini,
            TokenKind::KwOpenAi => Provider::OpenAi,
            TokenKind::KwGroq   => Provider::Groq,
        }
        .then(
            string_literal(source, int_prov)
                .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen))
        )
        .map_with(|(provider, (model_str, model_span)), extra: &mut _| {
            Expr::Provider(ProviderCall {
                provider,
                model_name: StringLit {
                    value: model_str,
                    kind: StringKind::Regular,
                    span: to_span(model_span),
                },
                config: vec![],
                span: to_span(extra.span()),
            })
        });

        // ── Array and grouping ────────────────────────────────────────────────

        let array = expr_rec.clone()
            .separated_by(just(TokenKind::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(TokenKind::LBracket), just(TokenKind::RBracket))
            .map_with(|elements, extra: &mut _| {
                Expr::Array(ArrayExpr { elements, span: to_span(extra.span()) })
            });

        let grouped = expr_rec.clone()
            .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen));

        let ident_expr = ident(source, interner.clone())
            .map_with(|name, extra: &mut _| {
                Expr::Ident(IdentExpr { name, span: to_span(extra.span()) })
            });

        // ── Atom = all leaf forms ─────────────────────────────────────────────
        // Use .or() chains to avoid choice() type-inference issues.
        let atom = float_lit      // float before int (both match digits)
            .or(int_lit)
            .or(triple)           // triple before regular string
            .or(str_lit)
            .or(bool_lit)
            .or(null_lit)
            .or(provider_call)
            .or(array)
            .or(grouped)
            .or(ident_expr);

        // ── Args for call postfix ─────────────────────────────────────────────
        let call_args = expr_rec.clone()
            .separated_by(just(TokenKind::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen));

        // ── Full pratt parser ─────────────────────────────────────────────────
        atom.pratt((
            // Unary `not` — prefix
            prefix(6, just(TokenKind::KwNot), |_op: TokenKind, rhs: Expr, extra: &mut _| {
                Expr::Unary(UnaryExpr {
                    op: UnaryOp::Not,
                    operand: Box::new(rhs),
                    span: to_span(extra.span()),
                })
            }),

            // Multiplicative
            infix(left(5), just(TokenKind::Star),    |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Mul, r, extra.span())),
            infix(left(5), just(TokenKind::Slash),   |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Div, r, extra.span())),
            infix(left(5), just(TokenKind::Percent), |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Mod, r, extra.span())),

            // Additive
            infix(left(4), just(TokenKind::Plus),  |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Add, r, extra.span())),
            infix(left(4), just(TokenKind::Minus), |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Sub, r, extra.span())),

            // Equality
            infix(left(3), just(TokenKind::EqEq),  |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Eq,    r, extra.span())),
            infix(left(3), just(TokenKind::BangEq), |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::NotEq, r, extra.span())),

            // Logical
            infix(left(2), just(TokenKind::KwAnd), |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::And, r, extra.span())),
            infix(left(2), just(TokenKind::KwOr),  |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Or,  r, extra.span())),

            // Assignment — right-associative, lowest binary precedence
            infix(right(1), just(TokenKind::Eq), |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Assign, r, extra.span())),

            // Postfix: call `(args)` — highest postfix precedence
            postfix(9, call_args, |callee: Expr, args: Vec<Expr>, extra: &mut _| {
                Expr::Call(CallExpr {
                    callee: Box::new(callee),
                    args,
                    span: to_span(extra.span()),
                })
            }),

            // Postfix: member `.field` — accepts keywords as field names
            postfix(
                9,
                just(TokenKind::Dot).ignore_then(field_name(source, interner.clone())),
                |obj: Expr, field, extra: &mut _| {
                    Expr::Member(MemberExpr {
                        object: Box::new(obj),
                        field,
                        span: to_span(extra.span()),
                    })
                },
            ),

            // Postfix: `is Type`
            postfix(
                8,
                just(TokenKind::KwIs).ignore_then(type_expr(source, interner.clone())),
                |val: Expr, ty, extra: &mut _| {
                    Expr::Is(IsExpr {
                        value: Box::new(val),
                        ty,
                        span: to_span(extra.span()),
                    })
                },
            ),

            // Postfix-if: `expr if cond else other` — lowest postfix precedence
            postfix(
                7,
                just(TokenKind::KwIf)
                    .ignore_then(expr_rec.clone())
                    .then_ignore(just(TokenKind::KwElse))
                    .then(expr_rec.clone()),
                |val: Expr, (cond, else_val): (Expr, Expr), extra: &mut _| {
                    Expr::PostfixIf(PostfixIfExpr {
                        value:      Box::new(val),
                        condition:  Box::new(cond),
                        else_value: Box::new(else_val),
                        span:       to_span(extra.span()),
                    })
                },
            ),
        ))
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bin(l: Expr, op: BinaryOp, r: Expr, s: CSpan) -> Expr {
    Expr::Binary(BinaryExpr {
        left:  Box::new(l),
        op,
        right: Box::new(r),
        span:  to_span(s),
    })
}
