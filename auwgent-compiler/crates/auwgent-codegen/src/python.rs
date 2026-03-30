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
        "from typing import TypedDict, Callable, Awaitable, Any, List, Dict, Union, Optional, Protocol, Literal, overload",
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
    sections.push(generate_intent_typing(ir, agent_name));

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

fn generate_intent_typing(ir: &Value, agent_name: &str) -> String {
    let mut blocks = vec![
        format!("class {agent_name}ResponseTextIntent(TypedDict, total=False):"),
        "    text: str".to_string(),
        String::new(),
        format!("{agent_name}ResponseSchemaIntent = {agent_name}Output"),
        String::new(),
        format!("class {agent_name}ErrorIntent(TypedDict, total=False):"),
        "    message: str".to_string(),
    ];
    let mut value_types = vec![
        format!("{agent_name}ResponseTextIntent"),
        format!("{agent_name}ResponseSchemaIntent"),
        format!("{agent_name}ErrorIntent"),
    ];
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
            blocks.push(format!("class {call_type_name}(TypedDict, total=False):"));
            blocks.push(format!("    type: Literal[\"{flow_name}\"]"));
            blocks.push(format!("    args: {args_type_name}"));
            blocks.push(String::new());
            blocks.push(format!("class {result_intent_name}(TypedDict, total=False):"));
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
            blocks.push(format!("class {call_type_name}(TypedDict, total=False):"));
            blocks.push(format!("    type: Literal[\"{helper_name}\"]"));
            blocks.push(format!("    args: {args_type_name}"));
            blocks.push(String::new());
            blocks.push(format!("class {result_intent_name}(TypedDict, total=False):"));
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
    let mut intent_names = vec!["response_text".to_string(), "response_schema".to_string(), "error".to_string()];
    if !workflow_call_types.is_empty() {
        blocks.push(format!(
            "{agent_name}WorkflowCallIntentValue = Union[{}]",
            workflow_call_types.join(", ")
        ));
        blocks.push(format!(
            "{agent_name}WorkflowResultIntentValue = Union[{}]",
            workflow_result_types.join(", ")
        ));
        intent_names.push("workflow_call".to_string());
        intent_names.push("workflow_result".to_string());
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!(
            "{agent_name}HelperCallIntentValue = Union[{}]",
            helper_call_types.join(", ")
        ));
        blocks.push(format!(
            "{agent_name}HelperResultIntentValue = Union[{}]",
            helper_result_types.join(", ")
        ));
        intent_names.push("helper_call".to_string());
        intent_names.push("helper_result".to_string());
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
    blocks.push(format!(
        "{agent_name}IntentHandler = Callable[[{agent_name}IntentName, {agent_name}IntentValue, str], Awaitable[Optional[Dict[str, Any]]]]"
    ));
    blocks.push(format!(
        "{agent_name}PartialIntentHandler = Callable[[{agent_name}IntentName, {agent_name}IntentValue, str], None]"
    ));
    blocks.push(String::new());
    blocks.push(format!("class {agent_name}IntentHandlers(TypedDict, total=False):"));
    blocks.push(format!(
        "    response_text: Callable[[{agent_name}ResponseTextIntent], Awaitable[Any]]"
    ));
    blocks.push(format!(
        "    response_schema: Callable[[{agent_name}ResponseSchemaIntent], Awaitable[Any]]"
    ));
    blocks.push(format!(
        "    error: Callable[[{agent_name}ErrorIntent], Awaitable[Any]]"
    ));
    if !workflow_call_types.is_empty() {
        blocks.push(format!(
            "    workflow_call: Callable[[Union[{}]], Awaitable[Any]]",
            workflow_call_types.join(", ")
        ));
        blocks.push(format!(
            "    workflow_result: Callable[[Union[{}]], Awaitable[Any]]",
            workflow_result_types.join(", ")
        ));
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!(
            "    helper_call: Callable[[Union[{}]], Awaitable[Any]]",
            helper_call_types.join(", ")
        ));
        blocks.push(format!(
            "    helper_result: Callable[[Union[{}]], Awaitable[Any]]",
            helper_result_types.join(", ")
        ));
    }
    blocks.push(String::new());
    blocks.push(format!("class {agent_name}PartialIntentHandlers(TypedDict, total=False):"));
    blocks.push(format!(
        "    response_text: Callable[[{agent_name}ResponseTextIntent], None]"
    ));
    blocks.push(format!(
        "    response_schema: Callable[[{agent_name}ResponseSchemaIntent], None]"
    ));
    blocks.push(format!(
        "    error: Callable[[{agent_name}ErrorIntent], None]"
    ));
    if !workflow_call_types.is_empty() {
        blocks.push(format!(
            "    workflow_call: Callable[[Union[{}]], None]",
            workflow_call_types.join(", ")
        ));
        blocks.push(format!(
            "    workflow_result: Callable[[Union[{}]], None]",
            workflow_result_types.join(", ")
        ));
    }
    if !helper_call_types.is_empty() {
        blocks.push(format!(
            "    helper_call: Callable[[Union[{}]], None]",
            helper_call_types.join(", ")
        ));
        blocks.push(format!(
            "    helper_result: Callable[[Union[{}]], None]",
            helper_result_types.join(", ")
        ));
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
    config_keys.push(format!("    middleware: NotRequired[List[Union['{agent_name}Middleware', 'type[{agent_name}Middleware]']]]"));


    if matches!(ir.get("context"), Some(context) if !context.is_null()) {
        config_keys.push(format!("    context: NotRequired['{agent_name}Context']"));
    }
    if has_api_keys {
        config_keys.push(format!("    apiKeys: NotRequired['{agent_name}ApiKeys']"));
    }

    let mut on_intent_overloads = vec![
        format!("    @overload"),
        format!("    def on_intent(self, callback: Callable[[Literal[\"response_text\"], {agent_name}ResponseTextIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
        format!("    @overload"),
        format!("    def on_intent(self, callback: Callable[[Literal[\"response_schema\"], {agent_name}ResponseSchemaIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
        format!("    @overload"),
        format!("    def on_intent(self, callback: Callable[[Literal[\"error\"], {agent_name}ErrorIntent, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
    ];
    let mut on_partial_overloads = vec![
        format!("    @overload"),
        format!("    def on_intent_partial(self, callback: Callable[[Literal[\"response_text\"], {agent_name}ResponseTextIntent, str], None]) -> None: ..."),
        format!("    @overload"),
        format!("    def on_intent_partial(self, callback: Callable[[Literal[\"response_schema\"], {agent_name}ResponseSchemaIntent, str], None]) -> None: ..."),
        format!("    @overload"),
        format!("    def on_intent_partial(self, callback: Callable[[Literal[\"error\"], {agent_name}ErrorIntent, str], None]) -> None: ..."),
    ];

    if ir
        .get("workflows")
        .and_then(Value::as_array)
        .map(|workflows| !workflows.is_empty())
        .unwrap_or(false)
    {
        on_intent_overloads.extend([
            "    @overload".to_string(),
            format!("    def on_intent(self, callback: Callable[[Literal[\"workflow_call\"], {agent_name}WorkflowCallIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
            "    @overload".to_string(),
            format!("    def on_intent(self, callback: Callable[[Literal[\"workflow_result\"], {agent_name}WorkflowResultIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
        ]);
        on_partial_overloads.extend([
            "    @overload".to_string(),
            format!("    def on_intent_partial(self, callback: Callable[[Literal[\"workflow_call\"], {agent_name}WorkflowCallIntentValue, str], None]) -> None: ..."),
            "    @overload".to_string(),
            format!("    def on_intent_partial(self, callback: Callable[[Literal[\"workflow_result\"], {agent_name}WorkflowResultIntentValue, str], None]) -> None: ..."),
        ]);
    }

    if ir
        .get("helpers")
        .and_then(Value::as_array)
        .map(|helpers| !helpers.is_empty())
        .unwrap_or(false)
    {
        on_intent_overloads.extend([
            "    @overload".to_string(),
            format!("    def on_intent(self, callback: Callable[[Literal[\"helper_call\"], {agent_name}HelperCallIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
            "    @overload".to_string(),
            format!("    def on_intent(self, callback: Callable[[Literal[\"helper_result\"], {agent_name}HelperResultIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."),
        ]);
        on_partial_overloads.extend([
            "    @overload".to_string(),
            format!("    def on_intent_partial(self, callback: Callable[[Literal[\"helper_call\"], {agent_name}HelperCallIntentValue, str], None]) -> None: ..."),
            "    @overload".to_string(),
            format!("    def on_intent_partial(self, callback: Callable[[Literal[\"helper_result\"], {agent_name}HelperResultIntentValue, str], None]) -> None: ..."),
        ]);
    }

    [
        format!("class {agent_name}Agent(TypedAuwgent[Any, {agent_name}Context, {agent_name}Output, {agent_name}Tools]):"),
        on_intent_overloads.join("\n"),
        format!("    def on_intent(self, callback: {agent_name}IntentHandler) -> None:"),
        "        return super().on_intent(callback)".to_string(),
        String::new(),
        on_partial_overloads.join("\n"),
        format!("    def on_intent_partial(self, callback: {agent_name}PartialIntentHandler) -> None:"),
        "        return super().on_intent_partial(callback)".to_string(),
        String::new(),
        format!("    def on_handlers(self, handlers: {agent_name}IntentHandlers) -> None:"),
        "        return super().on_handlers(handlers)".to_string(),
        String::new(),
        format!("    def on_handlers_partial(self, handlers: {agent_name}PartialIntentHandlers) -> None:"),
        "        return super().on_handlers_partial(handlers)".to_string(),
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
        format!("AuwgentIntentHandlers = {agent_name}IntentHandlers"),
        format!("AuwgentPartialIntentHandlers = {agent_name}PartialIntentHandlers"),
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

        let output = generate(&ir, "main");
        assert!(output.contains("class ManagerdeleteAccountWorkflowArgs(TypedDict, total=False):"));
        assert!(output.contains("class ManagerReviewerHelperArgs(TypedDict, total=False):"));
        assert!(output.contains("ManagerIntentHandler = Callable[[ManagerIntentName, ManagerIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]"));
        assert!(output.contains("def on_intent(self, callback: Callable[[Literal[\"workflow_call\"], ManagerWorkflowCallIntentValue, str], Awaitable[Optional[Dict[str, Any]]]]) -> None: ..."));
        assert!(output.contains("class ManagerIntentHandlers(TypedDict, total=False):"));
    }
}
