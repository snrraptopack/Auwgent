use crate::common::{
    array_at, collect_custom_provider_ids, collect_helper_tools, collect_required_providers,
    collect_workflow_tools, join_sections, merge_tool_defs, object_at, string_at,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub fn generate(ir: &Value, _base_name: &str) -> String {
    let agent_name = string_at(ir, &["name"]).unwrap_or("Agent");
    let workflow_tools = collect_workflow_tools(ir);
    let helper_tools = collect_helper_tools(ir);
    let all_tools = merge_tool_defs(
        array_at(ir, &["tools"]),
        workflow_tools.into_iter().chain(helper_tools.into_iter()).collect(),
    );
    let has_tools = !all_tools.is_empty();
    let has_context = ir
        .get("context")
        .and_then(Value::as_object)
        .map(|context| !context.is_empty())
        .unwrap_or(false);
    let required_providers = collect_required_providers(ir);
    let custom_provider_ids = collect_custom_provider_ids(ir);
    let custom_intents = collect_custom_intents(ir);
    let has_workflows = !array_at(ir, &["workflows"]).is_empty();
    let has_helpers = !array_at(ir, &["helpers"]).is_empty();
    let has_components = !array_at(ir, &["components"]).is_empty();

    let mut sections = vec![
        format!("// Auto-generated Rust bindings for {agent_name}"),
        "// Do not edit manually".to_string(),
        String::new(),
        "use serde_json::{Map as JsonMap, Value as JsonValue};".to_string(),
        String::new(),
    ];

    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        sections.push(generate_custom_types(types));
    }

    sections.push(generate_named_shape(
        &format!("{agent_name}Input"),
        unwrap_input_fields(ir.get("input")).as_ref(),
    ));
    sections.push(generate_named_shape(
        &format!("{agent_name}Output"),
        ir.get("output"),
    ));
    sections.push(generate_named_shape(
        &format!("{agent_name}Context"),
        ir.get("context"),
    ));

    if has_tools {
        sections.push(generate_tools(agent_name, &all_tools));
    }

    sections.push(generate_intent_name_enum(
        ir,
        agent_name,
        has_tools,
        has_workflows,
        has_helpers,
        has_components,
        &custom_intents,
    ));
    sections.push(generate_core_intents(agent_name));
    sections.push(generate_callable_intents(agent_name, "Tool", &all_tools, "name", "params", "returns"));

    if has_helpers {
        sections.push(generate_callable_intents(
            agent_name,
            "Helper",
            array_at(ir, &["helpers"]),
            "name",
            "input",
            "output",
        ));
    }

    sections.push(generate_handler_traits(
        agent_name,
        has_tools,
        has_helpers,
    ));
    sections.push(generate_api_keys(
        agent_name,
        &required_providers,
        &custom_provider_ids,
    ));
    sections.push(generate_middleware_trait(agent_name));
    sections.push(generate_config(
        agent_name,
        has_tools,
        has_context,
        !required_providers.is_empty(),
    ));
    sections.push(generate_agent(agent_name));
    sections.push(generate_aliases(
        agent_name,
        !required_providers.is_empty(),
    ));

    join_sections(&sections)
}

fn generate_custom_types(types: &Map<String, Value>) -> String {
    let mut blocks = Vec::new();
    for (type_name, type_def) in types {
        blocks.push(generate_named_shape(type_name, Some(type_def)));
    }
    blocks.join("\n")
}

fn generate_named_shape(name: &str, value: Option<&Value>) -> String {
    if let Some(properties) = shape_properties(value, false) {
        return generate_struct(name, &properties);
    }

    format!("pub type {name} = {};\n", rust_type(value, false, "JsonValue"))
}

