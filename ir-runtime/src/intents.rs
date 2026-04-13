use crate::flat_args::{
    flatten_example_object, flatten_helper_input_specs, flatten_named_field_specs,
    flatten_output_specs,
};
use crate::types::{AgentIR, ComponentDefinition, TypeDefinition};
use serde_json::Value;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// BLOCK PROTOCOL PROMPT GENERATION
// ═══════════════════════════════════════════════════════════════════════════

pub fn generate_block_protocol_prompt(ir: &AgentIR) -> String {
    let mut sections = Vec::new();

    let has_tools = !ir.tools.is_empty();
    let has_workflows = !ir.workflows.is_empty();
    let has_helpers = !ir.helpers.is_empty();
    let has_actions = has_tools || has_workflows || has_helpers;
    let has_components = !ir.components.is_empty();
    let has_custom = ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let has_output = ir
        .output
        .as_ref()
        .and_then(|o| o.0.as_object())
        .map(|o| !o.is_empty())
        .unwrap_or(false);
    let action_type_count = [has_tools, has_workflows, has_helpers]
        .iter()
        .filter(|&&x| x)
        .count();

    // ── Header + rules ──────────────────────────────────────────────────────
    let mut rules = vec![
        "You are an execution engine. Respond only with valid protocol blocks.".to_string(),
        "- Always use at least one protocol block.".to_string(),
        "- If no action needed, use [response_text].".to_string(),
    ];
    if has_actions {
        rules.push("- For actions: emit only action block(s) for that turn and stop. Wait for [result] before replying.".to_string());
    }
    if has_components {
        rules.push("- For UI: emit [component] block(s) then [render_component].".to_string());
    }
    rules.push("- Close every block. Never invent names or fields not listed.".to_string());

    sections.push(rules.join("\n"));

    // ── Blocks (merged allowed + syntax — one section) ──────────────────────
    let mut blocks = Vec::new();

    blocks.push("[response_text] plain text [/response_text]".to_string());

    if has_tools {
        blocks.push("[tool_call: name] key: value per line [/tool]".to_string());
    }
    if has_workflows {
        blocks.push("[workflow_call: name] key: value per line [/workflow]".to_string());
    }
    if has_helpers {
        blocks.push("[helper_call: name] key: value per line [/helper]".to_string());
    }
    if has_components {
        blocks.push("[component: name, c_id:\"id\"] key: value per line [/component]".to_string());
        blocks.push("[render_component] root: \"c_id\" [/render_component]".to_string());
    }
    if has_custom {
        blocks.push("[custom: name] key: value per line [/custom]".to_string());
    }
    if has_output {
        blocks.push("[schema: name] key: value per line [/schema]".to_string());
    }

    blocks.push("Values: string, number, boolean, null, [array], {object}".to_string());

    sections.push(format!("\nBlocks:\n- {}", blocks.join("\n- ")));

    // ── Tools ────────────────────────────────────────────────────────────────
    if has_tools {
        let mut tool_section = String::from("\nTools:\n");
        for tool in &ir.tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!(
                "- {}",
                format_callable_signature(&tool.name, &params)
            ));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }
        let examples = collect_tool_example_blocks(&ir.tools, ir.types.as_ref());
        if !examples.is_empty() {
            tool_section.push_str("\nTool examples:\n");
            for ex in examples.iter().take(3) {
                tool_section.push_str(ex);
                tool_section.push('\n');
            }
        }
        sections.push(tool_section);
    }

    // ── Workflows ────────────────────────────────────────────────────────────
    if has_workflows {
        let mut workflow_section = String::from("\nWorkflows:\n");
        for workflow in &ir.workflows {
            let params = format_params_signature(&workflow.params.0, ir.types.as_ref());
            workflow_section.push_str(&format!(
                "- {}",
                format_callable_signature(&workflow.name, &params)
            ));
            if let Some(desc) = &workflow.description {
                workflow_section.push_str(&format!(" // {}", desc));
            }
            workflow_section.push('\n');
        }
        let examples = collect_workflow_example_blocks(&ir.workflows, ir.types.as_ref());
        if let Some(example) = examples.first() {
            workflow_section.push_str("\nWorkflow example:\n");
            workflow_section.push_str(example);
            workflow_section.push('\n');
        }
        sections.push(workflow_section);
    }

    // ── Helpers ──────────────────────────────────────────────────────────────
    if has_helpers {
        let mut helper_section = String::from("\nHelpers:\n");
        for helper in &ir.helpers {
            let params = format_helper_params_signature(
                helper.input.as_ref().map(|v| &v.0),
                ir.types.as_ref(),
            );
            helper_section.push_str(&format!(
                "- {}",
                format_callable_signature(&helper.name, &params)
            ));
            if let Some(desc) = &helper.description {
                helper_section.push_str(&format!(" // {}", desc));
            }
            helper_section.push('\n');
        }
        let examples = collect_helper_example_blocks(&ir.helpers, ir.types.as_ref());
        if let Some(example) = examples.first() {
            helper_section.push_str("\nHelper example:\n");
            helper_section.push_str(example);
            helper_section.push('\n');
        }
        sections.push(helper_section);
    }

    // ── Components ───────────────────────────────────────────────────────────
    if has_components {
        let mut component_section = String::from("\nComponents:\n");
        for component in &ir.components {
            component_section.push_str(&format!(
                "- {}\n",
                format_component_signature(component, ir.types.as_ref())
            ));
        }
        if let Some(example) = collect_component_example_block(&ir.components, ir.types.as_ref()) {
            component_section.push_str("\nComponent example:\n");
            component_section.push_str(&example);
            component_section
                .push_str("\n[render_component]\nroot: \"<c_id>\"\n[/render_component]\n");
        }
        sections.push(component_section);
    }

    // ── Custom intents ───────────────────────────────────────────────────────
    if let Some(custom) = &ir.custom_intents {
        if !custom.is_empty() {
            let mut custom_section = String::from("\nCustom intents:\n");
            for ci in custom {
                let params_sig = format_params_signature(&ci.fields.0, ir.types.as_ref());
                custom_section.push_str(&format!(
                    "- {}",
                    format_callable_signature(&ci.name, &params_sig)
                ));
                if let Some(desc) = &ci.description {
                    custom_section.push_str(&format!(" // {}", desc));
                }
                custom_section.push('\n');
                if let Some(example) = ci.examples.first() {
                    custom_section.push_str(&format!(
                        "\nCustom example:\n[custom: {}]\n{}\n[/custom]\n",
                        ci.name,
                        format_custom_intent_example_inline(
                            &example.0,
                            &ci.fields.0,
                            ir.types.as_ref(),
                        )
                    ));
                }
            }
            sections.push(custom_section);
        }
    }

    // ── Output schemas ───────────────────────────────────────────────────────
    if has_output {
        if let Some(output) = &ir.output {
            let schema_entries = collect_output_schema_entries(&output.0, ir.types.as_ref());
            let schema_list = schema_entries
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
                .join("|");

            let mut schema_section = format!(
                "\nSchemas (final structured output only):\n[schema: {}]\n",
                schema_list
            );
            for (schema_name, specs) in &schema_entries {
                schema_section.push_str(&format!(
                    "- {}\n",
                    format_callable_signature(schema_name, &format_flat_field_specs(specs))
                ));
            }
            if let Some(example) = collect_output_schema_example_block(&output.0, ir.types.as_ref())
            {
                schema_section.push_str("\nSchema example:\n");
                schema_section.push_str(&example);
                schema_section.push('\n');
            }
            sections.push(schema_section);
        }
    }

    // ── Constraints (only emit what applies) ─────────────────────────────────
    let mut constraints = Vec::new();
    if action_type_count > 1 {
        constraints.push(
            "- One action type per response. No mixing tool_call, workflow_call, helper_call."
                .to_string(),
        );
    }
    if has_actions {
        constraints.push("- No response_text in the same response as action blocks.".to_string());
    }
    if has_tools {
        constraints.push("- Tool fields must match listed signatures.".to_string());
    }
    if has_workflows {
        constraints.push("- Workflow fields must match listed signatures.".to_string());
    }
    if has_helpers {
        constraints.push("- Helper fields must match listed signatures.".to_string());
    }
    if has_components {
        constraints
            .push("- Components require c_id. UI must end with [render_component].".to_string());
    }
    if has_output {
        constraints.push("- Schema output must match listed shape.".to_string());
    }

    if !constraints.is_empty() {
        sections.push(format!("\nConstraints:\n{}", constraints.join("\n")));
    }

    sections.join("")
}

