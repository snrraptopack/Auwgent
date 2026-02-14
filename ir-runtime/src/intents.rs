use crate::schema;
use crate::types::AgentIR;

pub fn generate_intents(ir: &AgentIR) -> String {
    let mut sections = Vec::new();

    sections.push("# Instructions\nRespond ONLY with a valid YAML block that matches exactly one  or more of the # Option structures defined below. Do not include any conversational text or explanation.".to_string());

    // 1. # tool available
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
        sections.push(format!("# tool available\n{}", tool_lines.join("\n")));
    }

    // 2. # workflow available
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
        sections.push(format!("# workflow available\n{}", wf_lines.join("\n")));
    }

    // 3. # helper available
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
        sections.push(format!("# helper available\n{}", helper_lines.join("\n")));
    }

    // 3. Options as standalone # Option sections
    if let Some(output) = &ir.output {
        if matches!(output.as_object(), Some(obj) if !obj.is_empty()) {
            let schema_str = schema::format_schema_yaml(output, 2, ir.types.as_ref());
            sections.push(format!("# Option\nresponse_schema:\n{}", schema_str));
        } else {
            sections.push("# Option\nresponse_text:\n  text: string".to_string());
        }
    } else {
        sections.push("# Option\nresponse_text:\n  text: string".to_string());
    }

    if !ir.tools.is_empty() {
        let tool_names: Vec<String> = ir.tools.iter().map(|t| t.name.clone()).collect();
        let tool_union = if tool_names.len() > 1 {
            format!("< {} >", tool_names.join(" | "))
        } else {
            tool_names[0].clone()
        };

        sections.push(format!(
            "# Option\ntool_call:\n  type: {}\n  args: {{ key: value }}",
            tool_union
        ));
    }

    if !ir.workflows.is_empty() {
        let wf_names: Vec<String> = ir.workflows.iter().map(|w| w.name.clone()).collect();
        let wf_union = if wf_names.len() > 1 {
            format!("< {} >", wf_names.join(" | "))
        } else {
            wf_names[0].clone()
        };

        sections.push(format!(
            "# Option\nworkflow_call:\n  type: {}\n  args: {{ key: value }}",
            wf_union
        ));
    }

    if !ir.helpers.is_empty() {
        let helper_names: Vec<String> = ir.helpers.iter().map(|h| h.name.clone()).collect();
        let helper_union = if helper_names.len() > 1 {
            format!("< {} >", helper_names.join(" | "))
        } else {
            helper_names[0].clone()
        };

        sections.push(format!(
            "# Option\nhelper_call:\n  type: {}\n  args: {{ key: value }}",
            helper_union
        ));
    }

    sections.join("\n\n")
}

pub fn generate_helper_intents(ir: &AgentIR, helper_name: &str) -> String {
    let mut sections = Vec::new();

    sections.push("# Instructions\nRespond ONLY with a valid YAML block that matches exactly one of the # Option structures defined below. Do not include any conversational text or explanation.".to_string());

    // Determine allowed tools
    let mut allowed_tools = Vec::new();
    if let Some(grants) = &ir.helper_tool_grants {
        if let Some(grant) = grants.get(helper_name) {
            match grant {
                serde_json::Value::String(s) if s == "all" => {
                    // Allow all tools
                    allowed_tools = ir.tools.clone();
                }
                serde_json::Value::Array(arr) => {
                    // Filter tools by name
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
                _ => {} // Invalid format, allow none
            }
        }
    }

    // 1. # tool available (filtered)
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
        sections.push(format!("# tool available\n{}", tool_lines.join("\n")));
    }

    // 3. Options
    // Since this is a helper, we assume it can respond with text or call allowed tools.
    // We don't have explicit output schema for helpers in the prompt yet, but we can default to text.
    sections.push("# Option\nresponse_text:\n  text: string".to_string());

    if !allowed_tools.is_empty() {
        let tool_names: Vec<String> = allowed_tools.iter().map(|t| t.name.clone()).collect();
        let tool_union = if tool_names.len() > 1 {
            format!("< {} >", tool_names.join(" | "))
        } else {
            tool_names[0].clone()
        };

        sections.push(format!(
            "# Option\ntool_call:\n  type: {}\n  args: {{ key: value }}",
            tool_union
        ));
    }

    sections.join("\n\n")
}