fn generate_struct(name: &str, properties: &Map<String, Value>) -> String {
    let mut fields = Vec::new();
    for (prop_name, prop_info) in properties {
        if prop_name.starts_with('@') || prop_name.starts_with("__") {
            continue;
        }

        let optional = prop_info
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let field_name = to_rust_field_name(prop_name);
        let field_type = rust_type(Some(prop_info), optional, "JsonValue");
        fields.push(format!("    pub {field_name}: {field_type},"));
    }

    if fields.is_empty() {
        return format!(
            "#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]\npub struct {name};\n"
        );
    }

    format!(
        "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {name} {{\n{}\n}}\n",
        fields.join("\n")
    )
}

fn generate_tools(agent_name: &str, tools: &[Value]) -> String {
    let mut result_aliases = Vec::new();
    let mut methods = Vec::new();

    for tool in tools {
        let Some(tool_name) = string_at(tool, &["name"]) else {
            continue;
        };
        let pascal = to_rust_type_name(tool_name);
        let method_name = to_rust_field_name(tool_name);
        let args_shape = tool.get("params");
        let returns_shape = tool.get("returns");

        if !is_empty_shape(args_shape, false) {
            result_aliases.push(generate_named_shape(
                &format!("{agent_name}{pascal}ToolArgs"),
                args_shape,
            ));
        }

        let result_alias = format!("{agent_name}{pascal}ToolResultValue");
        result_aliases.push(format!(
            "pub type {result_alias} = {};\n",
            rust_type(returns_shape, false, "()")
        ));

        let method = if is_empty_shape(args_shape, false) {
            format!("    fn {method_name}(&self) -> {result_alias};")
        } else {
            format!(
                "    fn {method_name}(&self, args: {agent_name}{pascal}ToolArgs) -> {result_alias};"
            )
        };
        methods.push(method);
    }

    format!(
        "{}\npub trait {agent_name}Tools {{\n{}\n}}\n",
        result_aliases.join("\n"),
        methods.join("\n")
    )
}

fn generate_intent_name_enum(
    _ir: &Value,
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut names = vec![
        "ResponseText".to_string(),
        "ResponseSchema".to_string(),
        "Error".to_string(),
    ];

    if has_tools {
        names.extend([
            "ToolCall".to_string(),
            "ToolResult".to_string(),
            "ToolError".to_string(),
            "ToolSkipped".to_string(),
        ]);
    }
    if has_workflows {
        names.extend(["WorkflowCall".to_string(), "WorkflowResult".to_string()]);
    }
    if has_helpers {
        names.extend(["HelperCall".to_string(), "HelperResult".to_string()]);
    }
    if has_components {
        names.extend(["Component".to_string(), "RenderComponent".to_string()]);
    }
    for custom_intent in custom_intents {
        names.push(to_rust_type_name(custom_intent));
    }

    let variants = names
        .iter()
        .map(|name| format!("    {name},"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {agent_name}IntentName {{\n{variants}\n}}\n"
    )
}

fn generate_core_intents(agent_name: &str) -> String {
    [
        format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ResponseTextIntent {{\n    pub text: String,\n}}\n"
        ),
        format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ResponseSchemaIntent {{\n    pub r#type: String,\n    pub response: {agent_name}Output,\n}}\n"
        ),
        format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}ErrorIntent {{\n    pub message: String,\n}}\n"
        ),
    ]
    .join("\n")
}

