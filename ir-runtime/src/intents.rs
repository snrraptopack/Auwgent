use crate::schema;
use crate::types::{AgentIR, TypeDefinition};
use serde_json::Value;
use std::collections::HashMap;

fn unwrap_schema_properties(val: &Value) -> Option<&serde_json::Map<String, Value>> {
    if let Some(obj) = val.as_object() {
        if obj.contains_key("properties") {
            return obj.get("properties").and_then(|p| p.as_object());
        }
        if let Some(inner_type) = obj.get("type") {
            if inner_type.is_object() {
                return unwrap_schema_properties(inner_type);
            }
        }
    }
    val.as_object()
}

// ═══════════════════════════════════════════════════════════════════════════
// BLOCK PROTOCOL PROMPT GENERATION
// ═══════════════════════════════════════════════════════════════════════════

/// Generate system prompt using the @@block protocol format.
/// This is the new format that replaces function-style intents.
pub fn generate_block_protocol_prompt(ir: &AgentIR) -> String {
    let mut sections = Vec::new();

    // ═══ HEADER ═══
    sections.push(
        "You are an execution engine. You communicate EXCLUSIVELY using the `@@` block protocol.\n\n\
         # AVAILABLE BLOCKS\n\n\
         You may ONLY use the following block types:".to_string()
    );

    // ═══ @@chat BLOCK ═══
    sections.push(
        "\n\n@@chat\n\
         Use this to when you are responding to the user directly\n\
         @@end"
            .to_string(),
    );

    // ═══ @@tool BLOCK ═══
    if !ir.tools.is_empty() {
        let mut tool_section = String::from(
            "\n\n@@tool\nUse this to execute parallel tools.\nAvailable tools and their exact arguments:\n",
        );

        for tool in &ir.tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!("- {}({})", tool.name, params));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }

        // Collect examples from tools that have them
        let example_calls = collect_tool_example_calls(&ir.tools);
        if !example_calls.is_empty() {
            tool_section.push_str("\nExample:\n@@tool\n");
            tool_section.push_str(&example_calls.join("\n"));
            tool_section.push_str("\n@@end");
        }

        sections.push(tool_section);
    }

    // ═══ @@workflow BLOCK ═══
    if !ir.workflows.is_empty() {
        let mut wf_section = String::from(
            "\n\n@@workflow\nUse this to execute a single, sequential backend workflow.\nAvailable workflows:\n",
        );

        for wf in &ir.workflows {
            let params = format_params_signature(&wf.params.0, ir.types.as_ref());
            wf_section.push_str(&format!("- {}({})", wf.name, params));
            if let Some(desc) = &wf.description {
                wf_section.push_str(&format!(" // {}", desc));
            }
            wf_section.push('\n');
        }

        // Collect examples
        let example_calls = collect_workflow_example_calls(&ir.workflows);
        if !example_calls.is_empty() {
            wf_section.push_str("\nExample:\n@@workflow\n");
            wf_section.push_str(&example_calls[0]); // Only one workflow per block
            wf_section.push_str("\n@@end");
        }

        sections.push(wf_section);
    }

    // ═══ @@out BLOCK ═══
    if let Some(output) = &ir.output {
        if let Some(obj) = output.0.as_object() {
            if !obj.is_empty() {
                let mut schema_section = String::from(
                    "\n\n@@out [SchemaName]\nUse this to return structured data to the system.\nAvailable schemas and their exact shapes:\n\n",
                );

                // Format schema structure (TypeScript-style)
                schema_section.push_str(&format_output_schema_ts_style(&output.0, ir));

                // Add examples
                if let Some(examples) = output.0.get("@examples").and_then(|v| v.as_array()) {
                    if !examples.is_empty() {
                        let ex = &examples[0];
                        let schema_name = ex
                            .get("__schema_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Output");

                        schema_section.push_str(&format!("\nExample:\n@@out {}\n", schema_name));
                        schema_section.push_str(&format_ts_object_for_example(ex));
                        schema_section.push_str("\n@@end");
                    }
                }

                sections.push(schema_section);
            }
        }
    }

    // ═══ @@helper BLOCK ═══
    if !ir.helpers.is_empty() {
        let mut helper_section = String::from(
            "\n\n@@helper\nUse this to delegate to a specialized sub-agent.\nAvailable helpers:\n",
        );

        for helper in &ir.helpers {
            let params = format_helper_params_signature(
                helper.input.as_ref().map(|v| &v.0),
                ir.types.as_ref(),
            );
            helper_section.push_str(&format!("- {}({})", helper.name, params));
            if let Some(desc) = &helper.description {
                helper_section.push_str(&format!(" // {}", desc));
            }
            helper_section.push('\n');
        }

        // Add examples
        let example_calls = collect_helper_example_calls(&ir.helpers);
        if !example_calls.is_empty() {
            helper_section.push_str("\nExample:\n@@helper\n");
            helper_section.push_str(&example_calls[0]); // Only one helper per block
            helper_section.push_str("\n@@end");
        }

        sections.push(helper_section);
    }

    // ═══ CUSTOM INTENTS ═══
    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            // Format: @@IntentName
            // Fields: param1 = type1, param2 = type2
            let mut custom_section = format!("\n\n@@{}\n", ci.name);

            if let Some(desc) = &ci.description {
                custom_section.push_str(&format!("{}\n", desc));
            }

            // Add field signature as inline format
            let params_sig = format_params_signature(&ci.fields.0, ir.types.as_ref());
            if !params_sig.is_empty() {
                custom_section.push_str(&format!("Fields: {}\n", params_sig));
            }

            // Add examples
            if !ci.examples.is_empty() {
                custom_section.push_str(&format!("\nExample:\n@@{}\n", ci.name));
                custom_section.push_str(&format_custom_intent_example_inline(&ci.examples[0].0));
                custom_section.push_str("\n@@end");
            }

            sections.push(custom_section);
        }
    }

    // ═══ CONSTRAINTS ═══
    let mut constraints = Vec::new();
    constraints.push("- NEVER invent blocks that are not listed above.".to_string());

    if !ir.tools.is_empty() {
        constraints.push(
            "- Your `@@tool` arguments must STRICTLY match the types and shapes provided."
                .to_string(),
        );
    }

    if let Some(output) = &ir.output {
        if let Some(obj) = output.0.as_object() {
            if !obj.is_empty() {
                constraints.push(
                    "- Your `@@out` JSON must STRICTLY match the schema shape provided."
                        .to_string(),
                );
                constraints.push("- Do not add properties to `@@out` objects that are not defined in the schema shape.".to_string());
            }
        }
    }

    if !ir.workflows.is_empty() {
        constraints.push(
            "- Your `@@workflow` arguments must STRICTLY match the types provided.".to_string(),
        );
    }

    if !ir.helpers.is_empty() {
        constraints.push(
            "- Your `@@helper` arguments must STRICTLY match the types provided.".to_string(),
        );
    }

    constraints.push(
        "- You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat)."
            .to_string(),
    );
    constraints.push("- Blocks auto-close when you start a new block, but using @@end is recommended for clarity.".to_string());

    sections.push(format!(
        "\n\n# CRITICAL CONSTRAINTS\n\n{}",
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

    // ═══ HEADER ═══
    sections.push(
        "You are a specialized helper agent. You communicate EXCLUSIVELY using the `@@` block protocol.\n\n\
         # AVAILABLE BLOCKS\n\n\
         You may ONLY use the following block types:".to_string()
    );

    // ═══ @@chat BLOCK ═══
    sections.push(
        "\n\n@@chat\n\
         Use this to speak to the user, explain your actions, or think out loud.\n\
         @@end"
            .to_string(),
    );

    // ═══ @@tool BLOCK (filtered) ═══
    if !allowed_tools.is_empty() {
        let mut tool_section = String::from(
            "\n\n@@tool\nUse this to execute parallel tools.\nAvailable tools and their exact arguments:\n",
        );

        for tool in &allowed_tools {
            let params = format_params_signature(&tool.params.0, ir.types.as_ref());
            tool_section.push_str(&format!("- {}({})", tool.name, params));
            if let Some(desc) = &tool.description {
                tool_section.push_str(&format!(" // {}", desc));
            }
            tool_section.push('\n');
        }

        // Collect examples
        let example_calls = collect_tool_example_calls(&allowed_tools);
        if !example_calls.is_empty() {
            tool_section.push_str("\nExample:\n@@tool\n");
            tool_section.push_str(&example_calls.join("\n"));
            tool_section.push_str("\n@@end");
        }

        sections.push(tool_section);
    }

    // ═══ CUSTOM INTENTS ═══
    if let Some(custom) = &ir.custom_intents {
        for ci in custom {
            // Format: @@IntentName
            // Fields: param1 = type1, param2 = type2
            let mut custom_section = format!("\n\n@@{}\n", ci.name);

            if let Some(desc) = &ci.description {
                custom_section.push_str(&format!("{}\n", desc));
            }

            // Add field signature as inline format
            let params_sig = format_params_signature(&ci.fields.0, ir.types.as_ref());
            if !params_sig.is_empty() {
                custom_section.push_str(&format!("Fields: {}\n", params_sig));
            }

            // Add examples
            if !ci.examples.is_empty() {
                custom_section.push_str(&format!("\nExample:\n@@{}\n", ci.name));
                custom_section.push_str(&format_custom_intent_example_inline(&ci.examples[0].0));
                custom_section.push_str("\n@@end");
            }

            sections.push(custom_section);
        }
    }

    // ═══ CONSTRAINTS ═══
    let mut constraints = Vec::new();
    constraints.push("- NEVER invent blocks that are not listed above.".to_string());

    if !allowed_tools.is_empty() {
        constraints.push(
            "- Your `@@tool` arguments must STRICTLY match the types and shapes provided."
                .to_string(),
        );
    }

    constraints.push(
        "- You can use multiple blocks in one response (e.g., @@chat then @@tool then @@chat)."
            .to_string(),
    );
    constraints.push("- Blocks auto-close when you start a new block, but using @@end is recommended for clarity.".to_string());

    sections.push(format!(
        "\n\n# CRITICAL CONSTRAINTS\n\n{}",
        constraints.join("\n")
    ));

    sections.join("")
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS FOR BLOCK PROTOCOL FORMATTING
// ═══════════════════════════════════════════════════════════════════════════

/// Format parameter signature like: session_id: string, apply_compression?: boolean
fn format_params_signature(
    params: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    if let Some(obj) = params.as_object() {
        let mut parts = Vec::new();
        for (name, def) in obj {
            let type_str = schema::format_type_value(def, types);
            let optional = def
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if optional {
                parts.push(format!("{}?: {}", name, type_str));
            } else {
                parts.push(format!("{}: {}", name, type_str));
            }
        }
        parts.join(", ")
    } else {
        String::new()
    }
}

/// Format helper parameter signature from input IR
fn format_helper_params_signature(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    if let Some(input) = input_ir {
        if input.get("kind").and_then(|v| v.as_str()) == Some("properties") {
            if let Some(fields) = input.get("fields").and_then(|v| v.as_object()) {
                let mut parts = Vec::new();
                let mut sorted_keys: Vec<_> = fields.keys().collect();
                sorted_keys.sort();
                for name in sorted_keys {
                    let def = &fields[name];
                    let type_str = schema::format_type_value(def, types);
                    let optional = def
                        .get("optional")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if optional {
                        parts.push(format!("{}?: {}", name, type_str));
                    } else {
                        parts.push(format!("{}: {}", name, type_str));
                    }
                }
                return parts.join(", ");
            }
        } else if input.get("kind").and_then(|v| v.as_str()) == Some("direct") {
            if let Some(ty) = input.get("type") {
                // Check if the direct type is an object with properties
                if ty.get("type").and_then(|v| v.as_str()) == Some("object") {
                    if let Some(props) = ty.get("properties").and_then(|v| v.as_object()) {
                        // Expand object properties instead of just showing "object"
                        let mut parts = Vec::new();
                        let mut sorted_keys: Vec<_> = props.keys().collect();
                        sorted_keys.sort();
                        for name in sorted_keys {
                            let def = &props[name];
                            let type_str = schema::format_type_value(def, types);
                            let optional = def
                                .get("optional")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            if optional {
                                parts.push(format!("{}?: {}", name, type_str));
                            } else {
                                parts.push(format!("{}: {}", name, type_str));
                            }
                        }
                        return parts.join(", ");
                    }
                }
                // Otherwise format as simple type
                let type_str = schema::format_type_value(ty, types);
                return format!("input: {}", type_str);
            }
        }
    }
    String::new()
}

/// Collect example function calls from tools
fn collect_tool_example_calls(tools: &[crate::types::Tool]) -> Vec<String> {
    let mut calls = Vec::new();

    for tool in tools {
        if !tool.examples.is_empty() {
            // Each example is an array of argument values
            if let Some(first_example) = tool.examples.first() {
                calls.push(format_function_call_from_args(&tool.name, &first_example.0));
                if calls.len() >= 5 {
                    break; // Show max 5 examples in the block example
                }
            }
        }
    }

    calls
}

/// Collect example function calls from workflows
fn collect_workflow_example_calls(workflows: &[crate::types::Workflow]) -> Vec<String> {
    let mut calls = Vec::new();

    for wf in workflows {
        if !wf.examples.is_empty() {
            if let Some(first_example) = wf.examples.first() {
                calls.push(format_function_call_from_args(&wf.name, &first_example.0));
                break; // Only one workflow per block
            }
        }
    }

    calls
}

/// Collect example function calls from helpers
fn collect_helper_example_calls(helpers: &[crate::types::Helper]) -> Vec<String> {
    let mut calls = Vec::new();

    for helper in helpers {
        if !helper.examples.is_empty() {
            if let Some(first_example) = helper.examples.first() {
                calls.push(format_function_call_from_args(
                    &helper.name,
                    &first_example.0,
                ));
                break; // Only one helper per block
            }
        }
    }

    calls
}

/// Format a function call from a HashMap of named arguments: ama(id = "20") or fetch_session(session_id = "sess_123")
fn format_function_call_from_args(name: &str, args: &serde_json::Value) -> String {
    if args.as_object().map_or(true, |o| o.is_empty()) {
        return format!("{}()", name);
    }

    // Format each named argument as key = value
    // Note: args values are IR expressions like {"type": "literal", "value": 20}
    let default_map = serde_json::Map::new();
    let obj_map = args.as_object().unwrap_or(&default_map);
    let mut formatted_args: Vec<String> = obj_map
        .iter()
        .map(|(key, val): (&String, &Value)| {
            format!("{} = {}", key, format_value_inline(val))
        })
        .collect();

    // Sort for consistent output
    formatted_args.sort();

    format!("{}({})", name, formatted_args.join(", "))
}

/// Format a value inline for function arguments
fn format_value_inline(val: &Value) -> String {
    // 1. Unwrap IR Expression if it's one
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
            for (k, v) in obj {
                if k.starts_with("__") {
                    continue; // Skip metadata fields
                }
                fields.push(format!("{} = {}", k, format_value_inline(v)));
            }
            format!("{{ {} }}", fields.join(", "))
        }
    }
}

