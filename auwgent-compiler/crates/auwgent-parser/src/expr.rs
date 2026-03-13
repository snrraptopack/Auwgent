//! Expression and condition parsers.

use auwgent_ast::*;
use auwgent_errors::Span;
use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

use crate::primitives::*;

// ── Expression Parser ────────────────────────────────────────────────────

pub(crate) fn object_literal_parser(
) -> impl Parser<TokenKind, ObjectLiteral, Error = Simple<TokenKind>> + Clone {
    let obj_prop = property_name()
        .then(tok(TokenKind::Colon).ignore_then(expr_parser()).or_not())
        .map_with_span(|(name, value), span| PropertyValue {
            name,
            value,
            span: s(span),
        });

    obj_prop
        .separated_by(tok(TokenKind::Comma))
        .allow_trailing()
        .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
        .map_with_span(|properties, span| ObjectLiteral {
            properties,
            span: s(span),
        })
}

pub(crate) fn expr_parser() -> impl Parser<TokenKind, Expr, Error = Simple<TokenKind>> + Clone {
    recursive(|expr: Recursive<'_, TokenKind, Expr, Simple<TokenKind>>| {
        let args = expr
            .clone()
            .separated_by(tok(TokenKind::Comma))
            .allow_trailing()
            .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen));

        // String literal
        let str_lit = string_lit().map(Expr::StringLit);
        let ml_str = multiline_string().map(Expr::MultilineStringLit);

        // Number literal
        let num_lit = number_lit().map(Expr::NumberLit);

        // Boolean literal
        let bool_lit = tok(TokenKind::True)
            .map_with_span(|_, span| Expr::BooleanLit(sp(true, span)))
            .or(tok(TokenKind::False).map_with_span(|_, span| Expr::BooleanLit(sp(false, span))));

        // Array literal: [expr, ...]
        let array_lit = expr
            .clone()
            .separated_by(tok(TokenKind::Comma))
            .allow_trailing()
            .delimited_by(tok(TokenKind::LBracket), tok(TokenKind::RBracket))
            .map_with_span(|elements, span| {
                Expr::Array(ArrayLiteral {
                    elements,
                    span: s(span),
                })
            });

        // Object literal: { name: expr, ... }
        let obj_prop = property_name()
            .then(tok(TokenKind::Colon).ignore_then(expr.clone()).or_not())
            .map_with_span(|(name, value), span| PropertyValue {
                name,
                value,
                span: s(span),
            });

        let object_lit = obj_prop
            .separated_by(tok(TokenKind::Comma))
            .allow_trailing()
            .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
            .map_with_span(|properties, span| {
                Expr::Object(ObjectLiteral {
                    properties,
                    span: s(span),
                })
            });

        // Inline prompt block: { "text" someExpr }
        // Keep object literal precedence so { name: value } and { name } stay objects.
        let inline_prompt = expr
            .clone()
            .repeated()
            .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace))
            .map_with_span(|parts, span| {
                Expr::InlinePrompt(InlinePromptBlock {
                    parts: parts.into_iter().map(PromptStatement::Expr).collect(),
                    span: s(span),
                })
            });

        // Context ref: ctx.property
        let ctx_ref = tok(TokenKind::Ctx)
            .ignore_then(tok(TokenKind::Dot))
            .ignore_then(property_name())
            .map_with_span(|prop, span| {
                Expr::ContextRef(ContextRef {
                    property: prop,
                    span: s(span),
                })
            });

        // Helper call: hlp.name(args)
        let hlp_call = tok(TokenKind::Hlp)
            .ignore_then(tok(TokenKind::Dot))
            .ignore_then(ident())
            .then(args.clone())
            .map_with_span(|(helper, args), span| {
                Expr::HelperCall(HelperCall {
                    helper,
                    args,
                    span: s(span),
                })
            });

        // Grouped: (expr)
        let grouped = expr
            .clone()
            .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen))
            .map_with_span(|e, span| Expr::Grouped(Box::new(e), s(span)));

        // Ident-based expressions: func_call, member_access, index_access, var_ref
        let dot_chain = tok(TokenKind::Dot).ignore_then(property_name()).repeated();

        let ident_expr = ident()
            .then(choice((
                // Function call: ident(args)
                args.clone().map(IdentSuffix::Call),
                // Index access: ident[expr](.props)*
                expr.clone()
                    .delimited_by(tok(TokenKind::LBracket), tok(TokenKind::RBracket))
                    .then(dot_chain.clone())
                    .map(|(idx, chain)| IdentSuffix::Index(idx, chain)),
                // Member access: ident.prop(.prop)*
                dot_chain.clone().at_least(1).map(IdentSuffix::Member),
                // Plain var ref (empty — no suffix)
                empty().to(IdentSuffix::None),
            )))
            .map_with_span(|(name, suffix), span| match suffix {
                IdentSuffix::Call(args) => Expr::FunctionCall(FunctionCall {
                    name,
                    args,
                    span: s(span),
                }),
                IdentSuffix::Member(chain) => {
                    let prop = chain[0].clone();
                    let rest = chain[1..].to_vec();
                    Expr::MemberAccess(MemberAccess {
                        object: name,
                        property: prop,
                        chain: rest,
                        span: s(span),
                    })
                }
                IdentSuffix::Index(idx, chain) => {
                    let (property, rest) = if chain.is_empty() {
                        (None, vec![])
                    } else {
                        (Some(chain[0].clone()), chain[1..].to_vec())
                    };
                    Expr::IndexAccess(IndexAccess {
                        object: name,
                        index: Box::new(idx),
                        property,
                        chain: rest,
                        span: s(span),
                    })
                }
                IdentSuffix::None => Expr::VarRef(name),
            });

        // All atoms
        let atom = choice((
            ctx_ref, hlp_call, grouped, array_lit, object_lit, inline_prompt, str_lit, ml_str, num_lit, bool_lit,
            ident_expr,
        ));

        // Binary ops: mul/div then add/sub
        let mul_op = choice((
            tok(TokenKind::Star).to(BinOperator::Mul),
            tok(TokenKind::Slash).to(BinOperator::Div),
        ));
        let product = atom
            .clone()
            .then(mul_op.then(atom).repeated())
            .foldl(|left, (op, right)| {
                Expr::BinaryOp(BinaryOp {
                    span: Span::new(0, 0),
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                })
            });

        let add_op = choice((
            tok(TokenKind::Plus).to(BinOperator::Add),
            tok(TokenKind::Minus).to(BinOperator::Sub),
        ));
        product
            .clone()
            .then(add_op.then(product).repeated())
            .foldl(|left, (op, right)| {
                Expr::BinaryOp(BinaryOp {
                    span: Span::new(0, 0),
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                })
            })
    })
}