fn generate_callable_intents(
    agent_name: &str,
    family_name: &str,
    items: &[Value],
    name_key: &str,
    args_key: &str,
    result_key: &str,
) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut blocks = Vec::new();
    let mut call_variants = Vec::new();
    let mut result_variants = Vec::new();

    for item in items {
        let Some(item_name) = string_at(item, &[name_key]) else {
            continue;
        };
        let pascal = to_rust_type_name(item_name);
        let args_shape = item.get(args_key);
        let result_shape = item.get(result_key);
        let args_type = if is_empty_shape(args_shape, false) {
            "()".to_string()
        } else {
            let name = format!("{agent_name}{pascal}{family_name}Args");
            blocks.push(generate_named_shape(&name, args_shape));
            name
        };
        let result_type = if result_shape.is_none() || result_shape.is_some_and(Value::is_null) {
            "()".to_string()
        } else {
            let name = format!("{agent_name}{pascal}{family_name}ResultValue");
            blocks.push(format!(
                "pub type {name} = {};\n",
                rust_type(result_shape, false, "()")
            ));
            name
        };

        let call_variant = format!("{pascal}({agent_name}{pascal}{family_name}CallIntent)");
        let result_variant = format!("{pascal}({agent_name}{pascal}{family_name}ResultIntent)");
        call_variants.push(call_variant);
        result_variants.push(result_variant);

        blocks.push(format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}{pascal}{family_name}CallIntent {{\n    pub r#type: String,\n    pub args: {args_type},\n}}\n"
        ));
        blocks.push(format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\npub struct {agent_name}{pascal}{family_name}ResultIntent {{\n    pub name: String,\n    pub args: {args_type},\n    pub result: {result_type},\n    pub overridden: bool,\n}}\n"
        ));
    }

    blocks.push(format!(
        "#[derive(Debug, Clone)]\npub enum {agent_name}{family_name}CallIntent {{\n{}\n}}\n",
        call_variants
            .iter()
            .map(|variant| format!("    {variant},"))
            .collect::<Vec<_>>()
            .join("\n")
    ));
    blocks.push(format!(
        "#[derive(Debug, Clone)]\npub enum {agent_name}{family_name}ResultIntent {{\n{}\n}}\n",
        result_variants
            .iter()
            .map(|variant| format!("    {variant},"))
            .collect::<Vec<_>>()
            .join("\n")
    ));

    blocks.join("\n")
}

fn generate_handler_traits(agent_name: &str, has_tools: bool, has_helpers: bool) -> String {
    let mut intent_methods = vec![
        format!(
            "    fn response_text(&mut self, intent: {agent_name}ResponseTextIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
        ),
        format!(
            "    fn response_schema(&mut self, intent: {agent_name}ResponseSchemaIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
        ),
        format!(
            "    fn error(&mut self, intent: {agent_name}ErrorIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
        ),
    ];
    let mut partial_methods = vec![
        "    fn response_text(&mut self, intent: PartialTextIntentValue, agent_name: &str) { let _ = (intent, agent_name); }".to_string(),
        format!(
            "    fn response_schema(&mut self, intent: PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
        ),
        format!(
            "    fn error(&mut self, intent: PartialStructuredIntentValue<{agent_name}ErrorIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
        ),
    ];

    if has_tools {
        intent_methods.extend([
            format!(
                "    fn tool_call(&mut self, intent: {agent_name}ToolCallIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
            ),
            format!(
                "    fn tool_result(&mut self, intent: {agent_name}ToolResultIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
            ),
        ]);
        partial_methods.extend([
            format!(
                "    fn tool_call(&mut self, intent: PartialStructuredIntentValue<{agent_name}ToolCallIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
            ),
            format!(
                "    fn tool_result(&mut self, intent: PartialStructuredIntentValue<{agent_name}ToolResultIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
            ),
        ]);
    }
    if has_helpers {
        intent_methods.extend([
            format!(
                "    fn helper_call(&mut self, intent: {agent_name}HelperCallIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
            ),
            format!(
                "    fn helper_result(&mut self, intent: {agent_name}HelperResultIntent, agent_name: &str) -> Option<IntentControl> {{ let _ = (intent, agent_name); None }}"
            ),
        ]);
        partial_methods.extend([
            format!(
                "    fn helper_call(&mut self, intent: PartialStructuredIntentValue<{agent_name}HelperCallIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
            ),
            format!(
                "    fn helper_result(&mut self, intent: PartialStructuredIntentValue<{agent_name}HelperResultIntent>, agent_name: &str) {{ let _ = (intent, agent_name); }}"
            ),
        ]);
    }

    [
        "pub type IntentControl = JsonValue;".to_string(),
        "pub type PartialTextIntentValue = JsonValue;".to_string(),
        "pub type PartialStructuredIntentValue<T> = JsonValue;".to_string(),
        format!(
            "pub trait {agent_name}BaseIntentHandler {{\n{}\n}}\n",
            intent_methods.join("\n")
        ),
        format!(
            "pub trait {agent_name}BasePartialIntentHandler {{\n{}\n}}\n",
            partial_methods.join("\n")
        ),
        generate_dispatch_functions(agent_name, has_tools, has_helpers),
    ]
    .join("\n")
}

