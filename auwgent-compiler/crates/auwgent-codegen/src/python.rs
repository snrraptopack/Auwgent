use crate::common::{
    array_at, collect_custom_provider_ids, collect_handoff_helpers, collect_helper_tools,
    collect_required_providers, collect_transferred_helpers, collect_workflow_tools, join_sections,
    merge_helpers, merge_tool_defs, object_at, string_at,
};
use serde_json::{Map, Value};

pub fn generate(ir: &Value, base_name: &str) -> String {
    let agent_name = string_at(ir, &["name"]).unwrap_or("Agent");
    let workflow_tools = collect_workflow_tools(ir);
    let helper_tools = collect_helper_tools(ir);
    let all_tools = merge_tool_defs(
        array_at(ir, &["tools"]),
        workflow_tools.into_iter().chain(helper_tools.into_iter()).collect()
    );
    let has_tools = !all_tools.is_empty();
    let transferred_helpers = collect_transferred_helpers(ir);
    let handoff_helpers = collect_handoff_helpers(ir);
    let output_helpers = merge_helpers(transferred_helpers, handoff_helpers);
    let required_providers = collect_required_providers(ir);
    let custom_provider_ids = collect_custom_provider_ids(ir);

    let imports = [
        "import os",
        "import json",
        "from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal",
        "",
        "# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10",
        "try:",
        "    from typing import NotRequired",
        "except ImportError:",
        "    from typing_extensions import NotRequired",
        "",
        "try:",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError",
        "except ImportError:",
        "    # For local testing if auwgent is not installed via pip",
        "    import sys",
        "    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, AuwgentToolError",
        "",
    ]
    .join("\n");

    let mut sections = vec![
        format!("# Auto-generated types for {agent_name}"),
        "# Do not edit manually".to_string(),
        String::new(),
        imports,
    ];

    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        sections.push(generate_custom_types(types));
    }

    sections.push(generate_typed_dict(agent_name, "Input", unwrap_input_fields(ir.get("input")).as_ref()));
    for helper in &output_helpers {
        sections.push(generate_helper_output_interface(helper));
    }
    sections.push(generate_output_interface(ir, agent_name, &output_helpers));
    sections.push(generate_typed_dict(agent_name, "Context", ir.get("context")));
    sections.push(generate_tools_protocol(agent_name, &all_tools));
    sections.push(generate_custom_intents_union(ir, agent_name));

    if !required_providers.is_empty() {
        sections.push(generate_api_keys(agent_name, &required_providers, &custom_provider_ids));
    }

    sections.push(generate_factory_function(
        ir,
        agent_name,
        has_tools,
        !required_providers.is_empty(),
        base_name,
    ));

    join_sections(&sections)
}

fn generate_custom_types(types: &Map<String, Value>) -> String {
    let mut blocks = Vec::new();
    for (type_name, type_def) in types {
        let mut lines = vec![format!("class {type_name}(TypedDict, total=False):")];
        if let Some(properties) = object_at(type_def, &["properties"]) {
            if properties.is_empty() {
                lines.push("    pass".to_string());
            } else {
                for (prop_name, prop_info) in properties {
                    if let Some(description) = string_at(prop_info, &["description"]) {
                        lines.push(format!("    # {description}"));
                    }

                    let mut python_type = type_to_python_string(prop_info);
                    if prop_info
                        .get("optional")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        python_type = format!("Optional[{python_type}]");
                    }

                    lines.push(format!("    {prop_name}: {python_type}"));
                }
            }
        } else {
            lines.push("    pass".to_string());
        }
        blocks.push(lines.join("\n"));
    }
    blocks.join("\n\n")
}

fn generate_helper_output_interface(helper: &Value) -> String {
    let helper_name = string_at(helper, &["name"]).unwrap_or("Helper");
    generate_typed_dict(helper_name, "Output", helper.get("output"))
}

fn generate_custom_intents_union(ir: &Value, agent_name: &str) -> String {
    let mut intent_types: Vec<String> = Vec::new();

    // Collect from main agent
    if let Some(custom) = ir.get("customIntents").and_then(Value::as_array) {
        for ci in custom {
            let name = string_at(ci, &["name"]).unwrap_or_default();
            let fields = ci.get("fields").unwrap_or(&Value::Null);
            intent_types.push(format!(
                "TypedDict('_{}CustomIntent', {{\"name\": Literal[\"{}\"], \"value\": {}}}, total=False)",
                name,
                name,
                generate_raw_typed_dict_inline(fields)
            ));
        }
    }

    // Collect from helpers
    if let Some(helpers) = ir.get("helpers").and_then(Value::as_array) {
        for helper in helpers {
            if let Some(custom) = helper.get("customIntents").and_then(Value::as_array) {
                for ci in custom {
                    let name = string_at(ci, &["name"]).unwrap_or_default();
                    let fields = ci.get("fields").unwrap_or(&Value::Null);
                    let ty = format!(
                        "TypedDict('_{}CustomIntent', {{\"name\": Literal[\"{}\"], \"value\": {}}}, total=False)",
                        name,
                        name,
                        generate_raw_typed_dict_inline(fields)
                    );
                    if !intent_types.contains(&ty) {
                        intent_types.push(ty);
                    }
                }
            }
        }
    }

    if intent_types.is_empty() {
        format!("# No custom intents defined\n{agent_name}CustomIntents = None\n")
    } else if intent_types.len() == 1 {
        format!("{agent_name}CustomIntents = {}\n", intent_types[0])
    } else {
        format!(
            "{agent_name}CustomIntents = Union[\n    {},\n]\n",
            intent_types.join(",\n    ")
        )
    }
}

