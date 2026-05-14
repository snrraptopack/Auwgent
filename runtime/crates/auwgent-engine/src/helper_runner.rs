use auwgent_ir_schema::{AgentIR, HandoffKindIR, ToolIR};
use auwgent_runtime_core::{AuwgentError, AuwgentResult};
use serde_json::Value;

/// The output of building a sub-agent context.
pub struct SubAgentContext {
    /// The constructed AgentIR for the helper.
    pub ir: AgentIR,
    /// A list of parent tool names that the helper is authorized to use.
    pub authorized_parent_tool_names: Vec<String>,
    /// How the helper's response should be routed.
    pub handoff_mode: HandoffKindIR,
}

/// Build the sub-agent context (IR + authorized tools + handoff mode) for a helper.
///
/// This is a pure function: it reads from the parent IR and returns a context
/// struct without touching any engine state.
pub fn build_sub_agent_context(
    parent_ir: &AgentIR,
    helper_name: &str,
) -> AuwgentResult<SubAgentContext> {
    // 1. Find the helper definition in the parent IR
    let helper = parent_ir
        .helpers
        .iter()
        .find(|h| h.name == helper_name)
        .ok_or_else(|| AuwgentError::UnknownHelper(helper_name.to_string()))?;

    // 2. Determine which parent tools the helper is authorized to use
    let mut authorized_parent_tool_names: Vec<String> = Vec::new();
    let mut inherited_tool_defs: Vec<ToolIR> = Vec::new();

    if let Some(grants) = &parent_ir.helper_tool_grants {
        if let Some(grant_val) = grants.get(helper_name) {
            match &grant_val.0 {
                Value::String(s) if s == "all" => {
                    // Grant all parent tools
                    for tool in &parent_ir.tools {
                        authorized_parent_tool_names.push(tool.name.clone());
                        inherited_tool_defs.push(tool.clone());
                    }
                }
                Value::Array(arr) => {
                    // Grant specific parent tools
                    let names: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();

                    for tool in &parent_ir.tools {
                        if names.contains(&tool.name) {
                            authorized_parent_tool_names.push(tool.name.clone());
                            inherited_tool_defs.push(tool.clone());
                        }
                    }
                }
                _ => {} // Invalid grant format — no parent tools authorized
            }
        }
    }

    // 3. Combine inherited parent tools with the helper's own tools
    let mut final_tools = helper.tools.clone();
    final_tools.extend(inherited_tool_defs);

    // 4. Extract handoff mode
    let handoff_mode = HandoffKindIR::from_ir_str(
        parent_ir
            .helper_handoff
            .as_ref()
            .and_then(|h| h.get(helper_name))
            .map(|s| s.as_str()),
    );

    // 5. Construct the sub-agent's AgentIR
    let sub_ir = AgentIR {
        name: helper.name.clone(),
        model_config: helper.model_config.clone(),
        input: helper.input.clone(),
        output: helper.output.clone(),
        context: helper.context.clone(),
        tools: final_tools,
        workflows: helper.workflows.clone(),
        helpers: Vec::new(), // Helpers cannot call other helpers
        components: parent_ir.components.clone(),
        types: parent_ir.types.clone(), // Share global type definitions
        helper_tool_grants: None,
        helper_handoff: None,
        tests: Vec::new(),
        lifecycle: parent_ir.lifecycle.clone(),
        custom_intents: helper.custom_intents.clone(),
    };

    Ok(SubAgentContext {
        ir: sub_ir,
        authorized_parent_tool_names,
        handoff_mode,
    })
}