fn generate_dispatch_functions(agent_name: &str, has_tools: bool, has_helpers: bool) -> String {
    let mut intent_cases = vec![
        format!(
            "        {agent_name}IntentName::ResponseText => handler.response_text(serde_json::from_value(value).unwrap(), agent_name),"
        ),
        format!(
            "        {agent_name}IntentName::ResponseSchema => handler.response_schema(serde_json::from_value(value).unwrap(), agent_name),"
        ),
        format!(
            "        {agent_name}IntentName::Error => handler.error(serde_json::from_value(value).unwrap(), agent_name),"
        ),
    ];
    let mut partial_cases = vec![
        format!(
            "        {agent_name}IntentName::ResponseText => handler.response_text(value, agent_name),"
        ),
        format!(
            "        {agent_name}IntentName::ResponseSchema => handler.response_schema(value, agent_name),"
        ),
        format!(
            "        {agent_name}IntentName::Error => handler.error(value, agent_name),"
        ),
    ];

    if has_tools {
        intent_cases.extend([
            format!(
                "        {agent_name}IntentName::ToolCall => handler.tool_call(serde_json::from_value(value).unwrap(), agent_name),"
            ),
            format!(
                "        {agent_name}IntentName::ToolResult => handler.tool_result(serde_json::from_value(value).unwrap(), agent_name),"
            ),
        ]);
        partial_cases.extend([
            format!(
                "        {agent_name}IntentName::ToolCall => handler.tool_call(value, agent_name),"
            ),
            format!(
                "        {agent_name}IntentName::ToolResult => handler.tool_result(value, agent_name),"
            ),
        ]);
    }
    if has_helpers {
        intent_cases.extend([
            format!(
                "        {agent_name}IntentName::HelperCall => handler.helper_call(serde_json::from_value(value).unwrap(), agent_name),"
            ),
            format!(
                "        {agent_name}IntentName::HelperResult => handler.helper_result(serde_json::from_value(value).unwrap(), agent_name),"
            ),
        ]);
        partial_cases.extend([
            format!(
                "        {agent_name}IntentName::HelperCall => handler.helper_call(value, agent_name),"
            ),
            format!(
                "        {agent_name}IntentName::HelperResult => handler.helper_result(value, agent_name),"
            ),
        ]);
    }

    format!(
        "pub fn dispatch_intent<H: {agent_name}BaseIntentHandler>(handler: &mut H, name: {agent_name}IntentName, value: JsonValue, agent_name: &str) -> Option<IntentControl> {{\n    match name {{\n{}\n    }}\n}}\n\npub fn dispatch_partial_intent<H: {agent_name}BasePartialIntentHandler>(handler: &mut H, name: {agent_name}IntentName, value: JsonValue, agent_name: &str) {{\n    match name {{\n{}\n    }}\n}}\n",
        intent_cases.join("\n"),
        partial_cases.join("\n")
    )
}

fn generate_api_keys(
    agent_name: &str,
    required_providers: &BTreeSet<String>,
    custom_provider_ids: &BTreeSet<String>,
) -> String {
    if required_providers.is_empty() {
        return String::new();
    }

    let mut fields = Vec::new();

    if required_providers.contains("openai") {
        fields.push("    pub openai_api_key: Option<String>,".to_string());
    }
    if required_providers.contains("gemini") {
        fields.push("    pub gemini_api_key: Option<String>,".to_string());
    }
    if required_providers.contains("groq") {
        fields.push("    pub groq_api_key: Option<String>,".to_string());
    }
    for id in custom_provider_ids {
        fields.push(format!(
            "    pub {}_api_key: Option<String>,",
            to_rust_field_name(&id.replace('-', "_"))
        ));
    }

    format!(
        "#[derive(Debug, Clone, Default)]\npub struct {agent_name}ApiKeys {{\n{}\n}}\n",
        fields.join("\n")
    )
}