/// Format output schema in TypeScript style
fn format_output_schema_ts_style(output: &Value, ir: &AgentIR) -> String {
    if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
        let mut variant_lines = Vec::new();
        for (variant_name, variant_schema) in variants {
            variant_lines.push(format!(
                "{} {{\n{}\n}}",
                variant_name,
                format_ts_object_fields(variant_schema, 2, ir.types.as_ref())
            ));
        }
        variant_lines.join("\n\n")
    } else if let Some(obj) = unwrap_schema_properties(output) {
        format!(
            "Output {{\n{}\n}}",
            format_ts_object_fields(&Value::Object(obj.clone()), 2, ir.types.as_ref())
        )
    } else {
        "{}".to_string()
    }
}

/// Format TypeScript object fields with proper indentation
fn format_ts_object_fields(
    schema: &Value,
    indent_level: usize,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let indent = "  ".repeat(indent_level / 2);
    let mut lines = Vec::new();

    if let Some(obj) = unwrap_schema_properties(schema) {
        for (name, def) in obj {
            let is_optional = def
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let name_tag = if is_optional {
                format!("{}?", name)
            } else {
                name.clone()
            };

            // Check if this is a typeRef that needs expansion
            let needs_expansion =
                if let Some(type_obj) = def.get("type").and_then(|t| t.as_object()) {
                    type_obj.get("type").and_then(|t| t.as_str()) == Some("typeRef")
                } else {
                    false
                };

            if needs_expansion {
                // Expand the typeRef inline
                if let Some(type_obj) = def.get("type").and_then(|t| t.as_object()) {
                    if let Some(ref_name) = type_obj.get("name").and_then(|n| n.as_str()) {
                        if let Some(types_map) = types {
                            if let Some(custom_type) = types_map.get(ref_name) {
                                // Recursively expand the referenced type
                                lines.push(format!("{}{}: {{", indent, name_tag));

                                // Convert TypeDefinition properties to Value for recursive formatting
                                if let Ok(props_value) =
                                    serde_json::to_value(&custom_type.properties)
                                {
                                    let nested_fields = format_ts_object_fields(
                                        &props_value,
                                        indent_level + 2,
                                        types,
                                    );
                                    lines.push(nested_fields);
                                }

                                lines.push(format!("{}}};", indent));
                                continue;
                            }
                        }
                    }
                }
            }

            // Normal field formatting
            let field_type = schema::format_type_value(def, types);
            let mut line = format!("{}{}: {};", indent, name_tag, field_type);

            if let Some(desc) = def.get("description").and_then(|d| d.as_str()) {
                line.push_str(&format!(" // {}", desc));
            }
            lines.push(line);
        }
    }

    lines.join("\n")
}