/// Generate helper-specific prompt using block protocol format.
pub fn generate_helper_block_protocol_prompt(ir: &AgentIR, helper_name: &str) -> String {
    let mut sections = Vec::new();

    let allowed_tools: Vec<_> = {
        let mut tools = Vec::new();
        if let Some(grants) = &ir.helper_tool_grants {
            if let Some(grant) = grants.get(helper_name) {
                match &grant.0 {
                    serde_json::Value::String(s) if s == "all" => {
                        tools = ir.tools.clone();
                    }
                    serde_json::Value::Array(arr) => {
                        let allowed_names: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        tools = ir
                            .tools
                            .iter()
                            .filter(|t| allowed_names.contains(&t.name))
                            .cloned()
                            .collect();
                    }
                    _ => {}
                }
            }
        }
        tools
    };

    let has_tools = !allowed_tools.is_empty();
    let has_components = !ir.components.is_empty();
    let has_custom = ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    // ── Header + rules ────────────────────────────────────────────────────────
    let mut rules = vec![
        "You are a specialized helper agent. Respond only with valid protocol blocks.".to_string(),
        "- Always use at least one protocol block.".to_string(),
        "- If no action needed, use [response_text].".to_string(),
    ];
    if has_tools {
        rules.push("- For tools: emit only tool_call block(s) and stop. Wait for [result] before replying.".to_string());
    }
    if has_components {
        rules.push("- For UI: emit [component] block(s) then [render_component].".to_string());
    }
    rules.push("- Close every block. Never invent names or fields not listed.".to_string());

    sections.push(rules.join("\n"));

    // ── Blocks ────────────────────────────────────────────────────────────────
    let mut blocks = vec!["[response_text] plain text [/response_text]".to_string()];
    if has_tools {
        blocks.push(
            "[tool_call: name] key: value per line [/tool]
            when you call a tool, it wait for the [result] every tool return that dont assume.
        "
            .to_string(),
        );
    }
    if has_components {
        blocks.push("[component: name, c_id:\"id\"] key: value per line [/component]".to_string());
        blocks.push("[render_component] root: \"c_id\" [/render_component]".to_string());
    }
    if has_custom {
        blocks.push("[custom: name] key: value per line [/custom]".to_string());
    }
    blocks.push("Values: string, number, boolean, null, [the list], {the object} array and object are like regular ts".to_string());

    sections.push(format!("\nBlocks:\n- {}", blocks.join("\n- ")));

    // ── Tools ─────────────────────────────────────────────────────────────────
    if has_tools {
        let mut tool_section = String::from("\nTools:\n");
        for tool in &allowed_tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!(
                "- {}",
                format_callable_signature(&tool.name, &params)
            ));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }
        let examples = collect_tool_example_blocks(&allowed_tools, ir.types.as_ref());
        if !examples.is_empty() {
            tool_section.push_str("\nTool examples:\n");
            tool_section.push_str(&examples.join("\n"));
            tool_section.push('\n');
        }
        sections.push(tool_section);
    }

    // ── Custom intents ────────────────────────────────────────────────────────
    if let Some(custom) = &ir.custom_intents {
        if !custom.is_empty() {
            let mut custom_section = String::from("\nCustom intents:\n");
            for ci in custom {
                let params_sig = format_params_signature(&ci.fields.0, ir.types.as_ref());
                custom_section.push_str(&format!(
                    "- {}",
                    format_callable_signature(&ci.name, &params_sig)
                ));
                if let Some(desc) = &ci.description {
                    custom_section.push_str(&format!(" // {}", desc));
                }
                custom_section.push('\n');
                if let Some(example) = ci.examples.first() {
                    custom_section.push_str(&format!(
                        "\nCustom example:\n[custom: {}]\n{}\n[/custom]\n",
                        ci.name,
                        format_custom_intent_example_inline(
                            &example.0,
                            &ci.fields.0,
                            ir.types.as_ref(),
                        )
                    ));
                }
            }
            sections.push(custom_section);
        }
    }

    // ── Components ────────────────────────────────────────────────────────────
    if has_components {
        let mut component_section = String::from("\nComponents:\n");
        for component in &ir.components {
            component_section.push_str(&format!(
                "- {}\n",
                format_component_signature(component, ir.types.as_ref())
            ));
        }
        if let Some(example) = collect_component_example_block(&ir.components, ir.types.as_ref()) {
            component_section.push_str("\nComponent example:\n");
            component_section.push_str(&example);
            component_section
                .push_str("\n[render_component]\nroot: \"<c_id>\"\n[/render_component]\n");
        }
        sections.push(component_section);
    }

    // ── Constraints ───────────────────────────────────────────────────────────
    let mut constraints = Vec::new();
    if has_tools {
        constraints
            .push("- No response_text in the same response as tool_call blocks.".to_string());
        constraints.push("- Tool fields must match listed signatures.".to_string());
    }
    if has_components {
        constraints
            .push("- Components require c_id. UI must end with [render_component].".to_string());
    }

    if !constraints.is_empty() {
        sections.push(format!("\nConstraints:\n{}", constraints.join("\n")));
    }

    sections.join("")
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS (unchanged)
// ═══════════════════════════════════════════════════════════════════════════

