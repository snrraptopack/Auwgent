use crate::schema;
use crate::types::AgentIR;

pub fn generate_intents(ir: &AgentIR) -> String {
    let mut sections = Vec::new();

    // ── Dynamic "# Things to Know" section ────────────────────────────────
    // Explains the intent system contextually based on what capabilities
    // the agent actually has (tools, workflows, response schemas, etc.)
    let mut know_items: Vec<String> = Vec::new();

    know_items.push(
        "Intents are actions you perform. Each response must be a YAML block using one or more intent."
            .to_string(),
    );

    if !ir.tools.is_empty() {
        know_items.push(
            "A `tool_call` intent invokes an external tool to fetch or mutate data. \
             After a tool executes, you will receive a `tool_result` — use it to \
             continue or respond to the user."
                .to_string(),
        );
    }

    if !ir.workflows.is_empty() {
        know_items.push(
            "A `workflow_call` intent triggers a deterministic workflow. \
             After it executes, you will receive a `tool_result` — use it to \
             continue or respond to the user."
                .to_string(),
        );
    }

    if !ir.helpers.is_empty() {
        know_items
            .push("A `helper_call` intent delegates a subtask to a specialized agent.".to_string());
    }

    if let Some(output) = &ir.output {
        if matches!(output.as_object(), Some(obj) if !obj.is_empty()) {
            know_items.push(
                "A `response_schema` intent sends structured data directly to the user. \
                 Use it when responding with data that matches the defined schema."
                    .to_string(),
            );
        }
    }

    know_items.push("A `response_text` intent sends a plain text reply to the user.".to_string());

    know_items.push(
        "When you receive a `tool_result`, you MUST respond with another intent \
         (e.g. `response_schema`, `response_text`, or another `tool_call`)."
            .to_string(),
    );

    let know_lines: Vec<String> = know_items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect();

    sections.push(format!("# Things to Know\n{}", know_lines.join("\n")));

    // ── Reference sections (tools, workflows, helpers, response schema) ──

    // Tools
    if !ir.tools.is_empty() {
        let mut tool_lines = Vec::new();
        for tool in &ir.tools {
            let mut params = Vec::new();
            if let Some(obj) = tool.params.as_object() {
                for (name, def) in obj {
                    let field_type = def["type"].as_str().unwrap_or("any");
                    params.push(format!("{}: {}", name, field_type));
                }
            }
            let mut sig = format!("{}({})", tool.name, params.join(", "));
            if let Some(desc) = &tool.description {
                sig.push_str(" // ");
                sig.push_str(desc);
            }
            tool_lines.push(sig);
        }
        sections.push(format!(
            "# Tools Available\nTools are used when external data access or state mutation is required.\n\n{}",
            tool_lines.join("\n")
        ));
    }

    // Workflows
    if !ir.workflows.is_empty() {
        let mut wf_lines = Vec::new();
        for wf in &ir.workflows {
            let mut params = Vec::new();
            if let Some(obj) = wf.params.as_object() {
                for (name, def) in obj {
                    let field_type = def["type"].as_str().unwrap_or("any");
                    params.push(format!("{}: {}", name, field_type));
                }
            }
            let mut sig = format!("{}({})", wf.name, params.join(", "));
            if let Some(desc) = &wf.description {
                sig.push_str(" // ");
                sig.push_str(desc);
            }
            wf_lines.push(sig);
        }
        sections.push(format!(
            "# Workflows Available\nWorkflows are used for deterministic or optimized operations.\n\n{}",
            wf_lines.join("\n")
        ));
    }

    // Helpers
    if !ir.helpers.is_empty() {
        let mut helper_lines = Vec::new();
        for helper in &ir.helpers {
            let mut params = Vec::new();
            if let Some(input) = &helper.input {
                if let Some(obj) = input.as_object() {
                    for (name, def) in obj {
                        let field_type = def["type"].as_str().unwrap_or("any");
                        params.push(format!("{}: {}", name, field_type));
                    }
                }
            }
            let mut sig = format!("{}({})", helper.name, params.join(", "));
            if let Some(desc) = &helper.description {
                sig.push_str(" // ");
                sig.push_str(desc);
            }
            helper_lines.push(sig);
        }
        sections.push(format!(
            "# Helpers Available\nHelpers delegate subtasks to specialized agents.\n\n{}",
            helper_lines.join("\n")
        ));
    }

    // ── Options (the YAML structures the model can select) ────────────────

    sections.push(
        "# Instructions\n\
         Your response MUST be a valid YAML block — not JSON, not plain text.\n\
         Use YAML key: value syntax on every turn, including after receiving a tool_result.\n\
         Never wrap your output in code fences or markdown.\n\
         "
        .to_string(),
    );

    let mut options: Vec<String> = Vec::new();

    // response_schema or response_text
    if let Some(output) = &ir.output {
        if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
            // Union output: multiple response schema variants
            // 1. Generate standalone reference section
            let mut variant_lines = Vec::new();
            let mut variant_names = Vec::new();

            for (variant_name, variant_schema) in variants {
                variant_names.push(variant_name.clone());
                let schema_str = schema::format_schema_yaml(variant_schema, 2, ir.types.as_ref());
                variant_lines.push(format!("{}\n{}", variant_name, schema_str));
            }

            sections.push(format!(
                "# Response Schemas Available\nUsed for replying directly to the user.\n\n{}",
                variant_lines.join("\n\n")
            ));

            // 2. Generate option with type discriminator
            let variant_union = if variant_names.len() > 1 {
                format!("< {} >", variant_names.join(" | "))
            } else {
                variant_names[0].clone()
            };
            options.push(format!("response_schema:\n  type: {}", variant_union));
        } else if matches!(output.as_object(), Some(obj) if !obj.is_empty()) {
            // Single output type (current behavior)
            let schema_str = schema::format_schema_yaml(output, 2, ir.types.as_ref());
            options.push(format!("response_schema:\n{}", schema_str));
        } else {
            options.push("response_text:\n  text: string".to_string());
        }
    } else {
        options.push("response_text:\n  text: string".to_string());
    }

    // tool_call
    if !ir.tools.is_empty() {
        let tool_names: Vec<String> = ir.tools.iter().map(|t| t.name.clone()).collect();
        let tool_union = if tool_names.len() > 1 {
            format!("< {} >", tool_names.join(" | "))
        } else {
            tool_names[0].clone()
        };
        options.push(format!(
            "tool_call:\n  type: {}\n  args: {{ key: value }}",
            tool_union
        ));
    }

    // workflow_call
    if !ir.workflows.is_empty() {
        let wf_names: Vec<String> = ir.workflows.iter().map(|w| w.name.clone()).collect();
        let wf_union = if wf_names.len() > 1 {
            format!("< {} >", wf_names.join(" | "))
        } else {
            wf_names[0].clone()
        };
        options.push(format!(
            "workflow_call:\n  type: {}\n  args: {{ key: value }}",
            wf_union
        ));
    }

    // helper_call
    if !ir.helpers.is_empty() {
        let helper_names: Vec<String> = ir.helpers.iter().map(|h| h.name.clone()).collect();
        let helper_union = if helper_names.len() > 1 {
            format!("< {} >", helper_names.join(" | "))
        } else {
            helper_names[0].clone()
        };
        options.push(format!(
            "helper_call:\n  type: {}\n  args: {{ key: value }}",
            helper_union
        ));
    }

    sections.push(format!("# Options\n{}", options.join("\n\n")));

    sections.join("\n\n")
}