/// Format a TypeScript object for examples (used in @@out blocks)
fn format_ts_object_for_example(val: &Value) -> String {
    if let Some(obj) = val.as_object() {
        let mut lines = Vec::new();
        lines.push("{".to_string());

        for (key, value) in obj {
            if key.starts_with("__") {
                continue; // Skip metadata fields
            }
            lines.push(format!("  {}: {}", key, format_value_multiline(value, 1)));
        }

        lines.push("}".to_string());
        lines.join("\n")
    } else {
        "{}".to_string()
    }
}

/// Format a value with proper multiline indentation
fn format_value_multiline(val: &Value, indent_level: usize) -> String {
    // 1. Unwrap IR Expression if it's one
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

    let indent = "  ".repeat(indent_level);
    let next_indent = "  ".repeat(indent_level + 1);

    match actual_val {
        Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        Value::Number(n) => {
            let s = n.to_string();
            if s.ends_with(',') {
                s
            } else {
                format!("{},", s)
            }
        }
        Value::Bool(b) => format!("{},", b),
        Value::Null => "null,".to_string(),
        Value::Array(arr) => {
            if arr.is_empty() {
                "[],".to_string()
            } else {
                let items: Vec<String> = arr
                    .iter()
                    .map(|v| {
                        format!(
                            "{}{}",
                            next_indent,
                            format_value_multiline(v, indent_level + 1).trim_end_matches(',')
                        )
                    })
                    .collect();
                format!("[\n{}\n{}],", items.join(",\n"), indent)
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                "{},".to_string()
            } else {
                let mut fields = Vec::new();
                for (k, v) in obj {
                    if k.starts_with("__") {
                        continue; // Skip metadata
                    }
                    fields.push(format!(
                        "{}{}: {}",
                        next_indent,
                        k,
                        format_value_multiline(v, indent_level + 1).trim_end_matches(',')
                    ));
                }
                format!("{{\n{}\n{}}},", fields.join(",\n"), indent)
            }
        }
    }
}

/// Format custom intent example as inline key-value assignments
fn format_custom_intent_example_inline(example: &serde_json::Value) -> String {
    if example.as_object().map_or(true, |o| o.is_empty()) {
        return String::new();
    }

    // Format each field as key = value (one per line)
    // Note: example values are IR expressions like {"type": "literal", "value": "text"}
    let default_map = serde_json::Map::new();
    let obj_map = example.as_object().unwrap_or(&default_map);
    let mut formatted_fields: Vec<String> = obj_map
        .iter()
        .map(|(key, val): (&String, &Value)| {
            format!("{} = {}", key, format_value_inline(val))
        })
        .collect();

    // Sort for consistent output
    formatted_fields.sort();

    // Format as bare assignments: field1 = value1\nfield2 = value2
    formatted_fields.join("\n")
}
