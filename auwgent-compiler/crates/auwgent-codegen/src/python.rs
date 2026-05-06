use crate::{
    common::{join_sections, object_at, string_at},
    generation_plan::CodegenPlan,
};
use serde_json::{Map, Value};

pub fn generate(plan: &CodegenPlan, base_name: &str) -> String {
    let ir = plan.ir();
    let agent_name = plan.agent_name();
    let public_name = "Auwgent";
    let all_tools = plan.tools();
    let all_helpers = plan.helpers();
    let output_helpers = plan.output_helpers();
    let schema_output_helpers = output_helpers
        .iter()
        .filter(|helper| helper_has_declared_output(helper))
        .cloned()
        .collect::<Vec<_>>();
    let has_tools = plan.has_tools();
    let required_providers = plan.required_providers();
    let custom_provider_ids = plan.custom_provider_ids();

    let imports = [
        "import os",
        "import json",
        "from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal, overload",
        "",
        "# Required/NotRequired are 3.11+; fall back to typing_extensions for 3.9/3.10",
        "try:",
        "    from typing import Required, NotRequired",
        "except ImportError:",
        "    from typing_extensions import Required, NotRequired",
        "",
        "try:",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError, AuwgentTextPart, AuwgentImagePart, AuwgentFilePart, AuwgentAudioPart, AuwgentVideoPart",
        "except ImportError:",
        "    # For local testing if auwgent is not installed via pip",
        "    import sys",
        "    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError, AuwgentTextPart, AuwgentImagePart, AuwgentFilePart, AuwgentAudioPart, AuwgentVideoPart",
        "",
    ]
    .join("\n");

    let mut sections = vec![
        format!("# Auto-generated types for {agent_name}"),
        "# Do not edit manually".to_string(),
        String::new(),
        imports,
        generate_input_part_aliases(),
        String::new(),
    ];

    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        sections.push(generate_custom_types(types));
    }

    sections.push(generate_input_alias(ir.get("input")));
    if let Some(builders) = generate_input_builders(ir.get("input")) {
        sections.push(builders);
    }
    for helper in all_helpers {
        sections.push(generate_helper_output_interface(helper));
    }
    sections.push(generate_output_interface(
        ir,
        public_name,
        &schema_output_helpers,
    ));
    sections.push(generate_typed_dict(
        public_name,
        "Context",
        ir.get("context"),
    ));
    sections.push(generate_tools_protocol(public_name, all_tools));
    sections.push(generate_custom_intents_union(plan, public_name));
    sections.push(generate_intent_typing(plan, public_name));

    if plan.has_api_keys() {
        sections.push(generate_api_keys(
            public_name,
            required_providers,
            custom_provider_ids,
        ));
    }

    sections.push(generate_factory_function(
        plan,
        public_name,
        has_tools,
        plan.has_api_keys(),
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

                    lines.push(format!(
                        "    {prop_name}: {}",
                        typed_dict_field_type(prop_info)
                    ));
                }
            }
        } else {
            lines.push("    pass".to_string());
        }
        blocks.push(lines.join("\n"));
    }
    blocks.join("\n\n")
}

fn generate_input_part_aliases() -> String {
    [
        "TextPart = AuwgentTextPart",
        "ImagePart = AuwgentImagePart",
        "FilePart = AuwgentFilePart",
        "AudioPart = AuwgentAudioPart",
        "VideoPart = AuwgentVideoPart",
        "InputPart = Union[TextPart, ImagePart, FilePart, AudioPart, VideoPart]",
    ]
    .join("\n")
}

fn generate_helper_output_interface(helper: &Value) -> String {
    let helper_name = string_at(helper, &["name"]).unwrap_or("Helper");
    generate_named_python_shape(
        &format!("{helper_name}Output"),
        helper.get("output"),
        "None",
        false,
    )
}

fn generate_custom_intents_union(plan: &CodegenPlan, agent_name: &str) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let mut intent_types: Vec<String> = Vec::new();

    for (_, ci) in plan.custom_intent_defs() {
        let name = string_at(ci, &["name"]).unwrap_or_default();
        let safe = sanitize_python_type_name(name);
        let type_name = format!("{safe}Intent");
        let fields = ci.get("fields").unwrap_or(&Value::Null);
        if !intent_types.contains(&type_name) {
            blocks.push(generate_typed_dict_raw(&type_name, Some(fields)));
            intent_types.push(type_name);
        }
    }

    if intent_types.is_empty() {
        format!("# No custom intents defined\n{agent_name}CustomIntents = None\n")
    } else if intent_types.len() == 1 {
        blocks.push(format!("{agent_name}CustomIntents = {}\n", intent_types[0]));
        blocks.join("\n")
    } else {
        blocks.push(format!(
            "{agent_name}CustomIntents = Union[\n    {},\n]\n",
            intent_types.join(",\n    ")
        ));
        blocks.join("\n")
    }
}

fn generate_output_interface(
    ir: &Value,
    agent_name: &str,
    transferred_helpers: &[Value],
) -> String {
    if let Some(variants) = object_at(ir, &["output", "__variants"]) {
        let mut blocks = Vec::new();
        let mut class_names = Vec::new();

        for (variant_name, variant_props) in variants {
            let class_name = format!("{agent_name}Output_{variant_name}");
            class_names.push(class_name.clone());
            blocks.push(generate_typed_dict_raw(&class_name, Some(variant_props)));
        }

        blocks.push(format!(
            "{agent_name}Output = Union[{}]\n",
            class_names.join(", ")
        ));
        return blocks.join("\n");
    }

    if transferred_helpers.is_empty() {
        return generate_typed_dict(agent_name, "Output", ir.get("output"));
    }

    let base_output = generate_typed_dict(agent_name, "BaseOutput", ir.get("output"));
    let union_members =
        std::iter::once(format!("{agent_name}BaseOutput"))
            .chain(transferred_helpers.iter().filter_map(|helper| {
                string_at(helper, &["name"]).map(|name| format!("{name}Output"))
            }))
            .collect::<Vec<_>>()
            .join(", ");

    format!("{base_output}\n{agent_name}Output = Union[{union_members}]\n")
}

