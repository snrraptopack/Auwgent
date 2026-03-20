use crate::schema;
use crate::types::AgentIR;

pub fn generate_intents(ir: &AgentIR) -> String {
    let mut sections = Vec::new();

    // ── Dynamic "# Things to Know" section ────────────────────────────────
    // Explains the intent system contextually based on what capabilities
    // the agent actually has (tools, workflows, response schemas, etc.)
    let mut know_items: Vec<String> = Vec::new();

    know_items.push(
        "Intents are actions you perform. Each response must be a valid Function Composition block using one or more intent."
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

    // Only mention response_text if the agent has no structured output schema
    let has_output_schema = ir
        .output
        .as_ref()
        .map(|o| matches!(o.as_object(), Some(obj) if !obj.is_empty()))
        .unwrap_or(false);
    if !has_output_schema {
        know_items
            .push("A `response_text` intent sends a plain text reply to the user.".to_string());
    }

    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let desc = ci.description.as_deref().unwrap_or("User-defined intent.");
            know_items.push(format!("A `{}` intent {}.", ci.name, desc));
        }
    }

    // Only mention tool_result follow-up if the agent can actually call tools/workflows/helpers
    let has_callables = !ir.tools.is_empty() || !ir.workflows.is_empty() || !ir.helpers.is_empty();
    if has_callables {
        let callable_options: Vec<&str> = [
            ir.output
                .as_ref()
                .filter(|o| matches!(o.as_object(), Some(obj) if !obj.is_empty()))
                .map(|_| "response_schema"),
            if !has_output_schema {
                Some("response_text")
            } else {
                None
            },
            if !ir.tools.is_empty() {
                Some("tool_call")
            } else {
                None
            },
            if !ir.workflows.is_empty() {
                Some("workflow_call")
            } else {
                None
            },
            if !ir.helpers.is_empty() {
                Some("helper_call")
            } else {
                None
            },
        ]
        .iter()
        .filter_map(|o| *o)
        .collect();

        let options_str = if callable_options.len() == 1 {
            format!("`{}`", callable_options[0])
        } else {
            let last = callable_options.last().unwrap();
            let rest: Vec<_> = callable_options[..callable_options.len() - 1]
                .iter()
                .map(|s| format!("`{}`", s))
                .collect();
            format!("{} or `{}`", rest.join(", "), last)
        };

        know_items.push(format!(
            "When you receive a `tool_result`, you MUST respond with another intent ({}).",
            options_str
        ));
    }

    let know_lines: Vec<String> = know_items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect();

    sections.push(format!("# Things to Know\n{}", know_lines.join("\n")));

    // ── Reference sections (tools, workflows, helpers, response schema) ──

    // Tools
    let mut expanded_tools_str = String::new();
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
        expanded_tools_str = format!(
            "# Tools Available\nExternal tools you can call. You MUST ONLY use tools from this exact list.\n\n{}",
            tool_lines.join("\n")
        );
    }

    // Workflows
    let mut expanded_wf_str = String::new();
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
        expanded_wf_str = format!(
            "# Workflows Available\nDeterministic workflows you can execute. You MUST ONLY use workflows from this exact list.\n\n{}",
            wf_lines.join("\n")
        );
    }

    // Helpers
    let mut expanded_helpers_str = String::new();
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
        expanded_helpers_str = format!(
            "# Helpers Available\nDelegates subtasks to specialized agents. You MUST ONLY use helpers from this exact list.\n\n{}",
            helper_lines.join("\n")
        );
    }

    let mut options: Vec<String> = Vec::new();

    // response_schema or response_text
    if let Some(output) = &ir.output {
        if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
            let mut variant_names = Vec::new();
            for (variant_name, _variant_schema) in variants {
                variant_names.push(variant_name.clone());
            }
            let variant_union = if variant_names.len() > 1 {
                format!("< {} >", variant_names.join(" | "))
            } else {
                variant_names[0].clone()
            };
            options.push(format!("// Respond to the user using one of the available schema variants\nresponse_schema(\n  type = \"{}\"\n  response = {{ /* match the requested schema's shape */ }}\n)", variant_union));
        } else if matches!(output.as_object(), Some(obj) if !obj.is_empty()) {
            let schema_str = schema::format_schema_function(output, 2, ir.types.as_ref());
            options.push(format!("// Respond to the user using the structured schema\nresponse_schema(\n{}\n)", schema_str));
        } else {
            options.push("// Respond to the user with plain text\nresponse_text(\n  text = \"string\"\n)".to_string());
        }
    } else {
        options.push("// Respond to the user with plain text\nresponse_text(\n  text = \"string\"\n)".to_string());
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
            "// Call a registered tool. See exact arguments below.\ntool_call(\n  type = \"{}\"\n  args = {{ /* match the requested tool's exact shape */ }}\n)\n\n{}",
            tool_union, expanded_tools_str
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
            "// Execute a predefined workflow. See exact arguments below.\nworkflow_call(\n  type = \"{}\"\n  args = {{ /* match the requested workflow's exact shape */ }}\n)\n\n{}",
            wf_union, expanded_wf_str
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
            "// Delegate a task to a specialized helper. See exact arguments below.\nhelper_call(\n  type = \"{}\"\n  args = {{ /* match the requested helper's exact shape */ }}\n)\n\n{}",
            helper_union, expanded_helpers_str
        ));
    }

    // Custom Intents
    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let schema_str = schema::format_schema_function(&ci.fields, 2, ir.types.as_ref());
            let mut ci_str = String::new();
            if let Some(desc) = &ci.description {
                ci_str.push_str(&format!("// {}\n", desc));
            }
            ci_str.push_str(&format!("{}(\n{}\n)", ci.name, schema_str));
            options.push(ci_str);
        }
    }

    sections.push(
        "# Instructions\n\
         Your response MUST ONLY be valid Function Composition blocks. ABSOLUTELY NO conversational text or explanations outside of a function block.\n\
         What is a Function Composition block? It is a structured format using intents as function calls. For example:\n\
         intent_name(\n\
           field_name = \"value\"\n\
         )\n\
         \n\
         CRITICAL RULE: You MUST NEVER assume or hallucinate a tool, helper, workflow, or intent that is not explicitly listed in the available options. If it is not listed in the sections above, IT DOES NOT EXIST. If a user asks you to use an unknown tool/helper, you must politely decline and explain what capabilities you actually have.\n\
         CRITICAL RULE: You are an intelligent AI. You must ALWAYS use your internal reasoning and knowledge to answer questions or converse directly with the user via `response_text` when possible. You should ONLY call specialized tools or helpers if the user explicitly requests a task that requires them.\n\
         \n\
         String values MUST be wrapped in double quotes. Multiline strings are supported natively inside quotes.\n\
         Keys and fields are assigned using `=`. Nested objects use `{}`.\n\
         For example, to execute an action, you MUST use a function:\n\
         obey(\n\
           action = \"This is my action payload.\nIt can span multiple lines.\"\n\
         )\n\
         \n\
         Never wrap your output in code fences or markdown. START IMMEDIATELY with a function name.\n\
         "
        .to_string(),
    );

    sections.push(format!("# Available Actions (Options)\nYou MUST select and output one or more of the following function blocks to perform your turn:\n\n{}", options.join("\n\n")));

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
        "Intents are actions you perform. Each response must be a valid Function Composition block using one or more intent."
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

    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let desc = ci.description.as_deref().unwrap_or("User-defined intent.");
            know_items.push(format!("A `{}` intent {}.", ci.name, desc));
        }
    }

    let know_lines: Vec<String> = know_items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect();
    sections.push(format!("# Things to Know\n{}", know_lines.join("\n")));

    // Tools (filtered)
    let mut expanded_tools_str = String::new();
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
        expanded_tools_str = format!(
            "# Tools Available\nExternal tools you can call. You MUST ONLY use tools from this exact list.\n\n{}",
            tool_lines.join("\n")
        );
    }

    let mut options: Vec<String> = Vec::new();
    options.push("// Respond to the user with plain text\nresponse_text(\n  text = \"string\"\n)".to_string());

    if !allowed_tools.is_empty() {
        let tool_names: Vec<String> = allowed_tools.iter().map(|t| t.name.clone()).collect();
        let tool_union = if tool_names.len() > 1 {
            format!("< {} >", tool_names.join(" | "))
        } else {
            tool_names[0].clone()
        };
        options.push(format!(
            "// Call a registered tool. See exact arguments below.\ntool_call(\n  type = \"{}\"\n  args = {{ /* match the requested tool's exact shape */ }}\n)\n\n{}",
            tool_union, expanded_tools_str
        ));
    }

    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            let schema_str = schema::format_schema_function(&ci.fields, 2, ir.types.as_ref());
            let mut ci_str = String::new();
            if let Some(desc) = &ci.description {
                ci_str.push_str(&format!("// {}\n", desc));
            }
            ci_str.push_str(&format!("{}(\n{}\n)", ci.name, schema_str));
            options.push(ci_str);
        }
    }

    sections.push(
        "# Instructions\n\
         Your response MUST ONLY be valid Function Composition blocks. ABSOLUTELY NO conversational text or explanations outside of a function block.\n\
         What is a Function Composition block? It is a structured format using intents as function calls. For example:\n\
         intent_name(\n\
           field_name = \"value\"\n\
         )\n\
         \n\
         CRITICAL RULE: You MUST NEVER assume or hallucinate a tool, helper, workflow, or intent that is not explicitly listed in the available options. If it is not listed in the sections above, IT DOES NOT EXIST. If a user asks you to use an unknown tool/helper, you must politely decline and explain what capabilities you actually have.\n\
         CRITICAL RULE: You are an intelligent AI. You must ALWAYS use your internal reasoning and knowledge to answer questions or converse directly with the user via `response_text` when possible. You should ONLY call specialized tools or helpers if the user explicitly requests a task that requires them.\n\
         String values MUST be wrapped in double quotes. Multiline strings are supported natively inside quotes.\n\
         Keys and fields are assigned using `=`. Nested objects use `{}`.\n\
         For example, to execute an action, you MUST use a function:\n\
         obey(\n\
           action = \"This is my action payload.\nIt can span multiple lines.\"\n\
         )\n\
         \n\
         Never wrap your output in code fences or markdown. START IMMEDIATELY with a function name.\n\
         "
        .to_string(),
    );

    sections.push(format!("# Available Actions (Options)\nYou MUST select and output one or more of the following function blocks to perform your turn:\n\n{}", options.join("\n\n")));

    sections.join("\n\n")
}
