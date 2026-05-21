//! Statement parser.

use std::sync::Arc;

use chumsky::prelude::*;
use quew_ast::{
    Stmt,
    stmt::{
        ElseClause, ExprStmt, ForStmt, IfStmt, LetStmt, ReplyStmt, ReturnMode, ReturnStmt,
        WhileStmt, WithBlock, WithField,
    },
};
use quew_interner::Interner;
use quew_lexer::TokenKind;

use crate::common::{Input, ParseError, field_name, ident, to_span};
use crate::parse_expr::expr;
use crate::parse_type::type_expr;

/// Parse a block body: `{ stmt* }`.
pub fn block<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Vec<Stmt>, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    stmt(source, interner)
        .separated_by(just(TokenKind::Newline).repeated().at_least(1))
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace))
}

/// Parse a single statement.
pub fn stmt<'tok, I>(
    source: &'tok str,
    interner: Arc<Interner>,
) -> impl Parser<'tok, I, Stmt, ParseError<'tok>> + Clone
where
    I: Input<'tok>,
{
    recursive(|stmt_rec| {
        let e = expr(source, interner.clone());

        // `let name[: Type] = expr`
        let let_stmt = just(TokenKind::KwLet)
            .ignore_then(ident(source, interner.clone()))
            .then(
                just(TokenKind::Colon)
                    .ignore_then(type_expr(source, interner.clone()))
                    .or_not(),
            )
            .then_ignore(just(TokenKind::Eq))
            .then(e.clone())
            .map_with(|((name, ty), init), extra| {
                Stmt::Let(LetStmt {
                    name,
                    ty,
                    init,
                    span: to_span(extra.span()),
                })
            });

        // `if cond { body } [else { body } | else if ...]`
        let if_stmt = {
            let es = e.clone();
            let block_inner = stmt_rec
                .clone()
                .separated_by(just(TokenKind::Newline).repeated().at_least(1))
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

            recursive(move |if_rec| {
                just(TokenKind::KwIf)
                    .ignore_then(es.clone())
                    .then(block_inner.clone())
                    .then(
                        just(TokenKind::KwElse)
                            .ignore_then(choice((
                                // else if ...
                                if_rec.map(|s| ElseClause::ElseIf(Box::new(s))),
                                // else { ... }
                                block_inner.clone().map_with(|body, extra| {
                                    ElseClause::Else(body, to_span(extra.span()))
                                }),
                            )))
                            .or(empty().to(ElseClause::None)),
                    )
                    .map_with(|((condition, then_body), else_clause), extra| IfStmt {
                        condition,
                        then_body,
                        else_clause,
                        span: to_span(extra.span()),
                    })
            })
            .map(Stmt::If)
        };

        // `return [expr [with turns]]`
        // The optional `with turns` suffix marks a transparent agent handoff —
        // the child's turn trace is merged into the parent's journal context.
        let return_stmt = just(TokenKind::KwReturn)
            .ignore_then(e.clone().or_not())
            .then(
                just(TokenKind::KwWith)
                    .ignore_then(just(TokenKind::KwTurns))
                    .to(ReturnMode::WithTurns)
                    .or(empty().to(ReturnMode::Normal)),
            )
            .map_with(|(value, mode), extra| {
                Stmt::Return(ReturnStmt {
                    value,
                    mode,
                    span: to_span(extra.span()),
                })
            });

        // `reply(expr) with { key: value, ... }`
        // Keywords are valid field names here (e.g. `model:`, `prompt:`, `config:`).
        let with_field = {
            let ef = e.clone();
            field_name(source, interner.clone())
                .then_ignore(just(TokenKind::Colon))
                .then(ef)
                .map_with(|(key, value), extra| WithField {
                    key,
                    value,
                    span: to_span(extra.span()),
                })
        };

        let with_block = with_field
            .separated_by(
                just(TokenKind::Newline)
                    .repeated()
                    .at_least(1)
                    .or(just(TokenKind::Comma).ignored()),
            )
            .allow_leading()
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace))
            .map_with(|fields, extra| WithBlock {
                fields,
                span: to_span(extra.span()),
            });

        let reply_stmt = just(TokenKind::KwReply)
            .ignore_then(
                e.clone()
                    .delimited_by(just(TokenKind::LParen), just(TokenKind::RParen)),
            )
            .then_ignore(just(TokenKind::KwWith))
            .then(with_block)
            .map_with(|(input, with_block), extra| {
                Stmt::Reply(ReplyStmt {
                    input,
                    with_block,
                    span: to_span(extra.span()),
                })
            });

        // `for [idx,] value in expr { body }`
        let for_stmt = {
            let block_inner = stmt_rec
                .clone()
                .separated_by(just(TokenKind::Newline).repeated().at_least(1))
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

            just(TokenKind::KwFor)
                .ignore_then(
                    // optional `idx,` prefix
                    ident(source, interner.clone())
                        .then_ignore(just(TokenKind::Comma))
                        .or_not(),
                )
                .then(ident(source, interner.clone()))
                .then_ignore(just(TokenKind::KwIn))
                .then(e.clone())
                .then(block_inner)
                .map_with(|(((index, value), iterable), body), extra| {
                    Stmt::For(ForStmt {
                        index,
                        value,
                        iterable,
                        body,
                        span: to_span(extra.span()),
                    })
                })
        };

        // `while condition { body }`
        let while_stmt = {
            let block_inner = stmt_rec
                .clone()
                .separated_by(just(TokenKind::Newline).repeated().at_least(1))
                .allow_leading()
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace));

            just(TokenKind::KwWhile)
                .ignore_then(e.clone())
                .then(block_inner)
                .map_with(|(condition, body), extra| {
                    Stmt::While(WhileStmt {
                        condition,
                        body,
                        span: to_span(extra.span()),
                    })
                })
        };

        // `break`
        let break_stmt = just(TokenKind::KwBreak).map_with(|_, extra| {
            Stmt::Break(to_span(extra.span()))
        });

        // `continue`
        let continue_stmt = just(TokenKind::KwContinue).map_with(|_, extra| {
            Stmt::Continue(to_span(extra.span()))
        });

        // Fall-through: bare expression statement
        let expr_stmt = e.clone().map_with(|expr, extra| {
            Stmt::Expr(ExprStmt {
                expr,
                span: to_span(extra.span()),
            })
        });

        choice((
            let_stmt,
            if_stmt,
            return_stmt,
            reply_stmt,
            for_stmt,
            while_stmt,
            break_stmt,
            continue_stmt,
            expr_stmt,
        ))
        // On stmt error, consume to end of line or closing brace and emit an Error expr.
        .recover_with(via_parser(
            any()
                .and_is(just(TokenKind::Newline).or(just(TokenKind::RBrace)).not())
                .repeated()
                .at_least(1)
                .map_with(|_, extra| {
                    use quew_ast::stmt::ExprStmt;
                    Stmt::Expr(ExprStmt {
                        expr: quew_ast::Expr::Error(to_span(extra.span())),
                        span: to_span(extra.span()),
                    })
                }),
        ))
    })
}