fn generate_middleware_trait(agent_name: &str) -> String {
    format!(
        "pub type MiddlewareContext = JsonMap<String, JsonValue>;\npub type SessionState = JsonValue;\n\npub trait {agent_name}Middleware {{\n    fn name(&self) -> &'static str;\n\n    fn on_run_start(&mut self, session: SessionState, ctx: &mut MiddlewareContext) -> SessionState {{ let _ = ctx; session }}\n    fn on_llm_start(&mut self, prompt: String, ctx: &mut MiddlewareContext) -> Option<String> {{ let _ = (prompt, ctx); None }}\n    fn on_intent(&mut self, name: {agent_name}IntentName, value: JsonValue, ctx: &mut MiddlewareContext) -> Option<IntentControl> {{ let _ = (name, value, ctx); None }}\n    fn on_intent_partial(&mut self, name: {agent_name}IntentName, value: JsonValue, ctx: &mut MiddlewareContext) {{ let _ = (name, value, ctx); }}\n    fn on_llm_end(&mut self, response: JsonValue, ctx: &mut MiddlewareContext) {{ let _ = (response, ctx); }}\n    fn on_run_complete(&mut self, final_session: SessionState, ctx: &mut MiddlewareContext) {{ let _ = (final_session, ctx); }}\n    fn on_error(&mut self, error: JsonValue, session: Option<SessionState>, ctx: &mut MiddlewareContext) -> bool {{ let _ = (error, session, ctx); false }}\n}}\n"
    )
}

fn generate_config(
    agent_name: &str,
    has_tools: bool,
    has_context: bool,
    has_api_keys: bool,
) -> String {
    let mut fields = Vec::new();
    if has_tools {
        fields.push(format!("    pub tools: {agent_name}ToolsRegistry,"));
    }
    fields.push(format!("    pub middleware: Vec<{agent_name}MiddlewareRegistry>,"));
    if has_context {
        fields.push(format!("    pub context: {agent_name}Context,"));
    }
    if has_api_keys {
        fields.push(format!("    pub api_keys: {agent_name}ApiKeys,"));
    }

    let tools_registry = if has_tools {
        format!("pub type {agent_name}ToolsRegistry = Box<dyn {agent_name}Tools>;")
    } else {
        format!("pub type {agent_name}ToolsRegistry = ();")
    };

    format!(
        "{tools_registry}\npub type {agent_name}MiddlewareRegistry = Box<dyn {agent_name}Middleware>;\n\n#[derive(Debug)]\npub struct {agent_name}Config {{\n{}\n}}\n",
        fields.join("\n")
    )
}

fn generate_agent(agent_name: &str) -> String {
    format!(
        "pub struct {agent_name}Agent {{\n    pub config: {agent_name}Config,\n}}\n\nimpl {agent_name}Agent {{\n    pub fn on_intent<F>(&mut self, _handler: F)\n    where\n        F: FnMut({agent_name}IntentName, JsonValue, &str) -> Option<IntentControl> + 'static,\n    {{}}\n\n    pub fn on_intent_partial<F>(&mut self, _handler: F)\n    where\n        F: FnMut({agent_name}IntentName, JsonValue, &str) + 'static,\n    {{}}\n\n    pub fn on_intent_handler<H>(&mut self, _handler: H)\n    where\n        H: {agent_name}BaseIntentHandler + 'static,\n    {{}}\n\n    pub fn on_intent_partial_handler<H>(&mut self, _handler: H)\n    where\n        H: {agent_name}BasePartialIntentHandler + 'static,\n    {{}}\n}}\n\npub fn create_{snake_agent_name}(config: {agent_name}Config) -> {agent_name}Agent {{\n    {agent_name}Agent {{ config }}\n}}\n\npub fn auwgent(config: {agent_name}Config) -> {agent_name}Agent {{\n    create_{snake_agent_name}(config)\n}}\n",
        snake_agent_name = to_rust_field_name(agent_name)
    )
}

