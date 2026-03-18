use crate::common::{
    array_at, collect_custom_provider_ids, collect_handoff_helpers, collect_required_providers,
    collect_transferred_helpers, collect_workflow_tools, join_sections, merge_helpers,
    merge_tool_defs, object_at, string_at,
};
use serde_json::{Map, Value};

pub fn generate(ir: &Value, base_name: &str) -> String {
    let agent_name = string_at(ir, &["name"]).unwrap_or("Agent");
    let workflow_tools = collect_workflow_tools(ir);
    let all_tools = merge_tool_defs(array_at(ir, &["tools"]), workflow_tools);
    let has_tools = !all_tools.is_empty();
    let has_context = ir
        .get("context")
        .and_then(Value::as_object)
        .map(|context| !context.is_empty())
        .unwrap_or(false);
    let required_providers = collect_required_providers(ir);
    let custom_provider_ids = collect_custom_provider_ids(ir);
    let output_helpers = merge_helpers(collect_transferred_helpers(ir), collect_handoff_helpers(ir));

    let workflow_types = match ir.get("workflows").and_then(Value::as_array) {
        Some(workflows) if !workflows.is_empty() => workflows
            .iter()
            .map(|workflow| {
                let flow_name = string_at(workflow, &["flowName"]).unwrap_or_default();
                let returns = workflow.get("returns").unwrap_or(&Value::Null);
                format!(
                    "{{ flowName: \"{}\"; returns: {} }}",
                    flow_name,
                    type_to_ts_string(returns)
                )
            })
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "undefined".to_string(),
    };

    let helper_types = match ir.get("helpers").and_then(Value::as_array) {
        Some(helpers) if !helpers.is_empty() => helpers
            .iter()
            .filter_map(|helper| string_at(helper, &["name"]))
            .map(|name| format!("{{ name: \"{}\" }}", name))
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "undefined".to_string(),
    };

    let workflow_array_type = if workflow_types == "undefined" {
        "undefined".to_string()
    } else {
        format!("({workflow_types})[]")
    };
    let helper_array_type = if helper_types == "undefined" {
        "undefined".to_string()
    } else {
        format!("({helper_types})[]")
    };

    let ir_import = [
        format!("import _importedIR from './{base_name}.agent.json' with {{ type: 'json' }};"),
        format!(
            "type {agent_name}IR = Omit<typeof _importedIR, \"name\" | \"workflows\" | \"helpers\"> & {{"
        ),
        format!("  name: \"{agent_name}\";"),
        format!("  workflows: {workflow_array_type};"),
        format!("  helpers: {helper_array_type};"),
        "};".to_string(),
        format!("const agentIR = _importedIR as unknown as {agent_name}IR;"),
    ]
    .join("\n");

    let mut sections = vec![
        format!("// Auto-generated types for {agent_name}"),
        "// Do not edit manually".to_string(),
        String::new(),
        "// Core Runtime Imports".to_string(),
        "import { createAuwgent } from \"@snrraptopack/auwgent-sdk\";".to_string(),
        "import type { ToolRegistry } from \"@snrraptopack/auwgent-sdk\";".to_string(),
        String::new(),
        ir_import,
        String::new(),
    ];

    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        sections.push(generate_custom_types(types));
    }

    sections.push(generate_object_alias(agent_name, "Input", ir.get("input")));
    for helper in &output_helpers {
        sections.push(generate_helper_output_interface(helper));
    }
    sections.push(generate_output_interface(ir, agent_name, &output_helpers));
    sections.push(generate_object_alias(agent_name, "Context", ir.get("context")));

    if has_tools {
        sections.push(generate_tools_interface(agent_name, &all_tools));
    }

    sections.push(generate_custom_intents_union(ir, agent_name));

    if !required_providers.is_empty() {
        sections.push(generate_api_keys(agent_name, &required_providers, &custom_provider_ids));
    }

    sections.push(generate_agent_factory(
        ir,
        agent_name,
        has_tools,
        has_context,
        !required_providers.is_empty(),
    ));

    join_sections(&sections)
}