#[derive(Clone)]
enum IdentSuffix {
    Call(Vec<Expr>),
    Member(Vec<Spanned<String>>),
    Index(Expr, Vec<Spanned<String>>),
    None,
}

// ── Condition Parser ─────────────────────────────────────────────────────

pub(crate) fn condition_parser(
) -> impl Parser<TokenKind, Condition, Error = Simple<TokenKind>> + Clone {
    let expr = expr_parser();
    let cmp_op = choice((
        tok(TokenKind::EqEq).to(ComparisonOp::Eq),
        tok(TokenKind::NotEq).to(ComparisonOp::Neq),
        tok(TokenKind::GtEq).to(ComparisonOp::Gte),
        tok(TokenKind::LtEq).to(ComparisonOp::Lte),
        tok(TokenKind::Gt).to(ComparisonOp::Gt),
        tok(TokenKind::Lt).to(ComparisonOp::Lt),
    ));

    let comparison = expr
        .clone()
        .then(cmp_op.then(expr.clone()).or_not())
        .map_with_span(|(left, op_right), span| match op_right {
            Some((op, right)) => Condition::Comparison {
                left,
                op,
                right,
                span: s(span),
            },
            None => Condition::Boolean {
                value: left,
                span: s(span),
            },
        });

    // Logical ops: comparison (&& ||) comparison
    let logical_op = choice((
        tok(TokenKind::And).to(LogicalOp::And),
        tok(TokenKind::Or).to(LogicalOp::Or),
    ));
    comparison
        .clone()
        .then(logical_op.then(comparison).repeated())
        .foldl(|left, (op, right)| Condition::Logical {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: Span::new(0, 0),
        })
}
