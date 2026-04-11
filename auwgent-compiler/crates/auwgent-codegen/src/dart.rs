use crate::common::{
    array_at, collect_custom_provider_ids, collect_handoff_helpers, collect_required_providers,
    collect_transferred_helpers, join_sections, merge_helpers, object_at, string_at,
};
use serde_json::{Map, Value};

pub fn generate(ir: &Value, base_name: &str) -> String {
    let agent_name = string_at(ir, &["name"]).unwrap_or("Agent");
    let output_helpers = merge_helpers(
        collect_transferred_helpers(ir),
        collect_handoff_helpers(ir),
    );
    let required_providers = collect_required_providers(ir);
    let custom_provider_ids = collect_custom_provider_ids(ir);
    let workflows = array_at(ir, &["workflows"]);
    let helpers = array_at(ir, &["helpers"]);
    let custom_intents = collect_custom_intents(ir);
    let has_tools = !array_at(ir, &["tools"]).is_empty();
    let has_components = !array_at(ir, &["components"]).is_empty();

    let mut sections = vec![
        format!("// Auto-generated Dart bindings for {agent_name}"),
        "// Do not edit manually".to_string(),
        "import 'dart:async';".to_string(),
        "import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;".to_string(),
        format!("import '{base_name}.agent.ir.dart';"),
        String::new(),
    ];

    if let Some(types) = ir.get("types").and_then(Value::as_object) {
        sections.push(generate_custom_types(types));
    }

    sections.push(generate_type_alias(
        &format!("{agent_name}Input"),
        ir.get("input"),
        true,
        "String",
    ));

    for helper in &output_helpers {
        if let Some(name) = string_at(helper, &["name"]) {
            sections.push(generate_type_alias(
                &format!("{name}Output"),
                helper.get("output"),
                false,
                "sdk.JsonMap",
            ));
        }
    }

    sections.push(generate_type_alias(
        &format!("{agent_name}Output"),
        ir.get("output"),
        false,
        "sdk.JsonMap",
    ));
    sections.push(generate_type_alias(
        &format!("{agent_name}Context"),
        ir.get("context"),
        false,
        "sdk.JsonMap",
    ));
    sections.push(format!(
        "typedef {agent_name}Tools = Map<String, sdk.ToolHandler>;\n"
    ));
    sections.push(generate_core_intent_models(agent_name));

    sections.push(generate_intent_aliases(
        agent_name,
        has_tools,
        !workflows.is_empty(),
        !helpers.is_empty(),
        has_components,
        &custom_intents,
    ));
    sections.push(generate_handler_classes(
        agent_name,
        has_tools,
        !workflows.is_empty(),
        !helpers.is_empty(),
        has_components,
        &custom_intents,
    ));

    if !required_providers.is_empty() || !custom_provider_ids.is_empty() {
        sections.push(generate_api_keys(
            agent_name,
            &required_providers,
            &custom_provider_ids,
        ));
    }

    sections.push(generate_config_class(
        agent_name,
        !required_providers.is_empty() || !custom_provider_ids.is_empty(),
        matches!(ir.get("context"), Some(v) if !v.is_null()),
    ));
    sections.push(generate_agent_class(
        agent_name,
        has_tools,
        !workflows.is_empty(),
        !helpers.is_empty(),
        has_components,
        &custom_intents,
    ));
    sections.push(generate_factory(
        agent_name,
        !required_providers.is_empty() || !custom_provider_ids.is_empty(),
        has_tools,
        !workflows.is_empty(),
        !helpers.is_empty(),
        has_components,
        &custom_intents,
    ));

    join_sections(&sections)
}