fn generate_raw_typed_dict_inline(value: &Value) -> String {
    let mut props = Vec::new();
    if let Some(obj) = value.as_object() {
        for (name, val) in obj {
            let mut python_type = type_to_python_string(val);
            if val.get("optional").and_then(Value::as_bool).unwrap_or(false) {
                python_type = format!("Optional[{python_type}]");
            }
            props.push(format!("\"{name}\": {python_type}"));
        }
    }
    format!("{{{}}}", props.join(", "))
}

fn generate_output_interface(ir: &Value, agent_name: &str, transferred_helpers: &[Value]) -> String {
    if let Some(variants) = object_at(ir, &["output", "__variants"]) {
        let mut blocks = Vec::new();
        let mut class_names = Vec::new();

        for (variant_name, variant_props) in variants {
            let class_name = format!("{agent_name}Output_{variant_name}");
            class_names.push(class_name.clone());
            blocks.push(generate_typed_dict_raw(&class_name, Some(variant_props)));
        }

        blocks.push(format!("{agent_name}Output = Union[{}]\n", class_names.join(", ")));
        return blocks.join("\n");
    }

    if transferred_helpers.is_empty() {
        return generate_typed_dict(agent_name, "Output", ir.get("output"));
    }

    let base_output = generate_typed_dict(agent_name, "BaseOutput", ir.get("output"));
    let union_members = std::iter::once(format!("{agent_name}BaseOutput"))
        .chain(transferred_helpers.iter().filter_map(|helper| {
            string_at(helper, &["name"]).map(|name| format!("{name}Output"))
        }))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{base_output}\n{agent_name}Output = Union[{union_members}]\n")
}

