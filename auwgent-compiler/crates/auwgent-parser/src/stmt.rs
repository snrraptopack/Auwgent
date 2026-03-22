//! Statement parsers (let, return, if, transfer, parallel, assign).

use auwgent_ast::*;
use auwgent_lexer::TokenKind;
use chumsky::prelude::*;

use crate::expr::{condition_parser, expr_parser};
use crate::primitives::*;
use crate::types::type_expr_parser;

pub(crate) fn statement_parser(
) -> impl Parser<TokenKind, Statement, Error = Simple<TokenKind>> + Clone + 'static {
    recursive(
        |stmt: Recursive<'_, TokenKind, Statement, Simple<TokenKind>>| {
            let expr = expr_parser();

            // let name [: type] = expr
            let let_stmt = tok(TokenKind::Let)
                .ignore_then(ident())
                .then(
                    tok(TokenKind::Colon)
                        .ignore_then(type_expr_parser())
                        .or_not(),
                )
                .then_ignore(tok(TokenKind::Eq))
                .then(expr.clone())
                .map_with_span(|((name, ty), value), span| {
                    Statement::Let(LetStatement {
                        name,
                        ty,
                        value,
                        span: s(span),
                    })
                });

            // return expr
            let return_stmt = tok(TokenKind::Return)
                .ignore_then(expr.clone())
                .map_with_span(|value, span| {
                    Statement::Return(ReturnStatement {
                        value,
                        span: s(span),
                    })
                });

            // if (condition) { stmts } [else { stmts }]
            let stmts_block = stmt
                .clone()
                .repeated()
                .delimited_by(tok(TokenKind::LBrace), tok(TokenKind::RBrace));

            let if_stmt = tok(TokenKind::If)
                .ignore_then(
                    condition_parser().delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
                )
                .then(stmts_block.clone())
                .then(
                    tok(TokenKind::Else)
                        .ignore_then(stmts_block.clone())
                        .or_not(),
                )
                .map_with_span(|((condition, then_block), else_block), span| {
                    Statement::If(IfStatement {
                        condition,
                        then_block,
                        else_block: else_block.unwrap_or_default(),
                        span: s(span),
                    })
                });

            // transfer to hlp.helper(args) [then continue]
            let transfer_stmt = tok(TokenKind::Transfer)
                .ignore_then(tok(TokenKind::To))
                .ignore_then(tok(TokenKind::Hlp))
                .ignore_then(tok(TokenKind::Dot))
                .ignore_then(ident())
                .then(
                    expr.clone()
                        .separated_by(tok(TokenKind::Comma))
                        .allow_trailing()
                        .delimited_by(tok(TokenKind::LParen), tok(TokenKind::RParen)),
                )
                .then(tok(TokenKind::Then).then(tok(TokenKind::Continue)).or_not())
                .map_with_span(|((helper, args), tc), span| {
                    Statement::Transfer(TransferStatement {
                        call: HelperCall {
                            helper,
                            args,
                            span: s(span.clone()),
                        },
                        then_continue: tc.is_some(),
                        span: s(span),
                    })
                });

            // parallel { stmts }
            let parallel_stmt = tok(TokenKind::Parallel)
                .ignore_then(stmts_block)
                .map_with_span(|body, span| {
                    Statement::Parallel(ParallelStatement {
                        body,
                        span: s(span),
                    })
                });

            // name = expr (assignment)
            let assign_stmt = ident()
                .then_ignore(tok(TokenKind::Eq))
                .then(expr)
                .map_with_span(|(variable, value), span| {
                    Statement::Assign(AssignStatement {
                        variable,
                        value,
                        span: s(span),
                    })
                });

            choice((
                let_stmt,
                return_stmt,
                if_stmt,
                transfer_stmt,
                parallel_stmt,
                assign_stmt,
            ))
        },
    )
    .boxed()
}