pub fn generate_ir_module(ir: &Value) -> String {
    let agent_name = string_at(ir, &["name"]).unwrap_or("Agent");
    let ir_json = serde_json::to_string_pretty(ir).unwrap_or_else(|_| "{}".to_string());

    [
        format!("// Auto-generated Dart IR for {agent_name}"),
        "// Do not edit manually".to_string(),
        "import 'dart:convert';".to_string(),
        "import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;".to_string(),
        String::new(),
        format!(
            "const String _{}AgentIrJson = r'''{ir_json}''';",
            sanitize_identifier(agent_name, false)
        ),
        String::new(),
        format!(
            "sdk.JsonMap decode{}Ir() {{\n  return Map<String, Object?>.from(jsonDecode(_{}AgentIrJson) as Map);\n}}\n",
            sanitize_identifier(agent_name, false),
            sanitize_identifier(agent_name, false),
        ),
    ]
    .join("\n")
}

fn collect_custom_intents(ir: &Value) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(items) = ir.get("customIntents").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = string_at(item, &["name"]) {
                if !names.iter().any(|existing| existing == name) {
                    names.push(name.to_string());
                }
            }
        }
    }

    for helper in array_at(ir, &["helpers"]) {
        if let Some(items) = helper.get("customIntents").and_then(Value::as_array) {
            for item in items {
                if let Some(name) = string_at(item, &["name"]) {
                    if !names.iter().any(|existing| existing == name) {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }

    names
}

fn generate_custom_types(types: &Map<String, Value>) -> String {
    let mut blocks = Vec::new();
    for (type_name, type_def) in types {
        blocks.push(generate_type_alias(type_name, Some(type_def), false, "sdk.JsonMap"));
    }
    blocks.join("\n")
}

fn generate_type_alias(
    name: &str,
    value: Option<&Value>,
    unwrap_input_kind: bool,
    null_fallback: &str,
) -> String {
    format!(
        "typedef {name} = {};\n",
        type_to_dart_string(value, unwrap_input_kind, null_fallback)
    )
}

fn generate_intent_aliases(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut lines = vec![
        format!("typedef {agent_name}IntentValue = Object?;"),
        format!("typedef {agent_name}IntentControl = Object?;"),
        format!(
            "typedef {agent_name}IntentHandler = FutureOr<{agent_name}IntentControl> Function(String name, Object? value, String agentName);"
        ),
        format!(
            "typedef {agent_name}PartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);"
        ),
    ];

    if has_tools {
        lines.extend([
            format!("typedef {agent_name}ToolCallIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}ToolResultIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}ToolErrorIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}ToolSkippedIntent = sdk.JsonMap;"),
        ]);
    }

    if has_workflows {
        lines.extend([
            format!("typedef {agent_name}WorkflowCallIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}WorkflowResultIntent = sdk.JsonMap;"),
        ]);
    }

    if has_helpers {
        lines.extend([
            format!("typedef {agent_name}HelperCallIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}HelperResultIntent = sdk.JsonMap;"),
        ]);
    }

    if has_components {
        lines.extend([
            format!("typedef {agent_name}ComponentIntent = sdk.JsonMap;"),
            format!("typedef {agent_name}RenderComponentIntent = sdk.JsonMap;"),
        ]);
    }

    for custom_intent in custom_intents {
        lines.push(format!(
            "typedef {} = sdk.JsonMap;",
            custom_intent_type_name(agent_name, custom_intent)
        ));
    }

    lines.join("\n") + "\n"
}

fn generate_core_intent_models(agent_name: &str) -> String {
    format!(
        "final class {agent_name}ResponseTextIntent {{\n  const {agent_name}ResponseTextIntent({{\n    required this.text,\n  }});\n\n  final String text;\n\n  factory {agent_name}ResponseTextIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ResponseTextIntent(\n      text: (json['text'] as String?) ?? '',\n    );\n  }}\n}}\n\nfinal class {agent_name}ResponseSchemaIntent {{\n  const {agent_name}ResponseSchemaIntent({{\n    required this.type,\n    required this.response,\n  }});\n\n  final String type;\n  final Object? response;\n\n  factory {agent_name}ResponseSchemaIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ResponseSchemaIntent(\n      type: (json['type'] as String?) ?? '',\n      response: json['response'],\n    );\n  }}\n}}\n\nfinal class {agent_name}ErrorIntent {{\n  const {agent_name}ErrorIntent({{\n    required this.message,\n  }});\n\n  final String message;\n\n  factory {agent_name}ErrorIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ErrorIntent(\n      message: (json['message'] as String?) ?? '',\n    );\n  }}\n}}\n"
    )
}

fn generate_handler_classes(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut intent_methods = vec![
        format!(
            "  FutureOr<{agent_name}IntentControl> responseText({agent_name}ResponseTextIntent intent, String agentName) => null;"
        ),
        format!(
            "  FutureOr<{agent_name}IntentControl> responseSchema({agent_name}ResponseSchemaIntent intent, String agentName) => null;"
        ),
        format!(
            "  FutureOr<{agent_name}IntentControl> error({agent_name}ErrorIntent intent, String agentName) => null;"
        ),
    ];

    let mut partial_methods = vec![
        format!(
            "  FutureOr<void> responseText({agent_name}ResponseTextIntent intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> responseSchema({agent_name}ResponseSchemaIntent intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> error({agent_name}ErrorIntent intent, String agentName) {{}}"
        ),
    ];

    if has_tools {
        intent_methods.extend([
            format!(
                "  FutureOr<{agent_name}IntentControl> toolCall({agent_name}ToolCallIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> toolResult({agent_name}ToolResultIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> toolError({agent_name}ToolErrorIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> toolSkipped({agent_name}ToolSkippedIntent intent, String agentName) => null;"
            ),
        ]);

        partial_methods.extend([
            format!(
                "  FutureOr<void> toolCall({agent_name}ToolCallIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolResult({agent_name}ToolResultIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolError({agent_name}ToolErrorIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolSkipped({agent_name}ToolSkippedIntent intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_workflows {
        intent_methods.extend([
            format!(
                "  FutureOr<{agent_name}IntentControl> workflowCall({agent_name}WorkflowCallIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> workflowResult({agent_name}WorkflowResultIntent intent, String agentName) => null;"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> workflowCall({agent_name}WorkflowCallIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> workflowResult({agent_name}WorkflowResultIntent intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_helpers {
        intent_methods.extend([
            format!(
                "  FutureOr<{agent_name}IntentControl> helperCall({agent_name}HelperCallIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> helperResult({agent_name}HelperResultIntent intent, String agentName) => null;"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> helperCall({agent_name}HelperCallIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> helperResult({agent_name}HelperResultIntent intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_components {
        intent_methods.extend([
            format!(
                "  FutureOr<{agent_name}IntentControl> component({agent_name}ComponentIntent intent, String agentName) => null;"
            ),
            format!(
                "  FutureOr<{agent_name}IntentControl> renderComponent({agent_name}RenderComponentIntent intent, String agentName) => null;"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> component({agent_name}ComponentIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> renderComponent({agent_name}RenderComponentIntent intent, String agentName) {{}}"
            ),
        ]);
    }

    for custom_intent in custom_intents {
        let type_name = custom_intent_type_name(agent_name, custom_intent);
        let method_name = dart_method_name(custom_intent);
        intent_methods.push(format!(
            "  FutureOr<{agent_name}IntentControl> {method_name}({type_name} intent, String agentName) => null;"
        ));
        partial_methods.push(format!(
            "  FutureOr<void> {method_name}({type_name} intent, String agentName) {{}}"
        ));
    }

    format!(
        "abstract class {agent_name}BaseIntentHandler {{\n{}\n}}\n\nabstract class {agent_name}BasePartialIntentHandler {{\n{}\n}}\n",
        intent_methods.join("\n"),
        partial_methods.join("\n"),
    )
}

fn generate_api_keys(
    agent_name: &str,
    providers: &std::collections::BTreeSet<String>,
    custom_ids: &std::collections::BTreeSet<String>,
) -> String {
    let mut fields = Vec::new();
    let mut map_lines = Vec::new();

    if providers.contains("gemini") {
        fields.push("  final String? geminiApiKey;".to_string());
        map_lines.push(
            "      if (geminiApiKey != null && geminiApiKey!.isNotEmpty) 'geminiApiKey': geminiApiKey!,"
                .to_string(),
        );
    }
    if providers.contains("openai") {
        fields.push("  final String? openaiApiKey;".to_string());
        map_lines.push(
            "      if (openaiApiKey != null && openaiApiKey!.isNotEmpty) 'openaiApiKey': openaiApiKey!,"
                .to_string(),
        );
    }
    for custom_id in custom_ids {
        let field_name = format!("{}ApiKey", sanitize_identifier(custom_id, true));
        fields.push(format!("  final String? {field_name};"));
        map_lines.push(format!(
            "      if ({field_name} != null && {field_name}!.isNotEmpty) '{field_name}': {field_name}!,"
        ));
    }

    let constructor_fields = fields
        .iter()
        .map(|field| {
            let name = field
                .trim()
                .trim_start_matches("final ")
                .trim_end_matches(';')
                .trim_start_matches("String? ")
                .to_string();
            format!("    this.{name},")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "final class {agent_name}ApiKeys {{\n  const {agent_name}ApiKeys({{\n{constructor_fields}\n  }});\n\n{}\n\n  Map<String, String> toMap() {{\n    return {{\n{}\n    }};\n  }}\n}}\n",
        fields.join("\n"),
        map_lines.join("\n"),
    )
}

fn generate_config_class(agent_name: &str, has_api_keys: bool, has_context: bool) -> String {
    let api_keys_field = if has_api_keys {
        format!("  final {agent_name}ApiKeys? apiKeys;\n")
    } else {
        "  final Map<String, String> apiKeys;\n".to_string()
    };

    let api_keys_ctor = if has_api_keys {
        "    this.apiKeys,\n"
    } else {
        "    this.apiKeys = const {},\n"
    };

    let api_keys_expr = if has_api_keys {
        "      apiKeys: apiKeys?.toMap() ?? const {},"
    } else {
        "      apiKeys: apiKeys,"
    };

    let context_field = if has_context {
        format!("  final {agent_name}Context? context;\n")
    } else {
        "  final sdk.JsonMap? context;\n".to_string()
    };

    format!(
        "typedef {agent_name}Middleware = sdk.Middleware;\n\nfinal class {agent_name}Config {{\n  const {agent_name}Config({{\n    this.tools = const {{}},\n    this.middleware = const [],\n    this.context,\n{api_keys_ctor}    this.libraryPath,\n  }});\n\n  final {agent_name}Tools tools;\n  final List<{agent_name}Middleware> middleware;\n{context_field}{api_keys_field}  final String? libraryPath;\n\n  sdk.AuwgentConfig toAuwgentConfig() {{\n    return sdk.AuwgentConfig(\n      tools: tools,\n      middleware: middleware,\n      context: context,\n{api_keys_expr}\n      libraryPath: libraryPath,\n    );\n  }}\n}}\n"
    )
}

fn generate_agent_class(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    format!(
        "final class {agent_name}Agent extends sdk.TypedAuwgent<sdk.JsonMap> {{\n  {agent_name}Agent({agent_name}Config config)\n      : super(decode{}Ir(), config.toAuwgentConfig());\n\n  void onIntentHandler({agent_name}BaseIntentHandler handler) {{\n    onIntent((name, value, agentName) => _dispatchIntent(handler, name, value, agentName));\n  }}\n\n  void onIntentPartialHandler({agent_name}BasePartialIntentHandler handler) {{\n    onIntentPartial((name, value, agentName) {{\n      _dispatchPartialIntent(handler, name, value, agentName);\n    }});\n  }}\n}}\n\nObject? _dispatchIntent({agent_name}BaseIntentHandler handler, String name, Object? value, String agentName) {{\n  switch (name) {{\n{}\n    default:\n      return null;\n  }}\n}}\n\nvoid _dispatchPartialIntent({agent_name}BasePartialIntentHandler handler, String name, Object? value, String agentName) {{\n  switch (name) {{\n{}\n    default:\n      return;\n  }}\n}}\n",
        sanitize_identifier(agent_name, false),
        generate_dispatch_cases(
            agent_name,
            has_tools,
            has_workflows,
            has_helpers,
            has_components,
            custom_intents,
            false,
        ),
        generate_dispatch_cases(
            agent_name,
            has_tools,
            has_workflows,
            has_helpers,
            has_components,
            custom_intents,
            true,
        ),
    )
}

fn generate_dispatch_cases(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
    partial: bool,
) -> String {
    let mut lines = vec![
        dispatch_case(
            "response_text",
            "responseText",
            &format!("{agent_name}ResponseTextIntent.fromJson(value as sdk.JsonMap)"),
            partial,
        ),
        dispatch_case(
            "response_schema",
            "responseSchema",
            &format!("{agent_name}ResponseSchemaIntent.fromJson(value as sdk.JsonMap)"),
            partial,
        ),
        dispatch_case(
            "error",
            "error",
            &format!("{agent_name}ErrorIntent.fromJson(value as sdk.JsonMap)"),
            partial,
        ),
    ];

    if has_tools {
        lines.extend([
            dispatch_case("tool_call", "toolCall", "value as sdk.JsonMap", partial),
            dispatch_case("tool_result", "toolResult", "value as sdk.JsonMap", partial),
            dispatch_case("tool_error", "toolError", "value as sdk.JsonMap", partial),
            dispatch_case("tool_skipped", "toolSkipped", "value as sdk.JsonMap", partial),
        ]);
    }

    if has_workflows {
        lines.extend([
            dispatch_case(
                "workflow_call",
                "workflowCall",
                "value as sdk.JsonMap",
                partial,
            ),
            dispatch_case(
                "workflow_result",
                "workflowResult",
                "value as sdk.JsonMap",
                partial,
            ),
        ]);
    }

    if has_helpers {
        lines.extend([
            dispatch_case("helper_call", "helperCall", "value as sdk.JsonMap", partial),
            dispatch_case(
                "helper_result",
                "helperResult",
                "value as sdk.JsonMap",
                partial,
            ),
        ]);
    }

    if has_components {
        lines.extend([
            dispatch_case("component", "component", "value as sdk.JsonMap", partial),
            dispatch_case(
                "render_component",
                "renderComponent",
                "value as sdk.JsonMap",
                partial,
            ),
        ]);
    }

    for custom_intent in custom_intents {
        lines.push(dispatch_case(
            custom_intent,
            &dart_method_name(custom_intent),
            "value as sdk.JsonMap",
            partial,
        ));
    }

    lines.join("\n")
}

fn dispatch_case(intent_name: &str, method_name: &str, value_expr: &str, partial: bool) -> String {
    if partial {
        format!("    case '{intent_name}':\n      handler.{method_name}({value_expr}, agentName);\n      return;")
    } else {
        format!("    case '{intent_name}':\n      return handler.{method_name}({value_expr}, agentName);")
    }
}

fn generate_factory(
    agent_name: &str,
    has_api_keys: bool,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut aliases = vec![
        format!("typedef AuwgentAgent = {agent_name}Agent;"),
        format!("typedef AuwgentConfig = {agent_name}Config;"),
        format!("typedef AuwgentTools = {agent_name}Tools;"),
        format!("typedef AuwgentContext = {agent_name}Context;"),
        format!("typedef AuwgentMiddleware = {agent_name}Middleware;"),
        format!("typedef AuwgentIntentValue = {agent_name}IntentValue;"),
        format!("typedef AuwgentIntentControl = {agent_name}IntentControl;"),
        format!("typedef AuwgentIntentHandler = {agent_name}IntentHandler;"),
        format!("typedef AuwgentPartialIntentHandler = {agent_name}PartialIntentHandler;"),
        format!("typedef ResponseText = {agent_name}ResponseTextIntent;"),
        format!("typedef ResponseSchema = {agent_name}ResponseSchemaIntent;"),
        format!("typedef ErrorIntent = {agent_name}ErrorIntent;"),
        format!("typedef AuwgentBaseIntentHandler = {agent_name}BaseIntentHandler;"),
        format!(
            "typedef AuwgentBasePartialIntentHandler = {agent_name}BasePartialIntentHandler;"
        ),
    ];

    if has_api_keys {
        aliases.push(format!("typedef AuwgentApiKeys = {agent_name}ApiKeys;"));
    }
    if has_tools {
        aliases.extend([
            format!("typedef ToolCall = {agent_name}ToolCallIntent;"),
            format!("typedef ToolResult = {agent_name}ToolResultIntent;"),
            format!("typedef ToolError = {agent_name}ToolErrorIntent;"),
            format!("typedef ToolSkipped = {agent_name}ToolSkippedIntent;"),
        ]);
    }
    if has_workflows {
        aliases.extend([
            format!("typedef WorkflowCall = {agent_name}WorkflowCallIntent;"),
            format!("typedef WorkflowResult = {agent_name}WorkflowResultIntent;"),
        ]);
    }
    if has_helpers {
        aliases.extend([
            format!("typedef HelperCall = {agent_name}HelperCallIntent;"),
            format!("typedef HelperResult = {agent_name}HelperResultIntent;"),
        ]);
    }
    if has_components {
        aliases.extend([
            format!("typedef Component = {agent_name}ComponentIntent;"),
            format!("typedef RenderComponent = {agent_name}RenderComponentIntent;"),
        ]);
    }
    for custom_intent in custom_intents {
        aliases.push(format!(
            "typedef {}Intent = {};",
            pascal_case_identifier(custom_intent),
            custom_intent_type_name(agent_name, custom_intent)
        ));
    }

    format!(
        "{agent_name}Agent create{agent_name}({agent_name}Config config) {{\n  return {agent_name}Agent(config);\n}}\n\nfinal auwgent = create{agent_name};\n\n{}\n",
        aliases.join("\n")
    )
}

fn type_to_dart_string(value: Option<&Value>, unwrap_input_kind: bool, null_fallback: &str) -> String {
    let Some(value) = value else {
        return null_fallback.to_string();
    };

    if value.is_null() {
        return null_fallback.to_string();
    }

    if unwrap_input_kind {
        if let Some(obj) = value.as_object() {
            match obj.get("kind").and_then(Value::as_str) {
                Some("direct") => return "String".to_string(),
                Some("properties") => return "sdk.JsonMap".to_string(),
                _ => {}
            }
        }
    }

    if value.as_object().is_some() {
        return object_or_type_to_dart_string(value, null_fallback);
    }

    if let Some(raw) = value.as_str() {
        return normalize_dart_type(raw);
    }

    null_fallback.to_string()
}

fn object_or_type_to_dart_string(value: &Value, null_fallback: &str) -> String {
    if let Some(kind) = string_at(value, &["type"]) {
        match kind {
            "typeRef" => {
                return string_at(value, &["name"])
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "sdk.JsonMap".to_string())
            }
            "array" => {
                let inner = type_to_dart_string(value.get("items"), false, "Object?");
                return format!("List<{inner}>");
            }
            "union" => {
                if let Some(options) = value.get("options").and_then(Value::as_array) {
                    if options.iter().all(Value::is_string) {
                        return "String".to_string();
                    }
                }
                return "Object?".to_string();
            }
            "object" => return "sdk.JsonMap".to_string(),
            other => return normalize_dart_type(other),
        }
    }

    if object_at(value, &["properties"]).is_some() || object_at(value, &["fields"]).is_some() {
        return "sdk.JsonMap".to_string();
    }

    null_fallback.to_string()
}

fn normalize_dart_type(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "int" => "int".to_string(),
        "float" | "number" => "double".to_string(),
        "bool" | "boolean" => "bool".to_string(),
        "text" | "string" => "String".to_string(),
        other => other.to_string(),
    }
}

fn custom_intent_type_name(agent_name: &str, intent_name: &str) -> String {
    format!("{agent_name}{}Intent", pascal_case_identifier(intent_name))
}

fn dart_method_name(name: &str) -> String {
    let mut out = String::new();
    let mut capitalize = false;
    for (index, ch) in name.chars().enumerate() {
        if ch == '_' || ch == '-' || ch == ' ' {
            capitalize = true;
            continue;
        }

        if index == 0 {
            out.push(ch.to_ascii_lowercase());
            continue;
        }

        if capitalize {
            out.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }

    if out.is_empty() {
        "intent".to_string()
    } else {
        out
    }
}

fn sanitize_identifier(name: &str, lower_first: bool) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        return "agent".to_string();
    }

    let starts_with_digit = out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
    if starts_with_digit {
        out.insert(0, '_');
    }

    if lower_first {
        let mut chars = out.chars();
        if let Some(first) = chars.next() {
            return first.to_ascii_lowercase().to_string() + chars.as_str();
        }
    }

    out
}

fn pascal_case_identifier(name: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;

    for ch in name.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            if capitalize {
                out.push(ch.to_ascii_uppercase());
                capitalize = false;
            } else if ch == '_' {
                capitalize = true;
            } else {
                out.push(ch);
            }
        } else {
            capitalize = true;
        }
    }

    if out.is_empty() {
        "Intent".to_string()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{generate, generate_ir_module};
    use serde_json::json;

    #[test]
    fn emits_dart_factory_and_embedded_ir() {
        let ir = json!({
            "name": "Demo",
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [],
            "modelConfig": []
        });

        let output = generate(&ir, "demo");
        assert!(output.contains("import 'dart:async';"));
        assert!(output.contains("import 'demo.agent.ir.dart';"));
        assert!(output.contains("import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;"));
        assert!(output.contains("final class DemoResponseTextIntent"));
        assert!(output.contains("final class DemoAgent extends sdk.TypedAuwgent<sdk.JsonMap>"));
        assert!(output.contains("DemoAgent createDemo(DemoConfig config)"));
    }

    #[test]
    fn emits_dart_ir_module() {
        let ir = json!({
            "name": "Demo",
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [],
            "modelConfig": []
        });

        let output = generate_ir_module(&ir);
        assert!(output.contains("const String _DemoAgentIrJson"));
        assert!(output.contains("sdk.JsonMap decodeDemoIr()"));
    }

    #[test]
    fn emits_dart_api_keys() {
        let ir = json!({
            "name": "Billing",
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [],
            "modelConfig": [{
                "defaultConfig": { "model": { "type": "custom", "id": "my-groq", "url": "https://example.com" } },
                "namedConfig": []
            }]
        });

        let output = generate(&ir, "billing");
        assert!(output.contains("final class BillingApiKeys"));
        assert!(output.contains("final String? my_groqApiKey;"));
    }

    #[test]
    fn emits_dart_handler_classes() {
        let ir = json!({
            "name": "UiAgent",
            "input": null,
            "output": null,
            "context": null,
            "tools": [{
                "name": "delete",
                "params": {},
                "returns": null
            }],
            "workflows": [],
            "helpers": [],
            "components": [{
                "name": "Button",
                "props": {},
                "action": null,
                "children": null
            }],
            "modelConfig": [],
            "customIntents": [{
                "name": "ask_user",
                "fields": {}
            }]
        });

        let output = generate(&ir, "ui");
        assert!(output.contains("abstract class UiAgentBaseIntentHandler"));
        assert!(output.contains("toolCall(UiAgentToolCallIntent intent, String agentName)"));
        assert!(output.contains("component(UiAgentComponentIntent intent, String agentName)"));
        assert!(output.contains("askUser(UiAgentAskUserIntent intent, String agentName)"));
        assert!(output.contains("void onIntentHandler(UiAgentBaseIntentHandler handler)"));
        assert!(output.contains("case 'tool_call':"));
        assert!(output.contains("case 'component':"));
        assert!(output.contains("case 'ask_user':"));
        assert!(output.contains("typedef AuwgentConfig = UiAgentConfig;"));
        assert!(output.contains("typedef ToolCall = UiAgentToolCallIntent;"));
        assert!(output.contains("typedef Component = UiAgentComponentIntent;"));
        assert!(output.contains("typedef AskUserIntent = UiAgentAskUserIntent;"));
        assert!(output.contains("typedef AuwgentMiddleware = UiAgentMiddleware;"));
        assert!(output.contains("UiAgentResponseTextIntent.fromJson(value as sdk.JsonMap)"));
    }
}