fn generate_aliases(agent_name: &str, has_api_keys: bool) -> String {
    let mut aliases = vec![
        format!("pub use {agent_name}Agent as AuwgentAgent;"),
        format!("pub use {agent_name}Config as AuwgentConfig;"),
        format!("pub use {agent_name}Tools as AuwgentTools;"),
        format!("pub use {agent_name}Context as AuwgentContext;"),
        format!("pub use {agent_name}MiddlewareRegistry as AuwgentMiddleware;"),
        format!("pub use {agent_name}IntentName as AuwgentIntentName;"),
        format!("pub use {agent_name}BaseIntentHandler as AuwgentBaseIntentHandler;"),
        format!("pub use {agent_name}BasePartialIntentHandler as AuwgentBasePartialIntentHandler;"),
    ];
    if has_api_keys {
        aliases.push(format!("pub use {agent_name}ApiKeys as AuwgentApiKeys;"));
    }
    aliases.join("\n")
}

fn collect_custom_intents(ir: &Value) -> Vec<String> {
    let mut intents = BTreeSet::new();
    if let Some(items) = ir.get("customIntents").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = string_at(item, &["name"]) {
                intents.insert(name.to_string());
            }
        }
    }
    intents.into_iter().collect()
}

fn unwrap_input_fields(value: Option<&Value>) -> Option<Value> {
    let value = value?;
    if value.get("kind").and_then(Value::as_str) == Some("properties") {
        return value.get("fields").cloned();
    }
    Some(value.clone())
}

fn shape_properties(value: Option<&Value>, unwrap_input_kind: bool) -> Option<Map<String, Value>> {
    let value = value?;
    let obj = value.as_object()?;

    if unwrap_input_kind
        && obj.get("kind").and_then(Value::as_str) == Some("properties")
    {
        return obj.get("fields").and_then(Value::as_object).cloned();
    }

    if obj.get("properties").and_then(Value::as_object).is_some() {
        return obj.get("properties").and_then(Value::as_object).cloned();
    }

    if obj.get("fields").and_then(Value::as_object).is_some() {
        return obj.get("fields").and_then(Value::as_object).cloned();
    }

    if obj.contains_key("__variants") {
        return None;
    }

    if !obj.contains_key("type") && !obj.contains_key("kind") {
        return Some(obj.clone());
    }

    if obj.get("type").and_then(Value::as_str) == Some("object") {
        return obj.get("properties").and_then(Value::as_object).cloned();
    }

    None
}

fn is_empty_shape(value: Option<&Value>, unwrap_input_kind: bool) -> bool {
    shape_properties(value, unwrap_input_kind)
        .map(|properties| {
            properties
                .iter()
                .filter(|(name, _)| !name.starts_with('@') && !name.starts_with("__"))
                .count()
                == 0
        })
        .unwrap_or(false)
}

fn rust_type(value: Option<&Value>, optional: bool, fallback: &str) -> String {
    let base = rust_type_base(value, fallback);
    if optional {
        format!("Option<{base}>")
    } else {
        base
    }
}

fn rust_type_base(value: Option<&Value>, fallback: &str) -> String {
    let Some(value) = value else {
        return fallback.to_string();
    };

    if let Some(properties) = shape_properties(Some(value), false)
        && !properties.is_empty()
    {
        return "JsonValue".to_string();
    }

    if let Some(obj) = value.as_object() {
        if let Some(type_name) = obj.get("type").and_then(Value::as_str) {
            return match type_name {
                "string" => "String".to_string(),
                "number" => "f64".to_string(),
                "boolean" => "bool".to_string(),
                "array" => {
                    let item_type = rust_type(obj.get("items"), false, "JsonValue");
                    format!("Vec<{item_type}>")
                }
                "typeRef" => obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(to_rust_type_name)
                    .unwrap_or_else(|| fallback.to_string()),
                "object" => "JsonValue".to_string(),
                _ => fallback.to_string(),
            };
        }

        if let Some(type_obj) = obj.get("type").and_then(Value::as_object)
            && let Some(type_name) = type_obj.get("type").and_then(Value::as_str)
        {
            return match type_name {
                "typeRef" => type_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(to_rust_type_name)
                    .unwrap_or_else(|| fallback.to_string()),
                "array" => {
                    let item_type = rust_type(type_obj.get("items"), false, "JsonValue");
                    format!("Vec<{item_type}>")
                }
                "object" => "JsonValue".to_string(),
                _ => fallback.to_string(),
            };
        }
    }

    match value {
        Value::String(_) => "String".to_string(),
        Value::Number(_) => "f64".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Array(_) => "Vec<JsonValue>".to_string(),
        Value::Object(_) => "JsonValue".to_string(),
        Value::Null => fallback.to_string(),
    }
}

