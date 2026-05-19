//! Lower `Expr` nodes into pure `IrExpr` values or existing `DataRef`s.
//!
//! This module deliberately handles only expression shape. Statements decide
//! when an expression becomes a graph node.

use std::sync::Arc;

use quew_ast::{BinaryOp as AstBinaryOp, Expr, Lit, UnaryOp as AstUnaryOp};
use quew_checker::resolved::CallKind;
use quew_checker::CheckResult;
use quew_interner::Interner;
use quew_types::{PrimTy, Ty};

use crate::defs::Definitions;
use crate::graph::{BinaryOp, DataRef, IrExpr, IrLit, UnaryOp};

use super::ctx::LowerCtx;

/// Lower an expression into an `IrExpr` for embedding in deterministic nodes.
pub fn lower_expr(
    expr: &Expr,
    check: &CheckResult,
    definitions: &Definitions,
    interner: &Arc<Interner>,
    ctx: &mut LowerCtx,
) -> IrExpr {
    match expr {
        Expr::Lit(lit) => IrExpr::Lit(lower_lit(lit)),
        Expr::Ident(ident) => IrExpr::Ref(
            ctx.resolve(ident.name)
                .unwrap_or_else(|| panic!("lowering bug: unresolved identifier {ident:?}")),
        ),
        Expr::Binary(binary) => IrExpr::Binary {
            left: Box::new(lower_expr(&binary.left, check, definitions, interner, ctx)),
            op: lower_binary_op(binary.op),
            right: Box::new(lower_expr(&binary.right, check, definitions, interner, ctx)),
        },
        Expr::Unary(unary) => IrExpr::Unary {
            op: lower_unary_op(unary.op),
            expr: Box::new(lower_expr(&unary.operand, check, definitions, interner, ctx)),
        },
        Expr::Member(member) => IrExpr::Member {
            base: Box::new(lower_expr(&member.object, check, definitions, interner, ctx)),
            field: member.field,
        },
        Expr::Call(call) => {
            // First try to resolve via the checker's sidecar (extension methods).
            if let Expr::Member(member) = call.callee.as_ref() {
                if let Some(resolved) = check.resolved.get_call(call.span) {
                    if resolved.kind == CallKind::ExtensionMethod {
                        let graph_ref = extension_graph_ref(resolved, interner);
                        let function = interner.intern(&graph_ref);

                        let mut args = indexmap::IndexMap::new();
                        args.insert(
                            interner.intern("self"),
                            lower_expr(&member.object, check, definitions, interner, ctx),
                        );

                        // Map explicit args using param names from the extension def.
                        if let Some(ext) = definitions.extensions.iter().find(|e| e.graph_ref == graph_ref) {
                            for (idx, (param_name, _)) in ext.params.iter().enumerate() {
                                if let Some(arg) = call.args.get(idx) {
                                    args.insert(
                                        *param_name,
                                        lower_expr(arg, check, definitions, interner, ctx),
                                    );
                                }
                            }
                        } else {
                            // Fallback: positional args with generated names.
                            for (idx, arg) in call.args.iter().enumerate() {
                                let name = interner.intern(&format!("arg{idx}"));
                                args.insert(name, lower_expr(arg, check, definitions, interner, ctx));
                            }
                        }

                        return IrExpr::Call { function, args };
                    }
                }
            }

            let function = match call.callee.as_ref() {
                Expr::Ident(ident) => ident.name,
                other => panic!(
                    "lowering bug: non-identifier call callee in pure expression: {other:?}"
                ),
            };

            let mut args = indexmap::IndexMap::new();
            if let Some(func) = definitions.functions.get(&function) {
                for (idx, (param_name, _)) in func.params.iter().enumerate() {
                    if let Some(arg) = call.args.get(idx) {
                        args.insert(
                            *param_name,
                            lower_expr(arg, check, definitions, interner, ctx),
                        );
                    }
                }
            } else {
                // Fallback for tools, agents, or unresolved calls: positional arg0, arg1, ...
                for (idx, arg) in call.args.iter().enumerate() {
                    let name = interner.intern(&format!("arg{idx}"));
                    args.insert(name, lower_expr(arg, check, definitions, interner, ctx));
                }
            }

            IrExpr::Call { function, args }
        }
        Expr::Array(array) => IrExpr::Array(
            array
                .elements
                .iter()
                .map(|element| lower_expr(element, check, definitions, interner, ctx))
                .collect(),
        ),
        Expr::PostfixIf(postfix) => IrExpr::Ternary {
            cond: Box::new(lower_expr(&postfix.condition, check, definitions, interner, ctx)),
            then: Box::new(lower_expr(&postfix.value, check, definitions, interner, ctx)),
            else_: Box::new(lower_expr(&postfix.else_value, check, definitions, interner, ctx)),
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

/// Build the graph_ref string for an extension method from checker resolution.
fn extension_graph_ref(resolved: &quew_checker::resolved::ResolvedCall, interner: &Arc<Interner>) -> String {
    let receiver_ty = resolved
        .receiver_ty
        .as_ref()
        .expect("extension method resolved call missing receiver_ty");
    let receiver_name = ty_to_string(receiver_ty, interner);
    format!(
        "extension:{}:{}",
        receiver_name,
        interner.resolve(resolved.target)
    )
}

/// Convert a checker `Ty` into the string form used in IR `graph_ref`s.
fn ty_to_string(ty: &Ty, interner: &Arc<Interner>) -> String {
    match ty {
        Ty::Primitive(PrimTy::String) => "string".into(),
        Ty::Primitive(PrimTy::Number) => "number".into(),
        Ty::Primitive(PrimTy::Float) => "float".into(),
        Ty::Primitive(PrimTy::Bool) => "bool".into(),
        Ty::Primitive(PrimTy::Null) => "null".into(),
        Ty::Primitive(PrimTy::Void) => "void".into(),
        Ty::Named(name) => interner.resolve(*name).to_string(),
        _ => "unknown".into(),
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
