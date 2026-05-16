//! Lower `Expr` nodes into pure `IrExpr` values or existing `DataRef`s.
//!
//! This module deliberately handles only expression shape. Statements decide
//! when an expression becomes a graph node.

use quew_ast::{BinaryOp as AstBinaryOp, Expr, Lit, UnaryOp as AstUnaryOp};
use quew_checker::CheckResult;

use crate::graph::{BinaryOp, DataRef, IrExpr, IrLit, UnaryOp};

use super::ctx::LowerCtx;

/// Lower an expression into an `IrExpr` for embedding in deterministic nodes.
pub fn lower_expr(expr: &Expr, check: &CheckResult, ctx: &mut LowerCtx) -> IrExpr {
    match expr {
        Expr::Lit(lit) => IrExpr::Lit(lower_lit(lit)),
        Expr::Ident(ident) => IrExpr::Ref(
            ctx.resolve(ident.name)
                .unwrap_or_else(|| panic!("lowering bug: unresolved identifier {ident:?}")),
        ),
        Expr::Binary(binary) => IrExpr::Binary {
            left: Box::new(lower_expr(&binary.left, check, ctx)),
            op: lower_binary_op(binary.op),
            right: Box::new(lower_expr(&binary.right, check, ctx)),
        },
        Expr::Unary(unary) => IrExpr::Unary {
            op: lower_unary_op(unary.op),
            expr: Box::new(lower_expr(&unary.operand, check, ctx)),
        },
        Expr::Member(member) => IrExpr::Member {
            base: Box::new(lower_expr(&member.object, check, ctx)),
            field: member.field,
        },
        Expr::Call(call) => {
            let function = match call.callee.as_ref() {
                Expr::Ident(ident) => ident.name,
                _ => panic!("lowering bug: non-identifier call callee in pure expression"),
            };
            let _ = call;
            IrExpr::Call {
                function,
                args: Default::default(),
            }
        }
        Expr::Array(array) => IrExpr::Array(
            array
                .elements
                .iter()
                .map(|element| lower_expr(element, check, ctx))
                .collect(),
        ),
        Expr::PostfixIf(postfix) => IrExpr::Ternary {
            cond: Box::new(lower_expr(&postfix.condition, check, ctx)),
            then: Box::new(lower_expr(&postfix.value, check, ctx)),
            else_: Box::new(lower_expr(&postfix.else_value, check, ctx)),
        },
        Expr::Is(_) | Expr::Provider(_) | Expr::Error(_) => IrExpr::Lit(IrLit::Null),
    }
}

/// Resolve an expression directly to data produced by an earlier node.
pub fn lower_expr_as_ref(expr: &Expr, _check: &CheckResult, ctx: &mut LowerCtx) -> DataRef {
    match expr {
        Expr::Ident(ident) => ctx
            .resolve(ident.name)
            .unwrap_or_else(|| panic!("lowering bug: unresolved identifier {ident:?}")),
        Expr::Member(member) => {
            let base = lower_expr_as_ref(&member.object, _check, ctx);
            DataRef::field(base.node, member.field)
        }
        _ => panic!("lowering bug: expression requires a node before it can be referenced"),
    }
}

fn lower_lit(lit: &Lit) -> IrLit {
    match lit {
        Lit::Int(value, _) => IrLit::Int(*value),
        Lit::Float(value, _) => IrLit::Float(*value),
        Lit::String(value) => IrLit::String(value.value),
        Lit::Bool(value, _) => IrLit::Bool(*value),
        Lit::Null(_) => IrLit::Null,
    }
}

fn lower_binary_op(op: AstBinaryOp) -> BinaryOp {
    match op {
        AstBinaryOp::Add => BinaryOp::Add,
        AstBinaryOp::Sub => BinaryOp::Sub,
        AstBinaryOp::Mul => BinaryOp::Mul,
        AstBinaryOp::Div => BinaryOp::Div,
        AstBinaryOp::Mod => BinaryOp::Rem,
        AstBinaryOp::Eq | AstBinaryOp::Assign => BinaryOp::Eq,
        AstBinaryOp::NotEq => BinaryOp::NotEq,
        AstBinaryOp::And => BinaryOp::And,
        AstBinaryOp::Or => BinaryOp::Or,
    }
}

fn lower_unary_op(op: AstUnaryOp) -> UnaryOp {
    match op {
        AstUnaryOp::Not => UnaryOp::Not,
    }
}