fn generate_custom_types(types: &Map<String, Value>) -> String {
    let mut blocks = Vec::new();

    for (type_name, type_def) in types {
        let mut lines = Vec::new();
        if type_def
            .get("isOutput")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            lines.push("/** Output type */".to_string());
        }

        lines.push(format!("export type {type_name} = {{"));
        if let Some(properties) = object_at(type_def, &["properties"]) {
            for (prop_name, prop_info) in properties {
                if let Some(description) = string_at(prop_info, &["description"]) {
                    lines.push(String::new());
                    lines.push(format!("    /** {description} */"));
                }

                let optional = if prop_info
                    .get("optional")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    "?"
                } else {
                    ""
                };

                lines.push(format!(
                    "    {prop_name}{optional}: {};",
                    type_to_ts_string(prop_info)
                ));
            }
        }
        lines.push("}".to_string());

        blocks.push(lines.join("\n"));
    }

    blocks.join("\n\n")
}

fn generate_custom_intents_union(ir: &Value, agent_name: &str) -> String {
    let mut intents = Vec::new();

    // 1. Collect from main agent
    if let Some(custom) = ir.get("customIntents").and_then(Value::as_array) {
        for ci in custom {
            let name = string_at(ci, &["name"]).unwrap_or_default();
            let fields = ci.get("fields").unwrap_or(&Value::Null);
            intents.push(format!(
                "{{ name: \"{}\"; value: {} }}",
                name,
                generate_raw_object_type(fields)
            ));
        }
    }

    // 2. Collect from all helpers (since they can trigger intents too)
    if let Some(helpers) = ir.get("helpers").and_then(Value::as_array) {
        for helper in helpers {
            if let Some(custom) = helper.get("customIntents").and_then(Value::as_array) {
                for ci in custom {
                    let name = string_at(ci, &["name"]).unwrap_or_default();
                    let fields = ci.get("fields").unwrap_or(&Value::Null);
                    let intent_type =
                        format!("{{ name: \"{}\"; value: {} }}", name, generate_raw_object_type(fields));
                    if !intents.contains(&intent_type) {
                        intents.push(intent_type);
                    }
                }
            }
        }
    }

    let union_type = if intents.is_empty() {
        "never".to_string()
    } else {
        intents.join("\n    | ")
    };

    format!(
        "/** Custom intents defined in the DSL (if any) */\nexport type {}CustomIntents =\n    | {};\n",
        agent_name, union_type
    )
}

fn generate_raw_object_type(value: &Value) -> String {
    let mut props = Vec::new();
    if let Some(obj) = value.as_object() {
        for (name, val) in obj {
            let optional = if val
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "?"
            } else {
                ""
            };
            props.push(format!("{name}{optional}: {}", type_to_ts_string(val)));
        }
    }
    format!("{{ {} }}", props.join("; "))
}

fn generate_helper_output_interface(helper: &Value) -> String {
    let helper_name = string_at(helper, &["name"]).unwrap_or("Helper");
    generate_object_alias(helper_name, "Output", helper.get("output"))
}