fn generate_tools_protocol(agent_name: &str, tools: &[Value]) -> String {
    if tools.is_empty() {
        return [
            format!("class {agent_name}Tools(Protocol):"),
            "    pass".to_string(),
            String::new(),
            format!("{agent_name}ToolsDict = Dict[str, Callable[..., Awaitable[Any]]]"),
            String::new(),
        ]
        .join("\n");
    }

    let mut lines = vec![format!("class {agent_name}Tools(Protocol):")];
    for tool in tools {
        if let Some(description) = string_at(tool, &["description"]) {
            lines.push(format!("    # {description}"));
        }

        let tool_name = string_at(tool, &["name"]).unwrap_or("tool");
        let param_signature = object_at(tool, &["params"])
            .map(|params| {
                params
                    .iter()
                    .filter_map(|(param_name, type_obj)| {
                        // Filter out internal AST schema items
                        if param_name.starts_with('@') || param_name.starts_with("__") {
                            return None;
                        }
                        if let Some(obj) = type_obj.as_object() {
                            if obj.contains_key("@id") || obj.contains_key("__source") {
                                return None;
                            }
                        }
                        let mut python_type = type_to_python_string(type_obj);
                        let optional = type_obj
                            .get("optional")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        if optional {
                            python_type = format!("Optional[{python_type}]");
                            Some(format!("{param_name}: {python_type} = ..."))
                        } else {
                            Some(format!("{param_name}: {python_type}"))
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let returns = type_to_python_string(tool.get("returns").unwrap_or(&Value::Null));

        if param_signature.is_empty() {
            lines.push(format!("    async def {tool_name}(self) -> {returns}: ..."));
        } else {
            lines.push(format!(
                "    async def {tool_name}(self, {}) -> {returns}: ...",
                param_signature.join(", ")
            ));
        }
        lines.push(String::new());
    }

    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }

    lines.push(String::new());
    lines.push(format!(
        "{agent_name}ToolsDict = Dict[str, Callable[..., Awaitable[Any]]]"
    ));

    lines.join("\n") + "\n"
}

fn generate_intent_typing(plan: &CodegenPlan, agent_name: &str) -> String {
    let ir = plan.ir();
    let has_declared_tools = plan.has_tools();
    let has_components = plan.has_components();

    let mut blocks = vec![];
    let mut value_types = vec![
        "ResponseText".to_string(),
        "ResponseSchema".to_string(),
        "ErrorIntent".to_string(),
    ];
    let mut intent_names = vec![
        "response_text".to_string(),
        "response_schema".to_string(),
        "error".to_string(),
    ];
    let mut custom_intents: Vec<(String, String)> = Vec::new();

    if has_declared_tools {
        let mut tool_call_types = Vec::new();
        let mut tool_result_types = Vec::new();
        let mut tool_error_types = Vec::new();
        let mut tool_skipped_types = Vec::new();

        blocks.push("class NoArgs(TypedDict, total=False):".to_string());
        blocks.push("    pass".to_string());
        blocks.push(String::new());

        if let Some(tools) = ir.get("tools").and_then(Value::as_array) {
            for tool in tools {
                let tool_name = string_at(tool, &["name"]).unwrap_or("tool");
                let safe_tool_name = sanitize_python_type_name(tool_name);
                let has_args = has_python_shape_fields(tool.get("params"));
                let args_type_name = if has_args {
                    format!("{safe_tool_name}ToolArgs")
                } else {
                    "NoArgs".to_string()
                };
                let result_type_name = format!("{safe_tool_name}Result");
                let call_intent_name = format!("{safe_tool_name}ToolCall");
                let result_intent_name = format!("{safe_tool_name}ToolResult");
                let error_intent_name = format!("{safe_tool_name}ToolError");
                let skipped_intent_name = format!("{safe_tool_name}ToolSkipped");

                if has_args {
                    blocks.push(generate_typed_dict_raw(&args_type_name, tool.get("params")));
                }
                blocks.push(generate_named_python_shape(
                    &result_type_name,
                    tool.get("returns"),
                    "Any",
                    false,
                ));

                blocks.push(format!("class {call_intent_name}(TypedDict):"));
                blocks.push(format!("    type: Literal[\"{tool_name}\"]"));
                if has_args {
                    blocks.push(format!("    args: {args_type_name}"));
                }
                blocks.push(String::new());

                blocks.push(format!("class {result_intent_name}(TypedDict):"));
                blocks.push(format!("    name: Literal[\"{tool_name}\"]"));
                blocks.push(format!("    args: {args_type_name}"));
                blocks.push(format!("    result: {result_type_name}"));
                blocks.push("    overridden: NotRequired[bool]".to_string());
                blocks.push(String::new());

                blocks.push(format!("class {error_intent_name}(TypedDict):"));
                blocks.push(format!("    tool: Literal[\"{tool_name}\"]"));
                blocks.push("    message: str".to_string());
                blocks.push(String::new());

                blocks.push(format!("class {skipped_intent_name}(TypedDict):"));
                blocks.push(format!("    type: Literal[\"{tool_name}\"]"));
                if has_args {
                    blocks.push(format!("    args: {args_type_name}"));
                }
                blocks.push(String::new());

                tool_call_types.push(call_intent_name);
                tool_result_types.push(result_intent_name);
                tool_error_types.push(error_intent_name);
                tool_skipped_types.push(skipped_intent_name);
            }
        }

        if !tool_call_types.is_empty() {
            blocks.push(format!("ToolCall = Union[{}]", tool_call_types.join(", ")));
            blocks.push(format!(
                "ToolResult = Union[{}]",
                tool_result_types.join(", ")
            ));
            blocks.push(format!(
                "ToolError = Union[{}]",
                tool_error_types.join(", ")
            ));
            blocks.push(format!(
                "ToolSkipped = Union[{}]",
                tool_skipped_types.join(", ")
            ));
            blocks.push("ToolCalls = ToolCall".to_string());
            blocks.push("ToolResults = ToolResult".to_string());
            blocks.push("ToolErrors = ToolError".to_string());
            blocks.push("ToolSkippeds = ToolSkipped".to_string());
            blocks.push(String::new());
        } else {
            blocks.extend([
                "class ToolCall(TypedDict):".to_string(),
                "    type: str".to_string(),
                "    args: Dict[str, Any]".to_string(),
                String::new(),
                "class ToolResult(TypedDict):".to_string(),
                "    name: str".to_string(),
                "    result: Any".to_string(),
                "    overridden: NotRequired[bool]".to_string(),
                String::new(),
                "class ToolError(TypedDict):".to_string(),
                "    tool: str".to_string(),
                "    message: str".to_string(),
                String::new(),
                "class ToolSkipped(TypedDict):".to_string(),
                "    type: str".to_string(),
                "    args: Dict[str, Any]".to_string(),
                String::new(),
            ]);
        }

        value_types.splice(
            0..0,
            vec![
                "ToolCall".to_string(),
                "ToolResult".to_string(),
                "ToolError".to_string(),
                "ToolSkipped".to_string(),
            ],
        );
        intent_names.splice(
            0..0,
            vec![
                "tool_call".to_string(),
                "tool_result".to_string(),
                "tool_error".to_string(),
                "tool_skipped".to_string(),
            ],
        );
    }

    let mut schema_names: Vec<String> = Vec::new();
    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        schema_names.extend(types.keys().cloned());
    }
    schema_names.push(format!("{agent_name}Output"));
    if let Some(helpers) = ir.get("helpers").and_then(Value::as_array) {
        for helper in helpers {
            if !helper_has_declared_output(helper) {
                continue;
            }
            if let Some(name) = string_at(helper, &["name"]) {
                schema_names.push(format!("{name}Output"));
            }
        }
    }
    schema_names.sort();
    schema_names.dedup();

    let mut response_schema_variant_types = Vec::new();
    if schema_names.is_empty() {
        blocks.push(format!("class ResponseSchema(TypedDict):"));
        blocks.push("    type: str".to_string());
        blocks.push("    response: Any".to_string());
        blocks.push(String::new());
    } else {
        for schema_name in &schema_names {
            let safe_schema_name = sanitize_python_type_name(schema_name);
            let variant_type_name = format!("{safe_schema_name}ResponseSchema");

            blocks.push(format!("class {variant_type_name}(TypedDict):"));
            blocks.push(format!("    type: Literal[\"{schema_name}\"]"));
            blocks.push(format!("    response: {schema_name}"));
            blocks.push(String::new());

            response_schema_variant_types.push(variant_type_name);
        }

        blocks.push(format!(
            "ResponseSchema = Union[{}]",
            response_schema_variant_types.join(", ")
        ));
        blocks.push(String::new());
    }

    blocks.extend([
        "class ResponseText(TypedDict):".to_string(),
        "    text: str".to_string(),
        String::new(),
        "class ErrorIntent(TypedDict):".to_string(),
        "    message: str".to_string(),
    ]);

    let mut seen_custom = std::collections::BTreeSet::new();
    if let Some(custom) = ir.get("customIntents").and_then(Value::as_array) {
        for ci in custom {
            let name = string_at(ci, &["name"]).unwrap_or_default();
            if name.is_empty() || !seen_custom.insert(name.to_string()) {
                continue;
            }
            let safe = sanitize_python_type_name(name);
            let type_name = format!("{safe}Intent");
            value_types.push(type_name.clone());
            intent_names.push(name.to_string());
            custom_intents.push((name.to_string(), type_name));
        }
    }
    if let Some(helpers) = ir.get("helpers").and_then(Value::as_array) {
        for helper in helpers {
            if let Some(custom) = helper.get("customIntents").and_then(Value::as_array) {
                for ci in custom {
                    let name = string_at(ci, &["name"]).unwrap_or_default();
                    if name.is_empty() || !seen_custom.insert(name.to_string()) {
                        continue;
                    }
                    let safe = sanitize_python_type_name(name);
                    let type_name = format!("{safe}Intent");
                    value_types.push(type_name.clone());
                    intent_names.push(name.to_string());
                    custom_intents.push((name.to_string(), type_name));
                }
            }
        }
    }

    let mut workflow_call_types = Vec::new();
    let mut workflow_result_types = Vec::new();
    let mut helper_call_types = Vec::new();
    let mut helper_result_types = Vec::new();

    if let Some(workflows) = ir.get("workflows").and_then(Value::as_array) {
        for workflow in workflows {
            let flow_name = string_at(workflow, &["flowName"]).unwrap_or("workflow");
            let safe_name = sanitize_python_type_name(flow_name);
            let args_type_name = format!("{safe_name}WorkflowArgs");
            let result_type_name = format!("{safe_name}WorkflowResultValue");
            let call_type_name = format!("{safe_name}WorkflowCall");
            let result_intent_name = format!("{safe_name}WorkflowResult");

            blocks.push(generate_named_python_shape(
                &args_type_name,
                workflow.get("flowParams"),
                "{}",
                false,
            ));
            blocks.push(generate_named_python_shape(
                &result_type_name,
                workflow.get("returns"),
                "str",
                false,
            ));
            blocks.push(format!("class {call_type_name}(TypedDict):"));
            blocks.push(format!("    type: Literal[\"{flow_name}\"]"));
            blocks.push(format!("    args: {args_type_name}"));
            blocks.push(String::new());
            blocks.push(format!("class {result_intent_name}(TypedDict):"));
            blocks.push(format!("    name: Literal[\"{flow_name}\"]"));
            blocks.push(format!("    result: {result_type_name}"));
            blocks.push(String::new());

            value_types.push(call_type_name.clone());
            value_types.push(result_intent_name.clone());
            workflow_call_types.push(call_type_name);
            workflow_result_types.push(result_intent_name);
        }
    }

    if let Some(helpers) = ir.get("helpers").and_then(Value::as_array) {
        for helper in helpers {
            let helper_name = string_at(helper, &["name"]).unwrap_or("Helper");
            let safe_name = sanitize_python_type_name(helper_name);
            let args_type_name = format!("{safe_name}HelperArgs");
            let result_type_name = format!("{safe_name}HelperResultValue");
            let call_type_name = format!("{safe_name}HelperCall");
            let result_intent_name = format!("{safe_name}HelperResult");

            blocks.push(generate_named_python_shape(
                &args_type_name,
                helper.get("input"),
                "str",
                true,
            ));
            blocks.push(generate_named_python_shape(
                &result_type_name,
                helper.get("output"),
                "TypedDict('_TextOutput', {\"text\": str}, total=False)",
                false,
            ));
            blocks.push(format!("class {call_type_name}(TypedDict):"));
            blocks.push(format!("    type: Literal[\"{helper_name}\"]"));
            blocks.push(format!("    args: {args_type_name}"));
            blocks.push(String::new());
            blocks.push(format!("class {result_intent_name}(TypedDict):"));
            blocks.push(format!("    name: Literal[\"{helper_name}\"]"));
            blocks.push(format!("    result: {result_type_name}"));
            blocks.push(String::new());

            value_types.push(call_type_name.clone());
            value_types.push(result_intent_name.clone());
            helper_call_types.push(call_type_name);
            helper_result_types.push(result_intent_name);
        }
    }

    blocks.push(format!(
        "{agent_name}IntentValue = Union[\n    {},\n]",
        value_types.join(",\n    ")
    ));
    if !workflow_call_types.is_empty() {
        blocks.push(format!(
            "WorkflowCall = Union[{}]",
            workflow_call_types.join(", ")
        ));
        blocks.push(format!(
            "WorkflowResult = Union[{}]",
            workflow_result_types.join(", ")
        ));
        blocks.push("WorkflowCalls = WorkflowCall".to_string());
        blocks.push("WorkflowResults = WorkflowResult".to_string());
        blocks.push(String::new());
        intent_names.push("workflow_call".to_string());
        intent_names.push("workflow_result".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!(
            "HelperCall = Union[{}]",
            helper_call_types.join(", ")
        ));
        blocks.push(format!(
            "HelperResult = Union[{}]",
            helper_result_types.join(", ")
        ));
        blocks.push("HelperCalls = HelperCall".to_string());
        blocks.push("HelperResults = HelperResult".to_string());
        blocks.push(String::new());
        intent_names.push("helper_call".to_string());
        intent_names.push("helper_result".to_string());
    }

    if has_components {
        blocks.push("class ComponentIntent(TypedDict):".to_string());
        blocks.push("    type: str".to_string());
        blocks.push("    c_id: str".to_string());
        blocks.push("    props: Dict[str, Any]".to_string());
        blocks.push("    action: NotRequired[Any]".to_string());
        blocks.push("    children: NotRequired[List[str]]".to_string());
        blocks.push(String::new());
        blocks.push(format!("class RenderComponentIntent(TypedDict):"));
        blocks.push("    root: NotRequired[str]".to_string());
        blocks.push("    roots: NotRequired[List[str]]".to_string());
        blocks.push("    components: NotRequired[Dict[str, Any]]".to_string());
        blocks.push("    tree: NotRequired[Any]".to_string());
        blocks.push("    trees: NotRequired[List[Any]]".to_string());
        blocks.push(String::new());

        value_types.push("ComponentIntent".to_string());
        value_types.push("RenderComponentIntent".to_string());
        intent_names.push("component".to_string());
        intent_names.push("render_component".to_string());
    }

    blocks.push(format!(
        "{agent_name}IntentName = Literal[{}]",
        intent_names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    blocks.push(String::new());
    blocks.push(format!("class {agent_name}IntentControlSkip(TypedDict):"));
    blocks.push("    skip: Literal[True]".to_string());
    blocks.push(String::new());
    blocks.push(format!(
        "class {agent_name}IntentControlOverride(TypedDict):"
    ));
    blocks.push("    result: Any".to_string());
    blocks.push(String::new());
    blocks.push(format!(
        "{agent_name}IntentControl = Union[{agent_name}IntentControlSkip, {agent_name}IntentControlOverride]"
    ));
    blocks.push(format!(
        "{agent_name}IntentHandlerReturn = Optional[Union[SessionState, {agent_name}IntentControl]]"
    ));
    blocks.push(String::new());
    blocks.push(format!(
        "{agent_name}IntentHandler = Callable[[{agent_name}IntentName, {agent_name}IntentValue, str], Awaitable[{agent_name}IntentHandlerReturn]]"
    ));
    blocks.push(format!(
        "# Partial intent payloads use top-level fields (for example: text/type/args/response)."
    ));
    blocks.push(format!(
        "{agent_name}PartialResponseTextIntent = PartialTextIntentValue"
    ));
    blocks.push(format!(
        "{agent_name}PartialResponseSchemaIntent = PartialStructuredIntentValue"
    ));
    blocks.push(format!(
        "{agent_name}PartialErrorIntent = PartialStructuredIntentValue"
    ));
    if has_declared_tools {
        blocks.push(format!(
            "{agent_name}PartialToolCallIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialToolResultIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialToolErrorIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialToolSkippedIntent = PartialStructuredIntentValue"
        ));
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!(
            "{agent_name}PartialWorkflowCallIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialWorkflowResultIntent = PartialStructuredIntentValue"
        ));
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!(
            "{agent_name}PartialHelperCallIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialHelperResultIntent = PartialStructuredIntentValue"
        ));
    }
    if has_components {
        blocks.push(format!(
            "{agent_name}PartialComponentIntent = PartialStructuredIntentValue"
        ));
        blocks.push(format!(
            "{agent_name}PartialRenderComponentIntent = PartialStructuredIntentValue"
        ));
    }
    blocks.push(format!(
        "{agent_name}PartialIntentHandler = Callable[[{agent_name}IntentName, PartialIntentValue, str], None]"
    ));

    blocks.push(String::new());
    blocks.push(format!("class {agent_name}BaseIntentHandler:"));
    if has_declared_tools {
        blocks.push(format!("    def tool_call(self, value: ToolCalls, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_result(self, value: ToolResults, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_error(self, value: ToolErrors, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_skipped(self, value: ToolSkippeds, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    blocks.push(format!("    def response_text(self, value: ResponseText, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def response_schema(self, value: ResponseSchema, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def error(self, value: ErrorIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    for (custom_name, custom_type) in &custom_intents {
        let method_name = sanitize_python_identifier(custom_name);
        blocks.push(format!("    def {method_name}(self, value: {custom_type}, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!("    def workflow_call(self, value: WorkflowCalls, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def workflow_result(self, value: WorkflowResults, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("    def helper_call(self, value: HelperCalls, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def helper_result(self, value: HelperResults, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    if has_components {
        blocks.push(format!("    def component(self, value: ComponentIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def render_component(self, value: RenderComponentIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }

    blocks.push(String::new());
    blocks.push(format!("class {agent_name}BasePartialIntentHandler:"));
    if has_declared_tools {
        blocks.push(format!("    def tool_call(self, value: {agent_name}PartialToolCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_result(self, value: {agent_name}PartialToolResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_error(self, value: {agent_name}PartialToolErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_skipped(self, value: {agent_name}PartialToolSkippedIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    blocks.push(format!("    def response_text(self, value: {agent_name}PartialResponseTextIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def response_schema(self, value: {agent_name}PartialResponseSchemaIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def error(self, value: {agent_name}PartialErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    for (custom_name, _custom_type) in &custom_intents {
        let method_name = sanitize_python_identifier(custom_name);
        blocks.push(format!("    def {method_name}(self, value: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!("    def workflow_call(self, value: {agent_name}PartialWorkflowCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def workflow_result(self, value: {agent_name}PartialWorkflowResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("    def helper_call(self, value: {agent_name}PartialHelperCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def helper_result(self, value: {agent_name}PartialHelperResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if has_components {
        blocks.push(format!("    def component(self, value: {agent_name}PartialComponentIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def render_component(self, value: {agent_name}PartialRenderComponentIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }

    blocks.join("\n") + "\n"
}

fn sanitize_python_type_name(name: &str) -> String {
    let mut out = String::new();
    for part in name
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }

    if out.is_empty() {
        "Value".to_string()
    } else if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

fn sanitize_python_identifier(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for (idx, c) in name.chars().enumerate() {
        let ch = if c.is_alphanumeric() || c == '_' {
            c
        } else {
            '_'
        };
        if idx == 0 && ch.is_ascii_digit() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }

    if out.is_empty() {
        return "intent".to_string();
    }

    match out.as_str() {
        "class" | "def" | "return" | "from" | "import" | "for" | "while" | "if" | "else"
        | "elif" | "try" | "except" | "finally" | "with" | "pass" | "raise" | "global"
        | "nonlocal" | "lambda" | "yield" | "await" | "async" | "True" | "False" | "None" => {
            format!("{out}_")
        }
        _ => out,
    }
}

fn generate_named_python_shape(
    type_name: &str,
    value: Option<&Value>,
    null_fallback: &str,
    unwrap_input_kind: bool,
) -> String {
    if let Some(original_shape) = value {
        let shape = unwrap_wrapped_shape(original_shape).unwrap_or(original_shape);
        if let Some(properties) = python_shape_properties(shape, unwrap_input_kind) {
            return generate_typed_dict_raw(type_name, Some(&properties));
        }

        return format!(
            "{type_name} = {}\n",
            python_shape_type(shape, null_fallback)
        );
    }

    format!("{type_name} = {null_fallback}\n")
}

fn unwrap_wrapped_shape(value: &Value) -> Option<&Value> {
    let obj = value.as_object()?;
    obj.get("type").filter(|inner| inner.is_object())
}

fn helper_has_declared_output(helper: &Value) -> bool {
    match helper.get("output") {
        None | Some(Value::Null) => false,
        Some(_) => true,
    }
}

fn python_shape_properties(value: &Value, unwrap_input_kind: bool) -> Option<Value> {
    match value {
        Value::Object(obj) if unwrap_input_kind => match obj.get("kind").and_then(Value::as_str) {
            Some("properties") => obj.get("fields").cloned(),
            _ => None,
        },
        Value::Object(obj) => {
            if obj.get("type").and_then(Value::as_str) == Some("object") {
                obj.get("properties").cloned()
            } else if obj.contains_key("type") {
                None
            } else {
                Some(Value::Object(obj.clone()))
            }
        }
        _ => None,
    }
}

fn python_shape_type(value: &Value, null_fallback: &str) -> String {
    match value {
        Value::Null => null_fallback.to_string(),
        _ => type_to_python_string(value),
    }
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
    if providers.contains("groq") {
        keys.push("    groqApiKey: str".to_string());
    }

    // Generate individual API key fields for each custom provider
    for custom_id in custom_ids {
        let sanitized: String = custom_id
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let field_name = format!("{}ApiKey", sanitized);
        keys.push(format!(
            "    {}: str  # API key for custom provider '{}'",
            field_name, custom_id
        ));
    }

    format!(
        "class {agent_name}ApiKeys(TypedDict, total=False):\n{}\n",
        keys.join("\n")
    )
}

fn generate_factory_function(
    plan: &CodegenPlan,
    agent_name: &str,
    has_tools: bool,
    has_api_keys: bool,
    base_name: &str,
) -> String {
    let mut config_keys = Vec::new();
    if has_tools {
        config_keys.push(format!(
            "    tools: NotRequired[Union['{agent_name}Tools', {agent_name}ToolsDict]]"
        ));
    }
    config_keys.push(format!("    middleware: NotRequired[List[Union['{agent_name}Middleware', 'type[{agent_name}Middleware]']]]"));

    if plan.has_context() {
        if has_required_python_fields(plan.ir().get("context")) {
            config_keys.push(format!("    context: '{agent_name}Context'"));
        } else {
            config_keys.push(format!("    context: NotRequired['{agent_name}Context']"));
        }
    }
    if has_api_keys {
        config_keys.push(format!("    apiKeys: NotRequired['{agent_name}ApiKeys']"));
    }

    [
        format!("class {agent_name}Agent(TypedAuwgent[Any, {agent_name}Context, {agent_name}Output, {agent_name}Tools]):"),
        format!("    def on_intent(self, handler: Union[{agent_name}BaseIntentHandler, type[{agent_name}BaseIntentHandler]]) -> None:"),
        "        return super().on_intent(handler)".to_string(),
        String::new(),
        format!("    def on_intent_partial(self, handler: Union[{agent_name}BasePartialIntentHandler, type[{agent_name}BasePartialIntentHandler]]) -> None:"),
        "        return super().on_intent_partial(handler)".to_string(),
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
    ]
    .join("\n")
}

fn generate_typed_dict(name: &str, suffix: &str, value: Option<&Value>) -> String {
    generate_typed_dict_raw(&format!("{name}{suffix}"), value)
}

fn generate_input_alias(input: Option<&Value>) -> String {
    if matches!(input, None | Some(Value::Null)) {
        return "Input = str\n".to_string();
    }

    if let Some(Value::Object(obj)) = input {
        if obj.get("kind").and_then(Value::as_str) == Some("properties") {
            return generate_typed_dict_raw("Input", obj.get("fields"));
        }
        if !obj.contains_key("type") && !obj.contains_key("kind") {
            return generate_typed_dict_raw("Input", input);
        }
    }

    let input_type = input
        .map(input_to_python_string)
        .unwrap_or_else(|| "str".to_string());
    format!("Input = {input_type}\n")
}

fn input_to_python_string(input: &Value) -> String {
    match input {
        Value::String(raw) if is_media_ir_name(raw) => {
            format!("List[Union[TextPart, {}]]", normalize_python_type(raw))
        }
        Value::Object(obj) if obj.get("type").and_then(Value::as_str) == Some("union") => {
            let media = obj
                .get("options")
                .and_then(Value::as_array)
                .map(|options| {
                    options
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|option| is_media_ir_name(option))
                        .map(normalize_python_type)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if media.is_empty() {
                type_to_python_string(input)
            } else {
                format!("List[Union[TextPart, {}]]", media.join(", "))
            }
        }
        _ => type_to_python_string(input),
    }
}

fn generate_input_builders(input: Option<&Value>) -> Option<String> {
    let media = media_input_names(input?);
    if media.is_empty() {
        return None;
    }

    let mut lines = vec![
        "class _InputBuilder:".to_string(),
        "    def text(self, text: str) -> TextPart:".to_string(),
        "        return {\"type\": \"text\", \"text\": text}".to_string(),
        String::new(),
    ];

    if media.contains(&"image") {
        lines.extend([
            "    def image(self, *, data: Any = None, encoding: Optional[str] = None, path: Optional[str] = None, url: Optional[str] = None, ref: Optional[str] = None, mimeType: Optional[str] = None, detail: Optional[str] = None) -> ImagePart:".to_string(),
            "        return _media_part(\"image\", data=data, encoding=encoding, path=path, url=url, ref=ref, mimeType=mimeType, detail=detail)".to_string(),
            String::new(),
        ]);
    }
    if media.contains(&"file") {
        lines.extend([
            "    def file(self, *, data: Any = None, encoding: Optional[str] = None, path: Optional[str] = None, url: Optional[str] = None, ref: Optional[str] = None, mimeType: Optional[str] = None, name: Optional[str] = None) -> FilePart:".to_string(),
            "        return _media_part(\"file\", data=data, encoding=encoding, path=path, url=url, ref=ref, mimeType=mimeType, name=name)".to_string(),
            String::new(),
        ]);
    }
    if media.contains(&"audio") {
        lines.extend([
            "    def audio(self, *, data: Any = None, encoding: Optional[str] = None, path: Optional[str] = None, url: Optional[str] = None, ref: Optional[str] = None, mimeType: Optional[str] = None, transcript: Optional[str] = None) -> AudioPart:".to_string(),
            "        return _media_part(\"audio\", data=data, encoding=encoding, path=path, url=url, ref=ref, mimeType=mimeType, transcript=transcript)".to_string(),
            String::new(),
        ]);
    }
    if media.contains(&"video") {
        lines.extend([
            "    def video(self, *, data: Any = None, encoding: Optional[str] = None, path: Optional[str] = None, url: Optional[str] = None, ref: Optional[str] = None, mimeType: Optional[str] = None, transcript: Optional[str] = None, sampledFrames: Optional[List[ImagePart]] = None) -> VideoPart:".to_string(),
            "        return _media_part(\"video\", data=data, encoding=encoding, path=path, url=url, ref=ref, mimeType=mimeType, transcript=transcript, sampledFrames=sampledFrames)".to_string(),
            String::new(),
        ]);
    }

    lines.extend([
        "def _media_part(type_name: str, **values: Any) -> Dict[str, Any]:".to_string(),
        "    part = {\"type\": type_name}".to_string(),
        "    part.update({key: value for key, value in values.items() if value is not None})".to_string(),
        "    return part".to_string(),
        String::new(),
        "input = _InputBuilder()".to_string(),
        String::new(),
    ]);

    Some(lines.join("\n"))
}

fn generate_typed_dict_raw(class_name: &str, value: Option<&Value>) -> String {
    let mut lines = vec![format!("class {class_name}(TypedDict, total=False):")];

    match value.and_then(Value::as_object) {
        Some(properties) if !properties.is_empty() => {
            for (prop_name, prop_info) in properties {
                if prop_name.starts_with('@') || prop_name.starts_with("__") {
                    continue;
                }
                lines.push(format!(
                    "    {prop_name}: {}",
                    typed_dict_field_type(prop_info)
                ));
            }
        }
        _ => lines.push("    pass".to_string()),
    }

    lines.join("\n") + "\n"
}

fn typed_dict_field_type(prop_info: &Value) -> String {
    let python_type = type_to_python_string(prop_info);
    if prop_info
        .get("optional")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        format!("NotRequired[Optional[{python_type}]]")
    } else {
        format!("Required[{python_type}]")
    }
}

fn has_required_python_fields(value: Option<&Value>) -> bool {
    match value.and_then(Value::as_object) {
        Some(properties) => properties.iter().any(|(prop_name, prop_info)| {
            if prop_name.starts_with('@') || prop_name.starts_with("__") {
                return false;
            }
            !prop_info
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }),
        None => false,
    }
}

fn has_python_shape_fields(value: Option<&Value>) -> bool {
    match value.and_then(Value::as_object) {
        Some(properties) => properties.iter().any(|(prop_name, prop_info)| {
            if prop_name.starts_with('@') || prop_name.starts_with("__") {
                return false;
            }
            !prop_info
                .as_object()
                .is_some_and(|obj| obj.contains_key("@id") || obj.contains_key("__source"))
        }),
        None => false,
    }
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
        if type_val
            .get("properties")
            .and_then(Value::as_object)
            .is_some()
        {
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
        "image" => "ImagePart".to_string(),
        "file" => "FilePart".to_string(),
        "audio" => "AudioPart".to_string(),
        "video" => "VideoPart".to_string(),
        other => other.to_string(),
    }
}

fn is_media_ir_name(raw: &str) -> bool {
    matches!(raw, "image" | "file" | "audio" | "video")
}

fn media_input_names(input: &Value) -> Vec<&'static str> {
    match input {
        Value::String(raw) => media_ir_name(raw).into_iter().collect(),
        Value::Object(obj) if obj.get("type").and_then(Value::as_str) == Some("union") => obj
            .get("options")
            .and_then(Value::as_array)
            .map(|options| {
                options
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(media_ir_name)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn media_ir_name(raw: &str) -> Option<&'static str> {
    match raw {
        "image" => Some("image"),
        "file" => Some("file"),
        "audio" => Some("audio"),
        "video" => Some("video"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::generate;
    use crate::generation_plan::CodegenPlan;
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

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("my_groqApiKey: str"));
        assert!(!output.contains("customUrl: NotRequired[str]"));
        assert!(output.contains("main.agent.json"));
    }

    #[test]
    fn emits_helper_outputs_without_null_helpers_in_response_schema() {
        let ir = json!({
            "name": "SimpleTool",
            "modelConfig": [],
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [{
                "name": "Joker",
                "input": null,
                "output": null
            }, {
                "name": "Plan",
                "input": null,
                "output": {
                    "type": {
                        "type": "object",
                        "properties": {
                            "steps": {
                                "type": {
                                    "type": "array",
                                    "items": "string"
                                },
                                "optional": false
                            },
                            "motivation": {
                                "type": "string",
                                "optional": false
                            }
                        }
                    }
                }
            }]
        });

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("JokerOutput = None"));
        assert!(output.contains("class PlanOutput(TypedDict, total=False):"));
        assert!(output.contains("    steps: Required[List[str]]"));
        assert!(output.contains("    motivation: Required[str]"));
        assert!(output.contains("class PlanOutputResponseSchema(TypedDict):"));
        assert!(!output.contains("class JokerOutputResponseSchema(TypedDict):"));
        assert!(!output.contains("response: JokerOutput"));
        assert!(!output.contains("    type: Dict[str, Any]"));
    }

    #[test]
    fn emits_required_context_fields_and_config_context() {
        let ir = json!({
            "name": "SimpleTool",
            "modelConfig": [],
            "input": null,
            "output": null,
            "context": {
                "user_name": { "type": "string", "optional": false },
                "age": { "type": "number", "optional": false },
                "nickname": { "type": "string", "optional": true }
            },
            "tools": [],
            "workflows": [],
            "helpers": []
        });

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("from typing import Required, NotRequired"));
        assert!(output.contains("class AuwgentContext(TypedDict, total=False):"));
        assert!(output.contains("    user_name: Required[str]"));
        assert!(output.contains("    age: Required[float]"));
        assert!(output.contains("    nickname: NotRequired[Optional[str]]"));
        assert!(output.contains("class AuwgentConfig(TypedDict, total=False):"));
        assert!(output.contains("    context: 'AuwgentContext'"));
        assert!(!output.contains("context: NotRequired['AuwgentContext']"));
    }

    #[test]
    fn emits_python_tool_intents_like_rust_handler_surface() {
        let ir = json!({
            "name": "SimpleTool",
            "modelConfig": [],
            "input": null,
            "output": null,
            "context": null,
            "tools": [{
                "name": "get_location",
                "description": "Get location",
                "params": {},
                "returns": "string"
            }, {
                "name": "get_marks",
                "description": "Return the user's score",
                "params": {
                    "id": { "type": "string", "optional": false }
                },
                "returns": "string"
            }],
            "workflows": [],
            "helpers": []
        });

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("class AuwgentTools(Protocol):"));
        assert!(output.contains("async def get_location(self) -> str: ..."));
        assert!(output.contains("    # Return the user's score"));
        assert!(output.contains("async def get_marks(self, id: str) -> str: ..."));
        assert!(output.contains("class NoArgs(TypedDict, total=False):"));
        assert!(output.contains("GetLocationResult = str"));
        assert!(output.contains("class GetLocationToolCall(TypedDict):"));
        assert!(output.contains("class GetLocationToolResult(TypedDict):"));
        assert!(output.contains("    args: NoArgs"));
        assert!(output.contains("class GetMarksToolArgs(TypedDict, total=False):"));
        assert!(output.contains("GetMarksResult = str"));
        assert!(output.contains("class GetMarksToolCall(TypedDict):"));
        assert!(output.contains("    args: GetMarksToolArgs"));
        assert!(output.contains("ToolCall = Union[GetLocationToolCall, GetMarksToolCall]"));
        assert!(output.contains("ToolCalls = ToolCall"));
        assert!(output.contains("ToolResults = ToolResult"));
        assert!(output.contains("def tool_call(self, value: ToolCalls, agent_name: str)"));
        assert!(output.contains("def tool_result(self, value: ToolResults, agent_name: str)"));
        assert!(output.contains("def response_text(self, value: ResponseText, agent_name: str)"));
        assert!(!output.contains("SimpleToolToolCallIntent"));
        assert!(!output.contains("AuwgentToolCallIntent"));
        assert!(!output.contains("class GetLocationToolArgs"));
    }

    #[test]
    fn emits_python_intent_typing_for_workflows_and_helpers() {
        let ir = json!({
            "name": "Manager",
            "modelConfig": [],
            "input": null,
            "output": {
                "approved": { "type": "boolean", "optional": false }
            },
            "context": null,
            "tools": [],
            "workflows": [{
                "flowName": "deleteAccount",
                "flowParams": {
                    "id": { "type": "string", "optional": false }
                },
                "returns": {
                    "type": "object",
                    "properties": {
                        "delete": { "type": "boolean", "optional": false }
                    }
                }
            }],
            "helpers": [{
                "name": "Reviewer",
                "input": {
                    "kind": "properties",
                    "fields": {
                        "text": { "type": "string", "optional": false }
                    }
                },
                "output": {
                    "type": "object",
                    "properties": {
                        "approved": { "type": "boolean", "optional": false }
                    }
                }
            }]
        });

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("class DeleteAccountWorkflowArgs(TypedDict, total=False):"));
        assert!(output.contains("class ReviewerHelperArgs(TypedDict, total=False):"));
        assert!(output.contains("WorkflowCall = Union[DeleteAccountWorkflowCall]"));
        assert!(output.contains("WorkflowCalls = WorkflowCall"));
        assert!(output.contains("HelperCall = Union[ReviewerHelperCall]"));
        assert!(output.contains("HelperCalls = HelperCall"));
        assert!(output.contains(
            "AuwgentIntentHandlerReturn = Optional[Union[SessionState, AuwgentIntentControl]]"
        ));
        assert!(output.contains("class AuwgentBaseIntentHandler:"));
        assert!(output.contains("def on_intent(self, handler: Union[AuwgentBaseIntentHandler, type[AuwgentBaseIntentHandler]]) -> None:"));
        assert!(output.contains("class AuwgentBasePartialIntentHandler:"));
        assert!(!output.contains("ManagerBaseIntentHandler"));
    }

    #[test]
    fn emits_python_intent_typing_for_components() {
        let ir = json!({
            "name": "UiAgent",
            "modelConfig": [],
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [{
                "name": "Button",
                "props": {},
                "action": {
                    "onclick": [{
                        "name": "delete",
                        "params": {
                            "id": { "type": "string", "optional": false }
                        }
                    }]
                },
                "children": null
            }]
        });

        let output = generate(&CodegenPlan::new(ir), "main");
        assert!(output.contains("class ComponentIntent(TypedDict):"));
        assert!(output.contains("class RenderComponentIntent(TypedDict):"));
        assert!(output.contains("AuwgentIntentName = Literal[\"response_text\", \"response_schema\", \"error\", \"component\", \"render_component\"]"));
        assert!(output.contains("def component(self, value: ComponentIntent, agent_name: str)"));
        assert!(
            output.contains(
                "def render_component(self, value: RenderComponentIntent, agent_name: str)"
            )
        );
        assert!(output.contains("AuwgentPartialComponentIntent = PartialStructuredIntentValue"));
        assert!(
            output.contains("AuwgentPartialRenderComponentIntent = PartialStructuredIntentValue")
        );
        assert!(!output.contains("AuwgentComponentIntent"));
        assert!(!output.contains("AuwgentRenderComponentIntent"));
        assert!(!output.contains("UiAgentComponentIntent"));
    }

    #[test]
    fn emits_generated_media_input_aliases() {
        let ir = json!({
            "name": "Vision",
            "modelConfig": [],
            "input": { "type": "union", "options": ["image", "file"] },
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": []
        });

        let output = generate(&CodegenPlan::new(ir), "vision");
        assert!(output.contains("InputPart = Union[TextPart, ImagePart, FilePart, AudioPart, VideoPart]"));
        assert!(output.contains("Input = List[Union[TextPart, ImagePart, FilePart]]"));
        assert!(output.contains("input = _InputBuilder()"));
        assert!(output.contains("def image(self, *"));
        assert!(!output.contains("def audio(self, *"));
    }
}
