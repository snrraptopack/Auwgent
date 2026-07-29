//! Expression parser using chumsky 0.13's `.pratt()` combinator.

use std::sync::Arc;

use chumsky::pratt::{infix, left, postfix, prefix, right};
use chumsky::prelude::*;
use quew_ast::{
    Expr,
    expr::{
        ArrayExpr, BinaryExpr, BinaryOp, CallExpr, IdentExpr, InterpolatedSegment,
        InterpolatedString, IsExpr, MemberExpr, ObjectExpr, ObjectField, PostfixIfExpr, Provider,
        ProviderCall, UnaryExpr, UnaryOp,
    },
    lit::{Lit, StringKind, StringLit},
};
use quew_errors::{Diagnostic, Span};
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{
    CSpan, Input, ParseError, field_name, ident, int_literal, newlines, string_literal, to_span,
    triple_string,
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

        let int_lit = int_literal(source).map(|(val, s)| Expr::Lit(Lit::Int(val, to_span(s))));

        let float_lit = just(TokenKind::FloatLiteral).map_with(move |_, extra: &mut _| {
            let s: CSpan = extra.span();
            let f: f64 = source[s.start..s.end].parse().unwrap_or(0.0);
            Expr::Lit(Lit::Float(f, to_span(s)))
        });

        let int_interp = interner.clone();
        let triple =
            triple_string(source, interner.clone()).map_with(move |(val, s), _extra: &mut _| {
                let raw = &source[s.start..s.end];
                let content = &raw[3..raw.len().saturating_sub(3)];
                if has_interpolation(content) {
                    build_interpolated(content, s.start + 3, &int_interp)
                } else {
                    Expr::Lit(Lit::String(StringLit {
                        value: val,
                        kind: StringKind::Triple,
                        span: to_span(s),
                    }))
                }
            });

        let int_interp2 = interner.clone();
        let str_lit =
            string_literal(source, interner.clone()).map_with(move |(val, s), _extra: &mut _| {
                let raw = &source[s.start..s.end];
                let content = &raw[1..raw.len().saturating_sub(1)];
                if has_interpolation(content) {
                    build_interpolated(content, s.start + 1, &int_interp2)
                } else {
                    Expr::Lit(Lit::String(StringLit {
                        value: val,
                        kind: StringKind::Regular,
                        span: to_span(s),
                    }))
                }
            });

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
                .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen)),
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

        // ── Array, object, and grouping ───────────────────────────────────────

        let expr_sep = just(TokenKind::Comma)
            .or(just(TokenKind::Newline))
            .repeated()
            .at_least(1)
            .ignored();

        let array = expr_rec
            .clone()
            .separated_by(expr_sep.clone())
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(
                just(TokenKind::LBracket).then_ignore(newlines()),
                newlines().ignore_then(just(TokenKind::RBracket)),
            )
            .map_with(|elements, extra: &mut _| {
                Expr::Array(ArrayExpr {
                    elements,
                    span: to_span(extra.span()),
                })
            });

        let object_key =
            ident(source, interner.clone())
                .or(string_literal(source, interner.clone()).map(|(name, _)| name));

        let object_field = object_key
            .then_ignore(just(TokenKind::Colon))
            .then(expr_rec.clone())
            .map_with(|(name, value), extra: &mut _| ObjectField {
                name,
                value: Box::new(value),
                span: to_span(extra.span()),
            });

        let object = object_field
            .separated_by(expr_sep)
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(
                just(TokenKind::LBrace).then_ignore(newlines()),
                newlines().ignore_then(just(TokenKind::RBrace)),
            )
            .map_with(|fields, extra: &mut _| {
                Expr::Object(ObjectExpr {
                    fields,
                    span: to_span(extra.span()),
                })
            });

        let grouped = expr_rec
            .clone()
            .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen));

        let ident_expr = ident(source, interner.clone()).map_with(|name, extra: &mut _| {
            Expr::Ident(IdentExpr {
                name,
                span: to_span(extra.span()),
            })
        });

        // ── Atom = all leaf forms ─────────────────────────────────────────────
        // Use .or() chains to avoid choice() type-inference issues.
        let atom = float_lit // float before int (both match digits)
            .or(int_lit)
            .or(triple) // triple before regular string
            .or(str_lit)
            .or(bool_lit)
            .or(null_lit)
            .or(provider_call)
            .or(array)
            .or(object)
            .or(grouped)
            .or(ident_expr);

        // ── Args for call postfix ─────────────────────────────────────────────
        let call_args = expr_rec
            .clone()
            .separated_by(just(TokenKind::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen));

        // ── Full pratt parser ─────────────────────────────────────────────────
        atom.pratt((
            // Unary `not` — prefix
            prefix(
                6,
                just(TokenKind::KwNot),
                |_op: TokenKind, rhs: Expr, extra: &mut _| {
                    Expr::Unary(UnaryExpr {
                        op: UnaryOp::Not,
                        operand: Box::new(rhs),
                        span: to_span(extra.span()),
                    })
                },
            ),
            // Multiplicative
            infix(
                left(5),
                just(TokenKind::Star),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Mul, r, extra.span()),
            ),
            infix(
                left(5),
                just(TokenKind::Slash),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Div, r, extra.span()),
            ),
            infix(
                left(5),
                just(TokenKind::Percent),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Mod, r, extra.span()),
            ),
            // Additive
            infix(
                left(4),
                just(TokenKind::Plus),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Add, r, extra.span()),
            ),
            infix(
                left(4),
                just(TokenKind::Minus),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Sub, r, extra.span()),
            ),
            // Comparison
            infix(
                left(3),
                just(TokenKind::LAngle),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Lt, r, extra.span()),
            ),
            infix(
                left(3),
                just(TokenKind::LtEq),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Lte, r, extra.span()),
            ),
            infix(
                left(3),
                just(TokenKind::RAngle),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Gt, r, extra.span()),
            ),
            infix(
                left(3),
                just(TokenKind::GtEq),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Gte, r, extra.span()),
            ),
            // Equality
            infix(
                left(3),
                just(TokenKind::EqEq),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Eq, r, extra.span()),
            ),
            infix(
                left(3),
                just(TokenKind::BangEq),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::NotEq, r, extra.span()),
            ),
            // Logical
            infix(
                left(2),
                just(TokenKind::KwAnd),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::And, r, extra.span()),
            ),
            infix(
                left(2),
                just(TokenKind::KwOr),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Or, r, extra.span()),
            ),
            // Assignment — right-associative, lowest binary precedence
            infix(
                right(1),
                just(TokenKind::Eq),
                |l, _op: TokenKind, r, extra: &mut _| bin(l, BinaryOp::Assign, r, extra.span()),
            ),
            // Postfix: call `(args)` — highest postfix precedence
            postfix(
                9,
                call_args,
                |callee: Expr, args: Vec<Expr>, extra: &mut _| {
                    Expr::Call(CallExpr {
                        callee: Box::new(callee),
                        args,
                        span: to_span(extra.span()),
                    })
                },
            ),
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
                        value: Box::new(val),
                        condition: Box::new(cond),
                        else_value: Box::new(else_val),
                        span: to_span(extra.span()),
                    })
                },
            ),
        ))
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bin(l: Expr, op: BinaryOp, r: Expr, s: CSpan) -> Expr {
    Expr::Binary(BinaryExpr {
        left: Box::new(l),
        op,
        right: Box::new(r),
        span: to_span(s),
    })
}