fn to_rust_type_name(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if uppercase_next {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            } else {
                out.extend(ch.to_lowercase());
            }
        } else {
            uppercase_next = true;
        }
    }
    if out.is_empty() {
        "Unknown".to_string()
    } else {
        out
    }
}

fn to_rust_field_name(name: &str) -> String {
    let mut out = String::new();
    let mut underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if underscore && !out.is_empty() {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            underscore = false;
        } else {
            underscore = true;
        }
    }
    if out.is_empty() {
        "value".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn emits_conditional_config_and_exhaustive_intent_name_enum() {
        let ir = json!({
            "name": "Hello",
            "context": {
                "user_id": { "type": "string", "optional": false }
            },
            "tools": [{
                "name": "get_details",
                "params": {},
                "returns": {
                    "type": "typeRef",
                    "name": "Person"
                }
            }],
            "helpers": [{
                "name": "Joker",
                "input": null,
                "output": null
            }],
            "types": {
                "Person": {
                    "properties": {
                        "name": { "type": "string", "optional": false },
                        "age": { "type": "number", "optional": false }
                    }
                }
            },
            "customIntents": [{
                "name": "ask_user",
                "fields": {
                    "question": { "type": "string", "optional": false }
                }
            }],
            "modelConfig": [{
                "defaultConfig": {
                    "model": {
                        "type": "openai",
                        "modelName": "gpt-4.1"
                    }
                }
            }]
        });

        let output = generate(&ir, "hello");
        assert!(output.contains("pub struct Person"));
        assert!(output.contains("pub type HelloGetDetailsToolResultValue = Person;"));
        assert!(output.contains("pub trait HelloTools"));
        assert!(output.contains("pub enum HelloIntentName"));
        assert!(output.contains("ResponseText"));
        assert!(output.contains("ToolCall"));
        assert!(output.contains("HelperCall"));
        assert!(output.contains("AskUser"));
        assert!(output.contains("pub struct HelloConfig"));
        assert!(output.contains("pub tools: HelloToolsRegistry,"));
        assert!(output.contains("pub middleware: Vec<HelloMiddlewareRegistry>,"));
        assert!(output.contains("pub context: HelloContext,"));
        assert!(output.contains("pub api_keys: HelloApiKeys,"));
        assert!(output.contains("pub fn dispatch_intent"));
        assert!(output.contains("match name"));
        assert!(output.contains("pub fn auwgent(config: HelloConfig) -> HelloAgent"));
        assert!(output.contains("pub use HelloIntentName as AuwgentIntentName;"));
    }

    #[test]
    fn omits_conditional_fields_when_not_needed() {
        let ir = json!({
            "name": "Mini",
            "tools": [],
            "helpers": [],
            "components": [],
            "modelConfig": []
        });

        let output = generate(&ir, "mini");
        assert!(output.contains("pub struct MiniConfig"));
        assert!(output.contains("pub middleware: Vec<MiniMiddlewareRegistry>,"));
        assert!(!output.contains("pub tools: MiniToolsRegistry,"));
        assert!(!output.contains("pub context: MiniContext,"));
        assert!(!output.contains("pub api_keys: MiniApiKeys,"));
    }
}
