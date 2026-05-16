//! Lower an agent body into an `AgentGraph`.
//! Step 3 will fill this with the full implementation.

use std::sync::Arc;

use quew_ast::AgentDecl;
use quew_checker::CheckResult;
use quew_interner::Interner;

use crate::defs::Definitions;
use crate::graph::{AgentGraph, CheckpointPolicy, IrNode, NodeKind};
use crate::types::IrType;

use super::ctx::LowerCtx;

/// Lower one agent declaration's body into an `AgentGraph`.
pub fn lower_agent(
    agent: &AgentDecl,
    _check: &CheckResult,
    interner: &Arc<Interner>,
    _definitions: &Definitions,
) -> AgentGraph {
    let mut ctx = LowerCtx::new();
    let graph_id = format!("agent:{}", interner.resolve(agent.name));

    // ── Boundary nodes ────────────────────────────────────────────────────────

    // n0: input node
    let input_id = ctx.next_node();
    let input_node = IrNode {
        id:         input_id,
        kind:       NodeKind::Input { input_ty: IrType::Text }, // TODO: lower agent.input param type
        checkpoint: CheckpointPolicy::Never,
    };

    // Bind the agent's single input parameter so body expressions can reference it.
    ctx.bind(agent.param.name, input_id);

    // n_last: output node (allocated now, wired after body lowering)
    let output_id = ctx.next_node();

    // TODO Step 3: lower agent body statements into nodes between input and output.

    let output_node = IrNode {
        id:         output_id,
        kind:       NodeKind::Output {
            // Temporary: wire output directly to input until body lowering is done.
            value: crate::graph::DataRef::scalar(input_id),
        },
        checkpoint: CheckpointPolicy::Never,
    };

    AgentGraph {
        graph_id,
        entry_node:  input_id,
        return_node: output_id,
        nodes:       vec![input_node, output_node],
        edges:       vec![],
    }
}