/// Returns `true` if the string content contains an unescaped `{`.
fn has_interpolation(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2; // escaped {{ → skip both
            } else {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// Build an `Expr::Interpolated` from string content that contains `{expr}` segments.
fn build_interpolated(content: &str, offset: usize, interner: &Arc<Interner>) -> Expr {
    let mut segments = Vec::new();
    let mut i = 0;
    let mut text_start = 0;
    let bytes = content.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                // Escaped brace — skip both, they'll be unescaped in post-processing.
                i += 2;
                continue;
            }

            // Interpolation start
            if i > text_start {
                let text = unescape_braces(&content[text_start..i]);
                if !text.is_empty() {
                    segments.push(InterpolatedSegment::Text(text));
                }
            }

            let expr_start = i + 1;
            let mut depth = 1;
            let mut in_string = false;
            let mut j = expr_start;
            while j < bytes.len() && depth > 0 {
                let c = bytes[j];
                if !in_string {
                    if c == b'"' {
                        in_string = true;
                    } else if c == b'{' {
                        depth += 1;
                    } else if c == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                } else {
                    if c == b'\\' && j + 1 < bytes.len() {
                        j += 1; // skip escaped char
                    } else if c == b'"' {
                        in_string = false;
                    }
                }
                j += 1;
            }

            if depth == 0 {
                let expr_text = &content[expr_start..j];
                match parse_expr_str(expr_text, interner) {
                    Ok(expr) => {
                        segments.push(InterpolatedSegment::Expr(Box::new(expr)));
                    }
                    Err(_) => {
                        // Fallback: treat as literal text on parse error.
                        segments.push(InterpolatedSegment::Text(format!("{{{expr_text}}}")));
                    }
                }
                i = j + 1;
                text_start = i;
            } else {
                // Unterminated interpolation — consume rest as text.
                let text = unescape_braces(&content[i..]);
                if !text.is_empty() {
                    segments.push(InterpolatedSegment::Text(text));
                }
                break;
            }
        } else {
            i += 1;
        }
    }

    if text_start < content.len() {
        let text = unescape_braces(&content[text_start..]);
        if !text.is_empty() {
            segments.push(InterpolatedSegment::Text(text));
        }
    }

    Expr::Interpolated(InterpolatedString {
        segments,
        span: Span::new(offset, offset + content.len()),
    })
}

/// Replace `{{` with `{` and `}}` with `}` in a text fragment.
fn unescape_braces(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            result.push('{');
            i += 2;
        } else if bytes[i] == b'}' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
            result.push('}');
            i += 2;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

/// Parse a single expression from a standalone string.
/// Used by string interpolation to parse `{expr}` segments.
pub fn parse_expr_str(source: &str, interner: &Arc<Interner>) -> Result<Expr, Vec<Diagnostic>> {
    let lex_result = quew_lexer::lex(source, quew_source::SourceId::dummy(), interner);
    let stream = crate::common::make_stream(&lex_result.tokens, source.len());
    let (expr, errs) = expr(source, Arc::clone(interner))
        .parse(stream)
        .into_output_errors();

    let mut errors: Vec<Diagnostic> = errs
        .into_iter()
        .map(|err| {
            let span = Span::new(err.span().start, err.span().end);
            Diagnostic::error(format!("{}", err.reason()), span)
        })
        .collect();

    errors.extend(lex_result.errors);

    expr.ok_or(errors)
}
