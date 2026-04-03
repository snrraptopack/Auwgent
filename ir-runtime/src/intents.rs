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

/// Generate system prompt using the tag/bracket block protocol.
pub fn generate_block_protocol_prompt(ir: &AgentIR) -> String {
    let mut sections = Vec::new();
    sections.push(
        "You are an execution engine. Respond only with valid protocol blocks.\n\n\
         Rules:\n\
         - Use only the block types listed below.\n\
         - If no external action is needed, reply with [response_text].\n\
         - If UI output is needed, emit one or more [component] blocks.\n\
         - If a tool, workflow, or helper is needed, emit only the action block(s) for that turn and stop.\n\
         - After an action turn, wait for the next turn's [result] block(s) before producing [response_text] or [schema].\n\
         - Close every block correctly.\n\
         - Do not invent tools, workflows, helpers, components, schemas, or custom intents."
            .to_string(),
    );

    let mut allowed_blocks = vec!["- [response_text]...[/response_text]".to_string()];
    if !ir.tools.is_empty() {
        allowed_blocks.push("- [tool_call: type]...[/tool]".to_string());
    }
    if !ir.workflows.is_empty() {
        allowed_blocks.push("- [workflow_call: type]...[/workflow]".to_string());
    }
    if !ir.helpers.is_empty() {
        allowed_blocks.push("- [helper_call: type]...[/helper]".to_string());
    }
    if !ir.components.is_empty() {
        allowed_blocks.push("- [component: type, c_id:\"meaningful_id\"]...[/component]".to_string());
    }
    if ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        allowed_blocks.push("- [custom: type]...[/custom]".to_string());
    }
    if let Some(output) = &ir.output {
        if output.0.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            allowed_blocks.push("- [schema: valid schema name]...[/schema]".to_string());
        }
    }

    sections.push(format!(
        "\n\nAllowed blocks:\n{}",
        allowed_blocks.join("\n")
    ));
    let mut block_syntax = vec![
        "Text response: [response_text]...plain text...[/response_text]".to_string(),
    ];
    if !ir.tools.is_empty() {
        block_syntax.push(
            "Tool call: [tool_call: valid_tool_name] then write one `key: value` or `key = value` field per line, then close with [/tool]".to_string(),
        );
    }
    if !ir.workflows.is_empty() {
        block_syntax.push(
            "Workflow call: [workflow_call: valid_workflow_name] then write one `key: value` or `key = value` field per line, then close with [/workflow]".to_string(),
        );
    }
    if !ir.helpers.is_empty() {
        block_syntax.push(
            "Helper call: [helper_call: valid_helper_name] then write one `key: value` or `key = value` field per line, then close with [/helper]".to_string(),
        );
    }
    if !ir.components.is_empty() {
        block_syntax.push(
            "Component output: [component: valid_component_name, c_id:\"meaningful_accessible_id\"] then write one `key: value` or `key = value` prop per line, use reserved `action_*` fields for actions, then close with [/component]".to_string(),
        );
    }
    if ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        block_syntax.push(
            "Custom intent: [custom: valid_intent_name] then write one `key: value` or `key = value` field per line, then close with [/custom]".to_string(),
        );
    }
    if let Some(output) = &ir.output {
        if output.0.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            block_syntax.push(
                "Schema output: [schema: valid_schema_name] then write one `key: value` or `key = value` field per line, then close with [/schema]".to_string(),
            );
        }
    }
    block_syntax.push(
        "Values may be strings, numbers, booleans, null, arrays like [1, 2], or objects like { city: \"Lagos\" }.".to_string(),
    );
    block_syntax.push(
        "Use only the exact listed name after the colon in the opening block tag.".to_string(),
    );

    sections.push(format!(
        "\n\nBlock syntax:\n- {}",
        block_syntax.join("\n- ")
    ));
    sections.push(
        "\n\nText example:\n[response_text]\nHello! How can I help you today?\n[/response_text]"
            .to_string(),
    );

    if !ir.tools.is_empty() {
        let mut tool_section = String::from("\n\nTools available:\n");
        for tool in &ir.tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!("- {}", format_callable_signature(&tool.name, &params)));
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

    if !ir.workflows.is_empty() {
        let mut workflow_section = String::from("\n\nWorkflows available:\n");
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

    if !ir.helpers.is_empty() {
        let mut helper_section = String::from("\n\nHelpers available:\n");
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

    if !ir.components.is_empty() {
        let mut component_section = String::from("\n\nComponents available:\n");
        for component in &ir.components {
            component_section.push_str(&format!(
                "- {}\n",
                format_component_signature(component, ir.types.as_ref())
            ));
        }

        if let Some(example) = collect_component_example_block(&ir.components) {
            component_section.push_str("\nComponent example:\n");
            component_section.push_str(&example);
            component_section.push('\n');
        }

        sections.push(component_section);
    }

    if let Some(custom) = &ir.custom_intents {
        if !custom.is_empty() {
            let mut custom_section = String::from("\n\nCustom intents available:\n");
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

    if let Some(output) = &ir.output {
        if let Some(obj) = output.0.as_object() {
            if !obj.is_empty() {
                let schema_entries =  collect_output_schema_entries(&output.0, ir.types.as_ref());
                let schema_names: Vec<String> =schema_entries
                    .iter()
                    .map(|(name,_)| name.clone())
                    .collect();

                let schema_list = schema_names.join("|");

                let mut schema_section = String::from(
                   &format!(
                     "\n\nSchemas available:\nUse [schema:{} ] only for final structured output.\n",
                     schema_list
                   ),
                );
                for (schema_name, specs) in
                    collect_output_schema_entries(&output.0, ir.types.as_ref())
                {
                    schema_section.push_str(&format!(
                        "- {}\n",
                        format_callable_signature(&schema_name, &format_flat_field_specs(&specs))
                    ));
                }

                if let Some(example) =
                    collect_output_schema_example_block(&output.0, ir.types.as_ref())
                {
                    schema_section.push_str("\nSchema example:\n");
                    schema_section.push_str(&example);
                    schema_section.push('\n');
                }

                sections.push(schema_section);
            }
        }
    }

    let mut constraints = Vec::new();
    constraints.push("- Use at least one protocol block in every response.".to_string());
    constraints.push("- Never invent names or fields that are not listed.".to_string());
    constraints.push("- You may emit multiple blocks in one response.".to_string());
    constraints.push(
        "- Do not mix tool_call, workflow_call, and helper_call in the same response.".to_string(),
    );
    constraints.push(
        "- Do not emit response_text or response_schema in the same response as any tool_call, workflow_call, or helper_call.".to_string(),
    );

    if !ir.tools.is_empty() {
        constraints.push("- Tool fields must match the listed tool signatures.".to_string());
    }
    if !ir.workflows.is_empty() {
        constraints
            .push("- Workflow fields must match the listed workflow signatures.".to_string());
    }
    if !ir.helpers.is_empty() {
        constraints.push("- Helper fields must match the listed helper signatures.".to_string());
    }
    if !ir.components.is_empty() {
        constraints.push("- Component blocks must use a listed component name and a required c_id header.".to_string());
        constraints.push("- Component fields must match the listed component props and reserved action_* bindings.".to_string());
    }
    if let Some(output) = &ir.output {
        if output.0.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            constraints.push("- Schema output must match the listed schema shape.".to_string());
        }
    }

    sections.push(format!(
        "\n\nCritical constraints:\n{}",
        constraints.join("\n")
    ));
    sections.join("")
}

/// Generate helper-specific prompt using block protocol format.
pub fn generate_helper_block_protocol_prompt(ir: &AgentIR, helper_name: &str) -> String {
    let mut sections = Vec::new();

    // Determine allowed tools
    let mut allowed_tools = Vec::new();
    if let Some(grants) = &ir.helper_tool_grants {
        if let Some(grant) = grants.get(helper_name) {
            match &grant.0 {
                serde_json::Value::String(s) if s == "all" => {
                    allowed_tools = ir.tools.clone();
                }
                serde_json::Value::Array(arr) => {
                    let allowed_names: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    allowed_tools = ir
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

    sections.push(
        "You are a specialized helper agent. Respond only with valid protocol blocks.\n\n\
         Rules:\n\
         - Use only the block types listed below.\n\
         - If no external action is needed, reply with [response_text].\n\
         - If UI output is needed, emit one or more [component] blocks.\n\
         - If a tool is needed, emit only the tool_call block(s) for that turn and stop.\n\
         - After a tool turn, wait for the next turn's [result] block(s) before producing [response_text].\n\
         - Close every block correctly.\n\
         - Do not invent tools, components, or custom intents."
            .to_string(),
    );

    let mut allowed_blocks = vec!["- [response_text]...[/response_text]".to_string()];
    if !allowed_tools.is_empty() {
        allowed_blocks.push("- [tool_call: type]...[/tool]".to_string());
    }
    if !ir.components.is_empty() {
        allowed_blocks.push("- [component: type, c_id:\"meaningful_id\"]...[/component]".to_string());
    }
    if ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        allowed_blocks.push("- [custom: type]...[/custom]".to_string());
    }
    sections.push(format!(
        "\n\nAllowed blocks:\n{}",
        allowed_blocks.join("\n")
    ));
    let mut block_syntax = vec![
        "Text response: [response_text]...plain text...[/response_text]".to_string(),
    ];
    if !allowed_tools.is_empty() {
        block_syntax.push(
            "Tool call: [tool_call: valid_tool_name] then write one `key: value` or `key = value` field per line, then close with [/tool]".to_string(),
        );
    }
    if !ir.components.is_empty() {
        block_syntax.push(
            "Component output: [component: valid_component_name, c_id:\"meaningful_accessible_id\"] then write one `key: value` or `key = value` prop per line, use reserved `action_*` fields for actions, then close with [/component]".to_string(),
        );
    }
    if ir
        .custom_intents
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        block_syntax.push(
            "Custom intent: [custom: valid_intent_name] then write one `key: value` or `key = value` field per line, then close with [/custom]".to_string(),
        );
    }
    block_syntax.push(
        "Values may be strings, numbers, booleans, null, arrays like [1, 2], or objects like { city: \"Lagos\" }.".to_string(),
    );
    block_syntax.push(
        "Use only the exact listed name after the colon in the opening block tag.".to_string(),
    );

    sections.push(format!(
        "\n\nBlock syntax:\n- {}",
        block_syntax.join("\n- ")
    ));
    sections.push(
        "\n\nText example:\n[response_text]\nHello! How can I help you today?\n[/response_text]"
            .to_string(),
    );

    if !allowed_tools.is_empty() {
        let mut tool_section = String::from("\n\nTools available:\n");

        for tool in &allowed_tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!("- {}", format_callable_signature(&tool.name, &params)));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }

        let examples = collect_tool_example_blocks(&allowed_tools, ir.types.as_ref());
        if !examples.is_empty() {
            tool_section.push_str("\nTool example:\n");
            tool_section.push_str(&examples.join("\n"));
            tool_section.push('\n');
        }

        sections.push(tool_section);
    }

    if let Some(custom) = &ir.custom_intents {
        if !custom.is_empty() {
            let mut custom_section = String::from("\n\nCustom intents available:\n");
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

    if !ir.components.is_empty() {
        let mut component_section = String::from("\n\nComponents available:\n");
        for component in &ir.components {
            component_section.push_str(&format!(
                "- {}\n",
                format_component_signature(component, ir.types.as_ref())
            ));
        }
        if let Some(example) = collect_component_example_block(&ir.components) {
            component_section.push_str("\nComponent example:\n");
            component_section.push_str(&example);
            component_section.push('\n');
        }
        sections.push(component_section);
    }

    let mut constraints = Vec::new();
    constraints.push("- Use at least one protocol block in every response.".to_string());
    constraints.push("- Never invent names or fields that are not listed.".to_string());

    if !allowed_tools.is_empty() {
        constraints.push("- Tool fields must match the listed tool signatures.".to_string());
    }
    if !ir.components.is_empty() {
        constraints.push("- Component blocks must use a listed component name and a required c_id header.".to_string());
    }

    sections.push(format!(
        "\n\nCritical constraints:\n{}",
        constraints.join("\n")
    ));

    sections.join("")
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS FOR BLOCK PROTOCOL FORMATTING
// ═══════════════════════════════════════════════════════════════════════════

fn format_callable_signature(name: &str, params: &str) -> String {
    if params.trim().is_empty() {
        name.to_string()
    } else {
        format!("{}({})", name, params)
    }
}

/// Format parameter signature like: session_id: string, apply_compression?: boolean
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
        if !tool.examples.is_empty() {
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
    }

    blocks
}

fn collect_workflow_example_blocks(
    workflows: &[crate::types::Workflow],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<String> {
    let mut blocks = Vec::new();

    for wf in workflows {
        if !wf.examples.is_empty() {
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
    }

    blocks
}

fn collect_helper_example_blocks(
    helpers: &[crate::types::Helper],
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<String> {
    let mut blocks = Vec::new();

    for helper in helpers {
        if !helper.examples.is_empty() {
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
                parts.push(format!("action_{}: {}", key, allowed.join(" | ")));
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
        return format!("{}(c_id: string)", component.name);
    }

    format!("{}(c_id: string, {})", component.name, parts.join(", "))
}

fn collect_component_example_block(components: &[ComponentDefinition]) -> Option<String> {
    let component = components.first()?;
    let mut lines = vec![format!(
        "[component: {}, c_id:\"{}_instance\"]",
        component.name, component.name
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
            lines.push(format!("action_{first_action}: \"{first_allowed}\""));
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