pub fn generate_helper_intents(ir: &AgentIR, helper_name: &str) -> String {
    let mut sections = Vec::new();

    // Determine allowed tools
    let mut allowed_tools = Vec::new();
    if let Some(grants) = &ir.helper_tool_grants {
        if let Some(grant) = grants.get(helper_name) {
            match grant {
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

    // Things to Know (helper variant)
    let mut know_items: Vec<String> = Vec::new();
    know_items.push(
        "Intents are actions you perform. Each response must be a YAML block using one or more intent."
            .to_string(),
    );
    if !allowed_tools.is_empty() {
        know_items.push(
            "A `tool_call` intent invokes an external tool. \
             After a tool executes, you will receive a `tool_result` — use it to respond."
                .to_string(),
        );
    }
    know_items.push("A `response_text` intent sends a plain text reply.".to_string());

    let know_lines: Vec<String> = know_items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect();
    sections.push(format!("# Things to Know\n{}", know_lines.join("\n")));

    // Tools (filtered)
    if !allowed_tools.is_empty() {
        let mut tool_lines = Vec::new();
        for tool in &allowed_tools {
            let mut params = Vec::new();
            if let Some(obj) = tool.params.as_object() {
                for (name, def) in obj {
                    let field_type = def["type"].as_str().unwrap_or("any");
                    params.push(format!("{}: {}", name, field_type));
                }
            }
            let mut sig = format!("{}({})", tool.name, params.join(", "));
            if let Some(desc) = &tool.description {
                sig.push_str(" // ");
                sig.push_str(desc);
            }
            tool_lines.push(sig);
        }
        sections.push(format!(
            "# Tools Available\nTools are used when external data access or state mutation is required.\n\n{}",
            tool_lines.join("\n")
        ));
    }

    // Instructions + Options
    sections.push(
        "# Instructions\nRespond ONLY with a valid YAML block. Do not include any conversational text or explanation."
            .to_string(),
    );

    let mut options: Vec<String> = Vec::new();
    options.push("response_text:\n  text: string".to_string());

    if !allowed_tools.is_empty() {
        let tool_names: Vec<String> = allowed_tools.iter().map(|t| t.name.clone()).collect();
        let tool_union = if tool_names.len() > 1 {
            format!("< {} >", tool_names.join(" | "))
        } else {
            tool_names[0].clone()
        };
        options.push(format!(
            "tool_call:\n  type: {}\n  args: {{ key: value }}",
            tool_union
        ));
    }

    sections.push(format!("# Options\n{}", options.join("\n\n")));

    sections.join("\n\n")
}
