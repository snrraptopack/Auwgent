use crate::{
    common::{join_sections, object_at, string_at},
    generation_plan::CodegenPlan,
};
use serde_json::{Map, Value};

pub fn generate(plan: &CodegenPlan, base_name: &str) -> String {
    let ir = plan.ir();
    let agent_name = plan.agent_name();
    let all_tools = plan.tools();
    let output_helpers = plan.output_helpers();
    let has_tools = plan.has_tools();
    let required_providers = plan.required_providers();
    let custom_provider_ids = plan.custom_provider_ids();

    let imports = [
        "import os",
        "import json",
        "from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal, overload",
        "",
        "# NotRequired is 3.11+; fall back to typing_extensions for 3.9/3.10",
        "try:",
        "    from typing import NotRequired",
        "except ImportError:",
        "    from typing_extensions import NotRequired",
        "",
        "try:",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError",
        "except ImportError:",
        "    # For local testing if auwgent is not installed via pip",
        "    import sys",
        "    sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))",
        "    from auwgent_sdk import TypedAuwgent, create_auwgent, Middleware, MiddlewareContext, SessionState, PartialIntentValue, PartialTextIntentValue, PartialStructuredIntentValue, AuwgentToolError",
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
    for helper in output_helpers {
        sections.push(generate_helper_output_interface(helper));
    }
    sections.push(generate_output_interface(ir, agent_name, output_helpers));
    sections.push(generate_typed_dict(agent_name, "Context", ir.get("context")));
    sections.push(generate_tools_protocol(agent_name, all_tools));
    sections.push(generate_custom_intents_union(plan, agent_name));
    sections.push(generate_intent_typing(plan, agent_name));

    if plan.has_api_keys() {
        sections.push(generate_api_keys(agent_name, required_providers, custom_provider_ids));
    }

    sections.push(generate_factory_function(
        plan,
        agent_name,
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

fn generate_custom_intents_union(plan: &CodegenPlan, agent_name: &str) -> String {
    let mut intent_types: Vec<String> = Vec::new();

    for (_, ci) in plan.custom_intent_defs() {
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
            lines.push(format!(
                "    async def {tool_name}(self) -> {returns}: ..."
            ));
        } else {
            lines.push(format!(
                "    async def {tool_name}(self, *, {}) -> {returns}: ...",
                param_signature.join(", ")
            ));
        }
        lines.push(String::new());
    }

    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }

    lines.push(String::new());
    lines.push(format!("{agent_name}ToolsDict = Dict[str, Callable[..., Awaitable[Any]]]"));

    lines.join("\n") + "\n"
}

fn generate_intent_typing(plan: &CodegenPlan, agent_name: &str) -> String {
    let ir = plan.ir();
    let has_declared_tools = plan.has_tools();
    let has_components = plan.has_components();

    let mut blocks = vec![];
    let mut value_types = vec![
        format!("{agent_name}ResponseTextIntent"),
        format!("{agent_name}ResponseSchemaIntent"),
        format!("{agent_name}ErrorIntent"),
    ];
    let mut intent_names = vec!["response_text".to_string(), "response_schema".to_string(), "error".to_string()];
    let mut custom_intents: Vec<(String, String)> = Vec::new();

    if has_declared_tools {
        let mut tool_call_types = Vec::new();
        let mut tool_result_types = Vec::new();
        let mut tool_error_types = Vec::new();
        let mut tool_skipped_types = Vec::new();

        if let Some(tools) = ir.get("tools").and_then(Value::as_array) {
            for tool in tools {
                let tool_name = string_at(tool, &["name"]).unwrap_or("tool");
                let safe_tool_name = sanitize_python_type_name(tool_name);
                let args_type_name = format!("{agent_name}{safe_tool_name}ToolArgs");
                let result_type_name = format!("{agent_name}{safe_tool_name}ToolResultValue");
                let call_intent_name = format!("{agent_name}{safe_tool_name}ToolCallIntent");
                let result_intent_name = format!("{agent_name}{safe_tool_name}ToolResultIntent");
                let error_intent_name = format!("{agent_name}{safe_tool_name}ToolErrorIntent");
                let skipped_intent_name = format!("{agent_name}{safe_tool_name}ToolSkippedIntent");

                blocks.push(generate_typed_dict_raw(&args_type_name, tool.get("params")));
                blocks.push(generate_named_python_shape(
                    &result_type_name,
                    tool.get("returns"),
                    "Any",
                    false,
                ));

                blocks.push(format!("class {call_intent_name}(TypedDict):"));
                blocks.push(format!("    type: Literal[\"{tool_name}\"]"));
                blocks.push(format!("    args: {args_type_name}"));
                blocks.push(String::new());

                blocks.push(format!("class {result_intent_name}(TypedDict):"));
                blocks.push(format!("    name: Literal[\"{tool_name}\"]"));
                blocks.push(format!("    result: {result_type_name}"));
                blocks.push("    overridden: NotRequired[bool]".to_string());
                blocks.push(String::new());

                blocks.push(format!("class {error_intent_name}(TypedDict):"));
                blocks.push(format!("    tool: Literal[\"{tool_name}\"]"));
                blocks.push("    message: str".to_string());
                blocks.push(String::new());

                blocks.push(format!("class {skipped_intent_name}(TypedDict):"));
                blocks.push(format!("    type: Literal[\"{tool_name}\"]"));
                blocks.push(format!("    args: {args_type_name}"));
                blocks.push(String::new());

                tool_call_types.push(call_intent_name);
                tool_result_types.push(result_intent_name);
                tool_error_types.push(error_intent_name);
                tool_skipped_types.push(skipped_intent_name);
            }
        }

        if !tool_call_types.is_empty() {
            blocks.push(format!("{agent_name}ToolCallIntent = Union[{}]", tool_call_types.join(", ")));
            blocks.push(format!("{agent_name}ToolResultIntent = Union[{}]", tool_result_types.join(", ")));
            blocks.push(format!("{agent_name}ToolErrorIntent = Union[{}]", tool_error_types.join(", ")));
            blocks.push(format!("{agent_name}ToolSkippedIntent = Union[{}]", tool_skipped_types.join(", ")));
            blocks.push(String::new());
        } else {
            blocks.extend([
                format!("class {agent_name}ToolCallIntent(TypedDict):"),
                "    type: str".to_string(),
                "    args: Dict[str, Any]".to_string(),
                String::new(),
                format!("class {agent_name}ToolResultIntent(TypedDict):"),
                "    name: str".to_string(),
                "    result: Any".to_string(),
                "    overridden: NotRequired[bool]".to_string(),
                String::new(),
                format!("class {agent_name}ToolErrorIntent(TypedDict):"),
                "    tool: str".to_string(),
                "    message: str".to_string(),
                String::new(),
                format!("class {agent_name}ToolSkippedIntent(TypedDict):"),
                "    type: str".to_string(),
                "    args: Dict[str, Any]".to_string(),
                String::new(),
            ]);
        }

        value_types.splice(
            0..0,
            vec![
                format!("{agent_name}ToolCallIntent"),
                format!("{agent_name}ToolResultIntent"),
                format!("{agent_name}ToolErrorIntent"),
                format!("{agent_name}ToolSkippedIntent"),
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
            if let Some(name) = string_at(helper, &["name"]) {
                schema_names.push(format!("{name}Output"));
            }
        }
    }
    schema_names.sort();
    schema_names.dedup();

    let mut response_schema_variant_types = Vec::new();
    if schema_names.is_empty() {
        blocks.push(format!("class {agent_name}ResponseSchemaIntent(TypedDict):"));
        blocks.push("    type: str".to_string());
        blocks.push("    response: Any".to_string());
        blocks.push(String::new());
    } else {
        for schema_name in &schema_names {
            let safe_schema_name = sanitize_python_type_name(schema_name);
            let variant_type_name = format!("{agent_name}{safe_schema_name}ResponseSchemaIntent");

            blocks.push(format!("class {variant_type_name}(TypedDict):"));
            blocks.push(format!("    type: Literal[\"{schema_name}\"]"));
            blocks.push(format!("    response: {schema_name}"));
            blocks.push(String::new());

            response_schema_variant_types.push(variant_type_name);
        }

        blocks.push(format!(
            "{agent_name}ResponseSchemaIntent = Union[{}]",
            response_schema_variant_types.join(", ")
        ));
        blocks.push(String::new());
    }

    blocks.extend([
        format!("class {agent_name}ResponseTextIntent(TypedDict):"),
        "    text: str".to_string(),
        String::new(),
        format!("class {agent_name}ErrorIntent(TypedDict):"),
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
            let type_name = format!("{agent_name}{safe}CustomIntent");
            let fields = ci.get("fields").unwrap_or(&Value::Null);
            blocks.push(format!("class {type_name}(TypedDict):"));
            blocks.push(format!("    name: Literal[\"{name}\"]"));
            blocks.push(format!("    value: {}", generate_raw_typed_dict_inline(fields)));
            blocks.push(String::new());
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
                    let type_name = format!("{agent_name}{safe}CustomIntent");
                    let fields = ci.get("fields").unwrap_or(&Value::Null);
                    blocks.push(format!("class {type_name}(TypedDict):"));
                    blocks.push(format!("    name: Literal[\"{name}\"]"));
                    blocks.push(format!("    value: {}", generate_raw_typed_dict_inline(fields)));
                    blocks.push(String::new());
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
            let args_type_name = format!("{agent_name}{safe_name}WorkflowArgs");
            let result_type_name = format!("{agent_name}{safe_name}WorkflowResultValue");
            let call_type_name = format!("{agent_name}{safe_name}WorkflowCall");
            let result_intent_name = format!("{agent_name}{safe_name}WorkflowResult");

            blocks.push(generate_named_python_shape(&args_type_name, workflow.get("flowParams"), "{}", false));
            blocks.push(generate_named_python_shape(&result_type_name, workflow.get("returns"), "str", false));
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
            let args_type_name = format!("{agent_name}{safe_name}HelperArgs");
            let result_type_name = format!("{agent_name}{safe_name}HelperResultValue");
            let call_type_name = format!("{agent_name}{safe_name}HelperCall");
            let result_intent_name = format!("{agent_name}{safe_name}HelperResult");

            blocks.push(generate_named_python_shape(&args_type_name, helper.get("input"), "str", true));
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

    blocks.push(format!("{agent_name}IntentValue = Union[\n    {},\n]", value_types.join(",\n    ")));
    if !workflow_call_types.is_empty() {
        blocks.push(format!("{agent_name}WorkflowCallIntentValue = Union[{}]", workflow_call_types.join(", ")));
        blocks.push(format!("{agent_name}WorkflowResultIntentValue = Union[{}]", workflow_result_types.join(", ")));
        intent_names.push("workflow_call".to_string());
        intent_names.push("workflow_result".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("{agent_name}HelperCallIntentValue = Union[{}]", helper_call_types.join(", ")));
        blocks.push(format!("{agent_name}HelperResultIntentValue = Union[{}]", helper_result_types.join(", ")));
        intent_names.push("helper_call".to_string());
        intent_names.push("helper_result".to_string());
    }

    if has_components {
        blocks.push(format!("class {agent_name}ComponentIntent(TypedDict):"));
        blocks.push("    type: str".to_string());
        blocks.push("    c_id: str".to_string());
        blocks.push("    props: Dict[str, Any]".to_string());
        blocks.push("    action: NotRequired[Any]".to_string());
        blocks.push("    children: NotRequired[List[str]]".to_string());
        blocks.push(String::new());
        blocks.push(format!("class {agent_name}RenderComponentIntent(TypedDict):"));
        blocks.push("    root: NotRequired[str]".to_string());
        blocks.push("    roots: NotRequired[List[str]]".to_string());
        blocks.push("    components: NotRequired[Dict[str, Any]]".to_string());
        blocks.push("    tree: NotRequired[Any]".to_string());
        blocks.push("    trees: NotRequired[List[Any]]".to_string());
        blocks.push(String::new());

        value_types.push(format!("{agent_name}ComponentIntent"));
        value_types.push(format!("{agent_name}RenderComponentIntent"));
        intent_names.push("component".to_string());
        intent_names.push("render_component".to_string());
    }

    blocks.push(format!(
        "{agent_name}IntentName = Literal[{}]",
        intent_names.iter().map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", ")
    ));
    blocks.push(String::new());
    blocks.push(format!("class {agent_name}IntentControlSkip(TypedDict):"));
    blocks.push("    skip: Literal[True]".to_string());
    blocks.push(String::new());
    blocks.push(format!("class {agent_name}IntentControlOverride(TypedDict):"));
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
    blocks.push(format!("# Partial intent payloads use top-level fields (for example: text/type/args/response)."));
    blocks.push(format!("{agent_name}PartialResponseTextIntent = PartialTextIntentValue"));
    blocks.push(format!("{agent_name}PartialResponseSchemaIntent = PartialStructuredIntentValue"));
    blocks.push(format!("{agent_name}PartialErrorIntent = PartialStructuredIntentValue"));
    if has_declared_tools {
        blocks.push(format!("{agent_name}PartialToolCallIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialToolResultIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialToolErrorIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialToolSkippedIntent = PartialStructuredIntentValue"));
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!("{agent_name}PartialWorkflowCallIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialWorkflowResultIntent = PartialStructuredIntentValue"));
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("{agent_name}PartialHelperCallIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialHelperResultIntent = PartialStructuredIntentValue"));
    }
    if has_components {
        blocks.push(format!("{agent_name}PartialComponentIntent = PartialStructuredIntentValue"));
        blocks.push(format!("{agent_name}PartialRenderComponentIntent = PartialStructuredIntentValue"));
    }
    blocks.push(format!(
        "{agent_name}PartialIntentHandler = Callable[[{agent_name}IntentName, PartialIntentValue, str], None]"
    ));

    blocks.push(String::new());
    blocks.push(format!("class {agent_name}BaseIntentHandler:"));
    if has_declared_tools {
        blocks.push(format!("    def tool_call(self, intent: {agent_name}ToolCallIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_result(self, intent: {agent_name}ToolResultIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_error(self, intent: {agent_name}ToolErrorIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_skipped(self, intent: {agent_name}ToolSkippedIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    blocks.push(format!("    def response_text(self, intent: {agent_name}ResponseTextIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def response_schema(self, intent: {agent_name}ResponseSchemaIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def error(self, intent: {agent_name}ErrorIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
    blocks.push("        pass".to_string());
    for (custom_name, custom_type) in &custom_intents {
        let method_name = sanitize_python_identifier(custom_name);
        blocks.push(format!("    def {method_name}(self, intent: {custom_type}, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!("    def workflow_call(self, intent: Union[{}], agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:", workflow_call_types.join(", ")));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def workflow_result(self, intent: Union[{}], agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:", workflow_result_types.join(", ")));
        blocks.push("        pass".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("    def helper_call(self, intent: Union[{}], agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:", helper_call_types.join(", ")));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def helper_result(self, intent: Union[{}], agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:", helper_result_types.join(", ")));
        blocks.push("        pass".to_string());
    }
    if has_components {
        blocks.push(format!("    def component(self, intent: {agent_name}ComponentIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def render_component(self, intent: {agent_name}RenderComponentIntent, agent_name: str) -> Union[{agent_name}IntentHandlerReturn, Awaitable[{agent_name}IntentHandlerReturn]]:"));
        blocks.push("        pass".to_string());
    }

    blocks.push(String::new());
    blocks.push(format!("class {agent_name}BasePartialIntentHandler:"));
    if has_declared_tools {
        blocks.push(format!("    def tool_call(self, intent: {agent_name}PartialToolCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_result(self, intent: {agent_name}PartialToolResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_error(self, intent: {agent_name}PartialToolErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def tool_skipped(self, intent: {agent_name}PartialToolSkippedIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    blocks.push(format!("    def response_text(self, intent: {agent_name}PartialResponseTextIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def response_schema(self, intent: {agent_name}PartialResponseSchemaIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    blocks.push(format!("    def error(self, intent: {agent_name}PartialErrorIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
    blocks.push("        pass".to_string());
    for (custom_name, _custom_type) in &custom_intents {
        let method_name = sanitize_python_identifier(custom_name);
        blocks.push(format!("    def {method_name}(self, intent: PartialStructuredIntentValue, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if !workflow_call_types.is_empty() {
        blocks.push(format!("    def workflow_call(self, intent: {agent_name}PartialWorkflowCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def workflow_result(self, intent: {agent_name}PartialWorkflowResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!("    def helper_call(self, intent: {agent_name}PartialHelperCallIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def helper_result(self, intent: {agent_name}PartialHelperResultIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }
    if has_components {
        blocks.push(format!("    def component(self, intent: {agent_name}PartialComponentIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
        blocks.push(format!("    def render_component(self, intent: {agent_name}PartialRenderComponentIntent, agent_name: str) -> Union[None, Awaitable[None]]:"));
        blocks.push("        pass".to_string());
    }

    blocks.join("\n") + "\n"
}

fn sanitize_python_type_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();

    if sanitized.is_empty() {
        "Value".to_string()
    } else {
        sanitized
    }
}

fn sanitize_python_identifier(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for (idx, c) in name.chars().enumerate() {
        let ch = if c.is_alphanumeric() || c == '_' { c } else { '_' };
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
    if let Some(shape) = value {
        if let Some(properties) = python_shape_properties(shape, unwrap_input_kind) {
            return generate_typed_dict_raw(type_name, Some(&properties));
        }

        return format!("{type_name} = {}\n", python_shape_type(shape, null_fallback));
    }

    format!("{type_name} = {null_fallback}\n")
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
        keys.push(format!("    {}: str  # API key for custom provider '{}'", field_name, custom_id));
    }

    format!("class {agent_name}ApiKeys(TypedDict, total=False):\n{}\n", keys.join("\n"))
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
        config_keys.push(format!("    tools: NotRequired[Union['{agent_name}Tools', {agent_name}ToolsDict]]"));
    }
    config_keys.push(format!("    middleware: NotRequired[List[Union['{agent_name}Middleware', 'type[{agent_name}Middleware]']]]"));


    if plan.has_context() {
        config_keys.push(format!("    context: NotRequired['{agent_name}Context']"));
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
        format!("AuwgentTools = {agent_name}Tools"),
        format!("AuwgentConfig = {agent_name}Config"),
        format!("AuwgentAgent = {agent_name}Agent"),
        format!("AuwgentMiddleware = {agent_name}Middleware"),
        format!("AuwgentContext = {agent_name}Context"),
        format!("AuwgentIntentName = {agent_name}IntentName"),
        format!("AuwgentIntentValue = {agent_name}IntentValue"),
        format!("AuwgentIntentHandler = {agent_name}IntentHandler"),
        format!("AuwgentPartialIntentHandler = {agent_name}PartialIntentHandler"),
        format!("AuwgentBaseIntentHandler = {agent_name}BaseIntentHandler"),
        format!("AuwgentBasePartialIntentHandler = {agent_name}BasePartialIntentHandler"),
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
        assert!(output.contains("class ManagerdeleteAccountWorkflowArgs(TypedDict, total=False):"));
        assert!(output.contains("class ManagerReviewerHelperArgs(TypedDict, total=False):"));
        assert!(output.contains("ManagerIntentHandlerReturn = Optional[Union[SessionState, ManagerIntentControl]]"));
        assert!(output.contains("class ManagerBaseIntentHandler:"));
        assert!(output.contains("def on_intent(self, handler: Union[ManagerBaseIntentHandler, type[ManagerBaseIntentHandler]]) -> None:"));
        assert!(output.contains("class ManagerBasePartialIntentHandler:"));
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
        assert!(output.contains("class UiAgentComponentIntent(TypedDict):"));
        assert!(output.contains("class UiAgentRenderComponentIntent(TypedDict):"));
        assert!(output.contains("UiAgentIntentName = Literal[\"response_text\", \"response_schema\", \"error\", \"component\", \"render_component\"]"));
        assert!(output.contains("def component(self, intent: UiAgentComponentIntent, agent_name: str)"));
        assert!(output.contains("def render_component(self, intent: UiAgentRenderComponentIntent, agent_name: str)"));
        assert!(output.contains("UiAgentPartialComponentIntent = PartialStructuredIntentValue"));
        assert!(output.contains("UiAgentPartialRenderComponentIntent = PartialStructuredIntentValue"));
    }
}