fn generate_tools_protocol(agent_name: &str, tools: &[Value]) -> String {
    if tools.is_empty() {
        return format!("class {agent_name}Tools(TypedDict, total=False):\n    pass\n");
    }

    let mut lines = vec![format!("class {agent_name}Tools(TypedDict, total=False):")];
    for tool in tools {
        if let Some(description) = string_at(tool, &["description"]) {
            lines.push(format!("    # {description}"));
        }

        let tool_name = string_at(tool, &["name"]).unwrap_or("tool");
        let param_types = object_at(tool, &["params"])
            .map(|params| {
                params
                    .values()
                    .filter(|type_obj| {
                        // Filter out internal AST schema items
                        if let Some(obj) = type_obj.as_object() {
                            if obj.contains_key("@id") || obj.contains_key("__source") {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|type_obj| {
                        let mut python_type = type_to_python_string(type_obj);
                        if type_obj
                            .get("optional")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            python_type = format!("Optional[{python_type}]");
                        }
                        python_type
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let returns = type_to_python_string(tool.get("returns").unwrap_or(&Value::Null));
        lines.push(format!(
            "    {tool_name}: Callable[[{param_types}], Awaitable[{returns}]]"
        ));
        lines.push(String::new());
    }

    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }

    lines.join("\n") + "\n"
}

fn generate_api_keys(
    agent_name: &str,
    providers: &std::collections::BTreeSet<String>,
    custom_ids: &std::collections::BTreeSet<String>,
) -> String {
    let mut keys = Vec::new();
    if providers.contains("gemini") {
        keys.push("    geminiApiKey: str".to_string());
    }
    if providers.contains("openai") {
        keys.push("    openaiApiKey: str".to_string());
    }
    
    // Generate individual API key fields for each custom provider
    for custom_id in custom_ids {
        let sanitized: String = custom_id
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let field_name = format!("{}ApiKey", sanitized);
        keys.push(format!("    {}: str  # API key for custom provider '{}'", field_name, custom_id));
    }

    format!("class {agent_name}ApiKeys(TypedDict, total=False):\n{}\n", keys.join("\n"))
}

fn generate_factory_function(ir: &Value, agent_name: &str, has_tools: bool, has_api_keys: bool, base_name: &str) -> String {
    let mut config_keys = Vec::new();
    if has_tools {
        config_keys.push(format!("    tools: NotRequired['{agent_name}Tools']"));
    }
    config_keys.push(format!("    middleware: NotRequired[List['{agent_name}Middleware']]"));

    if matches!(ir.get("context"), Some(context) if !context.is_null()) {
        config_keys.push(format!("    context: NotRequired['{agent_name}Context']"));
    }
    if has_api_keys {
        config_keys.push(format!("    apiKeys: NotRequired['{agent_name}ApiKeys']"));
    }

    [
        format!("{agent_name}Agent = TypedAuwgent"),
        String::new(),
        format!("{agent_name}Middleware = Middleware"),
        String::new(),
        format!("class {agent_name}Config(TypedDict, total=False):"),
        config_keys.join("\n"),
        String::new(),
        format!("def create{agent_name}(config: {agent_name}Config) -> '{agent_name}Agent':"),
        format!("    \"\"\"Create a fully configured {agent_name} agent from config.\"\"\""),
        format!("    ir_path = os.path.join(os.path.dirname(__file__), \"{base_name}.agent.json\")"),
        "    with open(ir_path, \"r\", encoding=\"utf-8\") as f:".to_string(),
        "        ir_dict = json.load(f)".to_string(),
        "    return create_auwgent(ir_dict, config)".to_string(),
        String::new(),
        format!("auwgent = create{agent_name}"),
        format!("AuwgentTools = {agent_name}Tools"),
        format!("AuwgentConfig = {agent_name}Config"),
        format!("AuwgentAgent = {agent_name}Agent"),
        format!("AuwgentMiddleware = {agent_name}Middleware"),
        format!("AuwgentContext = {agent_name}Context"),
    ]
    .join("\n")
}

fn generate_typed_dict(name: &str, suffix: &str, value: Option<&Value>) -> String {
    generate_typed_dict_raw(&format!("{name}{suffix}"), value)
}

fn generate_typed_dict_raw(class_name: &str, value: Option<&Value>) -> String {
    let mut lines = vec![format!("class {class_name}(TypedDict, total=False):")];

    match value.and_then(Value::as_object) {
        Some(properties) if !properties.is_empty() => {
            for (prop_name, prop_info) in properties {
                if prop_name.starts_with('@') || prop_name.starts_with("__") {
                    continue;
                }
                let mut python_type = type_to_python_string(prop_info);
                if prop_info
                    .get("optional")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    python_type = format!("Optional[{python_type}]");
                }
                lines.push(format!("    {prop_name}: {python_type}"));
            }
        }
        _ => lines.push("    pass".to_string()),
    }

    lines.join("\n") + "\n"
}

fn type_to_python_string(type_val: &Value) -> String {
    if let Some(raw) = type_val.as_str() {
        return normalize_python_type(raw);
    }

    if string_at(type_val, &["type"]) == Some("typeRef") {
        if let Some(name) = string_at(type_val, &["name"]) {
            return format!("\"{name}\"");
        }
    }

    if string_at(type_val, &["type"]) == Some("array") {
        if let Some(items) = type_val.get("items") {
            return format!("List[{}]", type_to_python_string(items));
        }
    }

    if string_at(type_val, &["type"]) == Some("union") {
        if type_val.get("options").and_then(Value::as_array).is_some() {
            return "str".to_string();
        }
    }

    if string_at(type_val, &["type"]) == Some("object") {
        if type_val.get("properties").and_then(Value::as_object).is_some() {
            return "Dict[str, Any]".to_string();
        }
    }

    if let Some(nested) = type_val.get("type") {
        if nested.is_object() {
            return type_to_python_string(nested);
        }
        if let Some(raw) = nested.as_str() {
            return normalize_python_type(raw);
        }
    }

    "Any".to_string()
}

fn normalize_python_type(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "int" | "number" | "float" => "float".to_string(),
        "bool" | "boolean" => "bool".to_string(),
        "string" => "str".to_string(),
        other => other.to_string(),
    }
}

/// Unwrap the input field from the IR format to the flat format expected by codegen.
/// IR format: { "kind": "properties", "fields": {...} } or { "kind": "direct", "type": "string" }
/// Codegen format: {...} (just the fields object)
fn unwrap_input_fields(input: Option<&Value>) -> Option<Value> {
    match input {
        Some(Value::Object(obj)) => {
            // Check if it has the "kind" wrapper
            if let Some(kind) = obj.get("kind").and_then(Value::as_str) {
                match kind {
                    "properties" => {
                        // Return the fields object directly
                        obj.get("fields").cloned()
                    }
                    "direct" => {
                        // For direct input (input: Text), return null since codegen expects
                        // properties format. Python will default to str.
                        None
                    }
                    _ => Some(Value::Object(obj.clone()))
                }
            } else {
                // Already in flat format
                Some(Value::Object(obj.clone()))
            }
        }
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::generate;
    use serde_json::json;

    #[test]
    fn emits_custom_provider_keys() {
        let ir = json!({
            "name": "Test",
            "modelConfig": [{
                "defaultConfig": { "model": { "type": "custom", "id": "my-groq" }, "prompt": { "type": "literal", "value": "Hello" } },
                "namedConfig": []
            }],
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": []
        });

        let output = generate(&ir, "main");
        assert!(output.contains("my_groqApiKey: str"));
        assert!(!output.contains("customUrl: NotRequired[str]"));
        assert!(output.contains("main.agent.json"));
    }
}