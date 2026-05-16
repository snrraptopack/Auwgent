//! Lower `Expr` nodes into `IrExpr` / `DataRef`.
//! Step 3 will fill this with the full implementation.

use quew_ast::Expr;
use quew_checker::CheckResult;

use crate::graph::{DataRef, IrExpr, IrLit};

use super::ctx::LowerCtx;

/// Lower an expression into an `IrExpr` (for use inside `LetBind` nodes).
///
/// For expressions that require their own node (agent calls, host tool calls),
/// call the relevant graph-lowering function instead.
pub fn lower_expr(
    _expr: &Expr,
    _check: &CheckResult,
    _ctx: &mut LowerCtx,
) -> IrExpr {
    // TODO: Step 3 — walk Expr variants and map to IrExpr
    IrExpr::Lit(IrLit::Null) // placeholder
}

/// Lower an expression into a `DataRef`, emitting a `LetBind` node if needed.
///
/// Simple identifier expressions resolve directly to their slot. Everything
/// else gets wrapped in a new `LetBind` node.
pub fn lower_expr_as_ref(
    _expr: &Expr,
    _check: &CheckResult,
    _ctx: &mut LowerCtx,
) -> DataRef {
    // TODO: Step 3
    DataRef::scalar(crate::graph::NodeId(0)) // placeholder
}
