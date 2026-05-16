//! Lower a `WithBlock` into a `ReplyConfig`.
//! Step 3 will fill this with the full implementation.

use std::sync::Arc;

use quew_ast::WithBlock;
use quew_checker::CheckResult;
use quew_interner::Interner;

use crate::defs::Definitions;
use crate::graph::ReplyConfig;

use super::ctx::LowerCtx;

/// Lower the `with { … }` block of a `reply(…)` statement into a `ReplyConfig`.
pub fn lower_reply_config(
    _with_block: &WithBlock,
    _check: &CheckResult,
    _interner: &Arc<Interner>,
    _defs: &Definitions,
    _ctx: &mut LowerCtx,
) -> ReplyConfig {
    // TODO: Step 3 — iterate with_block.fields, match on key, populate ReplyConfig
    unimplemented!("lower_reply_config: Step 3 not yet implemented")
}