fn format_callable_signature(name: &str, params: &str) -> String {
    if params.trim().is_empty() {
        name.to_string()
    } else {
        format!("{}({})", name, params)
    }
}

fn format_params_signature(
    params: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let specs = flatten_named_field_specs(params, types);
    format_flat_field_specs(&specs)
}

fn format_helper_params_signature(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let specs = flatten_helper_input_specs(input_ir, types);
    format_flat_field_specs(&specs)
}

fn collect_tool_example_blocks(
    tools: &[crate::types::Tool],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<String> {
    let mut blocks = Vec::new();
    for tool in tools {
        if let Some(first_example) = tool.examples.first() {
            blocks.push(format_named_block_example(
                "tool_call",
                &tool.name,
                &first_example.0,
                &flatten_named_field_specs(&tool.params.0, types),
            ));
            if blocks.len() >= 5 {
                break;
            }
        }
    }
    blocks
}

fn collect_workflow_example_blocks(
    workflows: &[crate::types::Workflow],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<String> {
    let mut blocks = Vec::new();
    for wf in workflows {
        if let Some(first_example) = wf.examples.first() {
            blocks.push(format_named_block_example(
                "workflow_call",
                &wf.name,
                &first_example.0,
                &flatten_named_field_specs(&wf.params.0, types),
            ));
            break;
        }
    }
    blocks
}

fn collect_helper_example_blocks(
    helpers: &[crate::types::Helper],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<String> {
    let mut blocks = Vec::new();
    for helper in helpers {
        if let Some(first_example) = helper.examples.first() {
            blocks.push(format_named_block_example(
                "helper_call",
                &helper.name,
                &first_example.0,
                &flatten_helper_input_specs(helper.input.as_ref().map(|v| &v.0), types),
            ));
            break;
        }
    }
    blocks
}

fn format_named_block_example(
    block_kind: &str,
    name: &str,
    args: &serde_json::Value,
    specs: &[crate::flat_args::FlatFieldSpec],
) -> String {
    let flattened_args = flatten_example_object(args, specs);
    let close_tag = match block_kind {
        "tool_call" => "tool",
        "workflow_call" => "workflow",
        "helper_call" => "helper",
        _ => "block",
    };
    if flattened_args.is_empty() {
        return format!("[{}: {}]\n[/{}]", block_kind, name, close_tag);
    }
    format!(
        "[{}: {}]\n{}\n[/{}]",
        block_kind,
        name,
        format_flattened_fields(&flattened_args),
        close_tag
    )
}

fn collect_output_schema_entries(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<(String, Vec<crate::flat_args::FlatFieldSpec>)> {
    let mut entries: Vec<(String, Vec<crate::flat_args::FlatFieldSpec>)> =
        flatten_output_specs(output, types).into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn collect_output_schema_example_block(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Option<String> {
    let examples = output.get("@examples").and_then(|v| v.as_array())?;
    let example = examples.first()?;
    let schema_name = example
        .get("__schema_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Output");
    let specs_map = flatten_output_specs(output, types);
    let specs = specs_map
        .get(schema_name)
        .or_else(|| specs_map.get("Output"))?;
    Some(format!(
        "[schema: {}]\n{}\n[/schema]",
        schema_name,
        format_flattened_fields(&flatten_example_object(example, specs))
    ))
}

fn format_value_inline(val: &Value) -> String {
    let mut actual_val = val;
    if let Some(obj) = val.as_object() {
        if let Some(t_str) = obj.get("type").and_then(|t| t.as_str()) {
            if matches!(t_str, "literal" | "array" | "object") {
                if let Some(v) = obj.get("value") {
                    actual_val = v;
                }
            }
        }
    }
    match actual_val {
        Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_inline).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut fields = Vec::new();
            let mut keys: Vec<&String> = obj.keys().filter(|key| !key.starts_with("__")).collect();
            keys.sort();
            for key in keys {
                if let Some(value) = obj.get(key) {
                    fields.push(format!("{}: {}", key, format_value_inline(value)));
                }
            }
            format!("{{ {} }}", fields.join(", "))
        }
    }
}

fn format_custom_intent_example_inline(
    example: &serde_json::Value,
    fields: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let specs = flatten_named_field_specs(fields, types);
    format_flattened_fields(&flatten_example_object(example, &specs))
}

fn format_flat_field_specs(specs: &[crate::flat_args::FlatFieldSpec]) -> String {
    specs
        .iter()
        .map(|spec| {
            let mut param_decl = if spec.optional {
                format!("{}?: {}", spec.alias, spec.type_repr)
            } else {
                format!("{}: {}", spec.alias, spec.type_repr)
            };
            if let Some(desc) = &spec.description {
                param_decl.push_str(&format!(" /* {} */", desc));
            }
            param_decl
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_component_signature(
    component: &ComponentDefinition,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let mut parts = Vec::new();
    let prop_sig = format_params_signature(&component.props.0, types);
    if !prop_sig.is_empty() {
        parts.push(prop_sig);
    }
    if let Some(action) = &component.action {
        let mut action_keys: Vec<_> = action.keys().cloned().collect();
        action_keys.sort();
        for key in action_keys {
            if let Some(allowed) = action.get(&key) {
                let variants = allowed
                    .iter()
                    .map(|target| {
                        if let Some(params) = target.params.as_ref() {
                            let sig = format_params_signature(&params.0, types);
                            if sig.is_empty() {
                                target.name.clone()
                            } else {
                                format!("{}({})", target.name, sig)
                            }
                        } else {
                            target.name.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                parts.push(format!("action_{}: {}", key, variants));
            }
        }
    }
    if let Some(children) = &component.children {
        let child_desc = match children {
            crate::types::ComponentChildrenConstraint::All => "children: all".to_string(),
            crate::types::ComponentChildrenConstraint::Only { components } => {
                format!("children: {}", components.join(" | "))
            }
        };
        parts.push(child_desc);
    }
    if parts.is_empty() {
        return format!("{}()", component.name);
    }
    format!("{}({})", component.name, parts.join(", "))
}

fn collect_component_example_block(
    components: &[ComponentDefinition],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Option<String> {
    let component = components.first()?;
    let mut lines = vec![format!(
        "[component: {}, c_id:\"{}_instance\"]",
        component.name,
        component.name.to_lowercase()
    )];
    if let Some(props) = component.props.0.as_object() {
        let mut keys: Vec<_> = props.keys().cloned().collect();
        keys.sort();
        if let Some(first_key) = keys.first() {
            lines.push(format!("{first_key}: \"value\""));
        }
    }
    if let Some(action) = &component.action {
        let mut action_keys: Vec<_> = action.keys().cloned().collect();
        action_keys.sort();
        if let Some(first_action) = action_keys.first()
            && let Some(allowed) = action.get(first_action)
            && let Some(first_allowed) = allowed.first()
        {
            if let Some(params) = &first_allowed.params {
                let specs = flatten_named_field_specs(&params.0, types);
                let args = specs
                    .iter()
                    .map(|spec| format!("{}: \"value\"", spec.alias))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!(
                    "action_{first_action}: {}({args})",
                    first_allowed.name
                ));
            } else {
                lines.push(format!("action_{first_action}: {}", first_allowed.name));
            }
        }
    }
    lines.push("[/component]".to_string());
    Some(lines.join("\n"))
}

fn format_flattened_fields(flattened: &[(String, Value)]) -> String {
    flattened
        .iter()
        .map(|(key, value)| format!("{}: {}", key, format_value_inline(value)))
        .collect::<Vec<_>>()
        .join("\n")
}