fn generate_output_interface(ir: &Value, agent_name: &str, output_helpers: &[Value]) -> String {
    if let Some(variants) = object_at(ir, &["output", "__variants"]) {
        let union_members = variants
            .iter()
            .map(|(variant_name, variant_props)| {
                let props = variant_props
                    .as_object()
                    .map(|properties| {
                        properties
                            .iter()
                            .map(|(name, val)| {
                                let optional = if val
                                    .get("optional")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    "?"
                                } else {
                                    ""
                                };
                                format!("    {name}{optional}: {};", type_to_ts_string(val))
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();

                format!("{{ type: \"{variant_name}\";\n{props}\n}}")
            })
            .collect::<Vec<_>>()
            .join("\n    | ");

        return format!("export type {agent_name}Output =\n    | {union_members};\n");
    }

    let props = object_lines(ir.get("output"));
    if output_helpers.is_empty() {
        return format!("export type {agent_name}Output = {{\n{}\n}}\n", props.join("\n"));
    }

    let base_output = format!("export type {agent_name}BaseOutput = {{\n{}\n}}\n", props.join("\n"));
    let union_members = std::iter::once(format!("{agent_name}BaseOutput"))
        .chain(output_helpers.iter().filter_map(|helper| {
            string_at(helper, &["name"]).map(|name| format!("{name}Output"))
        }))
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        "{base_output}\n/** Union of possible output types (includes transfer destinations) */\nexport type {agent_name}Output = {union_members};\n"
    )
}

fn generate_tools_interface(agent_name: &str, tools: &[Value]) -> String {
    let methods = tools
        .iter()
        .filter_map(|tool| {
            let tool_name = string_at(tool, &["name"])?;
            let params = object_at(tool, &["params"])
                .map(|param_map| {
                    param_map
                        .iter()
                        .map(|(name, type_obj)| {
                            let optional = if type_obj
                                .get("optional")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            {
                                "?"
                            } else {
                                ""
                            };
                            format!("{name}{optional}: {}", type_to_ts_string(type_obj))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let returns = type_to_ts_string(tool.get("returns").unwrap_or(&Value::Null));

            Some(format!(
                "    {tool_name}: (args: {{ {params} }}) => Promise<{returns}>;"
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("export type {agent_name}Tools = {{\n{methods}\n}}\n")
}

fn generate_api_keys(
    agent_name: &str,
    providers: &std::collections::BTreeSet<String>,
    custom_ids: &std::collections::BTreeSet<String>,
) -> String {
    let mut keys = Vec::new();
    
    if providers.contains("gemini") {
        keys.push("    geminiApiKey: string;".to_string());
    }
    
    if providers.contains("openai") {
        keys.push("    openaiApiKey: string;".to_string());
    }
    
    // Generate individual API key fields for each custom provider (URL is in the IR, not needed here)
    for custom_id in custom_ids {
        let field_name = format!("{}ApiKey", custom_id.replace("-", "_"));
        keys.push(format!("    {}: string;  // API key for custom provider '{}'", field_name, custom_id));
    }

    format!(
        "/**\n * API keys required for {agent_name}\n */\nexport type {agent_name}ApiKeys = {{\n{}\n}}\n",
        keys.join("\n")
    )
}

fn generate_agent_factory(
    ir: &Value,
    agent_name: &str,
    has_tools: bool,
    has_context: bool,
    has_api_keys: bool,
) -> String {
    let tools_type = if has_tools {
        format!("{agent_name}Tools")
    } else {
        "Record<string, never>".to_string()
    };

    let output_type = format!("{agent_name}Output");

    let mut config_props = Vec::new();
    if has_tools {
        config_props.push(format!("    tools: {tools_type};"));
    }
    config_props.push(format!("    middleware?: {agent_name}Middleware[];"));
    if has_context {
        config_props.push(format!("    context: {agent_name}Context;"));
    }
    if has_api_keys {
        config_props.push(format!("    apiKeys: {agent_name}ApiKeys;"));
    }

    let factory_tools_arg = if has_tools {
        "tools: config.tools,".to_string()
    } else {
        "tools: {} as Record<string, never>,".to_string()
    };
    let context_line = if has_context {
        "        context: config.context,".to_string()
    } else {
        String::new()
    };
    let api_keys_line = if has_api_keys {
        "        apiKeys: config.apiKeys".to_string()
    } else {
        String::new()
    };

    let mut lines = vec![
        "// Defined explicitly (not via ReturnType) so RouterMiddleware can derive from it without circularity".to_string(),
        format!("export type {agent_name}Agent = import(\"@snrraptopack/auwgent-sdk\").TypedAuwgent<"),
        "    typeof agentIR,".to_string(),
        format!("    {agent_name}CustomIntents,"),
        format!("    {output_type},"),
        format!("    {tools_type}"),
        ">;".to_string(),
        String::new(),
        format!(
            "/** Middleware object type — consistent with `{agent_name}Agent.onIntent` intent narrowing */"
        ),
        format!(
            "export type {agent_name}Middleware<T extends import(\"@snrraptopack/auwgent-sdk\").MiddlewareContext<typeof agentIR>['activeAgent'] = import(\"@snrraptopack/auwgent-sdk\").MiddlewareContext<typeof agentIR>['activeAgent']> = import(\"@snrraptopack/auwgent-sdk\").Middleware<"
        ),
        "    typeof agentIR,".to_string(),
        format!("    {agent_name}CustomIntents,"),
        format!("    {output_type},"),
        format!("    {tools_type},"),
        "    T".to_string(),
        ">;".to_string(),
        String::new(),
        format!("export type {agent_name}Config = {{"),
        config_props.join("\n"),
        "}".to_string(),
        String::new(),
        format!("export function create{agent_name}(config: {agent_name}Config): {agent_name}Agent {{"),
        "    return createAuwgent<".to_string(),
        "        typeof agentIR,".to_string(),
        format!("        {agent_name}CustomIntents,"),
        format!("        {output_type},"),
        format!("        {tools_type}"),
        "    >(agentIR, {".to_string(),
        format!("        {factory_tools_arg}"),
        "        middleware: config.middleware as any,".to_string(),
    ];

    if has_context {
        lines.push(context_line);
    }
    if has_api_keys {
        lines.push(api_keys_line);
    }

    lines.extend([
        "    });".to_string(),
        "}".to_string(),
        String::new(),
        format!("export const auwgent = create{agent_name};"),
        format!("export type AuwgentTools = {tools_type};"),
        format!("export type AuwgentConfig = {agent_name}Config;"),
        format!("export type AuwgentAgent = {agent_name}Agent;"),
        format!("export type AuwgentMiddleware = {agent_name}Middleware;"),
        format!("export type AuwgentContext = {agent_name}Context;"),
    ]);

    lines.join("\n")
}

fn generate_object_alias(name: &str, suffix: &str, value: Option<&Value>) -> String {
    let props = object_lines(value);
    format!("export type {name}{suffix} = {{\n{}\n}}\n", props.join("\n"))
}

fn object_lines(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(name, val)| {
                    let optional = if val
                        .get("optional")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        "?"
                    } else {
                        ""
                    };

                    format!("    {name}{optional}: {};", type_to_ts_string(val))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn type_to_ts_string(type_val: &Value) -> String {
    if let Some(raw) = type_val.as_str() {
        return normalize_ts_type(raw);
    }

    if string_at(type_val, &["type"]) == Some("typeRef") {
        if let Some(name) = string_at(type_val, &["name"]) {
            return name.to_string();
        }
    }

    if string_at(type_val, &["type"]) == Some("array") {
        if let Some(items) = type_val.get("items") {
            return format!("{}[]", type_to_ts_string(items));
        }
    }

    if string_at(type_val, &["type"]) == Some("union") {
        if let Some(options) = type_val.get("options").and_then(Value::as_array) {
            return options
                .iter()
                .filter_map(Value::as_str)
                .map(|option| format!("\"{}\"", option.trim_matches(|c| c == '\'' || c == '\"')))
                .collect::<Vec<_>>()
                .join(" | ");
        }
    }

    if string_at(type_val, &["type"]) == Some("object") {
        if let Some(properties) = type_val.get("properties").and_then(Value::as_object) {
            let props = properties
                .iter()
                .map(|(key, val)| format!("{key}: {}", type_to_ts_string(val)))
                .collect::<Vec<_>>()
                .join("; ");
            return format!("{{ {props} }}");
        }
    }

    if let Some(nested) = type_val.get("type") {
        if nested.is_object() {
            return type_to_ts_string(nested);
        }
        if let Some(raw) = nested.as_str() {
            return normalize_ts_type(raw);
        }
    }

    "unknown".to_string()
}

fn normalize_ts_type(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "int" | "float" | "number" => "number".to_string(),
        "bool" | "boolean" => "boolean".to_string(),
        "string" => "string".to_string(),
        other => other.to_string(),
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
            "input": { "text": { "type": "string", "optional": false } },
            "output": { "name": { "type": "string", "optional": false } },
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": []
        });

        let output = generate(&ir, "main");
        assert!(output.contains("./main.agent.json"));
        assert!(output.contains("my_groqApiKey: string;"));
        assert!(!output.contains("customUrl?: string;"));
    }
}