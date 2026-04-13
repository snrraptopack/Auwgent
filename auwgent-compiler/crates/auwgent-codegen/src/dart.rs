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
    let tools = array_at(ir, &["tools"]);
    let workflows = array_at(ir, &["workflows"]);
    let helpers = array_at(ir, &["helpers"]);
    let custom_intents = collect_custom_intents(ir);
    let custom_intent_defs = collect_custom_intent_defs(ir);
    let has_tools = !tools.is_empty();
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

    sections.push(generate_named_shape(
        &format!("{agent_name}Input"),
        ir.get("input"),
        true,
        "String",
    ));

    for helper in &output_helpers {
        if let Some(name) = string_at(helper, &["name"]) {
            sections.push(generate_named_shape(
                &format!("{name}Output"),
                helper.get("output"),
                false,
                "sdk.JsonMap",
            ));
        }
    }

    sections.push(generate_named_shape(
        &format!("{agent_name}Output"),
        ir.get("output"),
        false,
        "sdk.JsonMap",
    ));
    sections.push(generate_named_shape(
        &format!("{agent_name}Context"),
        ir.get("context"),
        false,
        "sdk.JsonMap",
    ));
    sections.push(generate_tools_registry(agent_name, tools));
    sections.push(generate_core_intent_models(agent_name, ir.get("output")));
    sections.push(generate_action_intent_models(
        agent_name,
        tools,
        workflows,
        helpers,
        array_at(ir, &["components"]),
        &custom_intent_defs,
    ));

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
        has_tools,
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

fn collect_custom_intent_defs(ir: &Value) -> Vec<(String, Value)> {
    let mut defs = Vec::new();

    if let Some(items) = ir.get("customIntents").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = string_at(item, &["name"]) {
                if !defs.iter().any(|(existing, _)| existing == name) {
                    defs.push((name.to_string(), item.clone()));
                }
            }
        }
    }

    for helper in array_at(ir, &["helpers"]) {
        if let Some(items) = helper.get("customIntents").and_then(Value::as_array) {
            for item in items {
                if let Some(name) = string_at(item, &["name"]) {
                    if !defs.iter().any(|(existing, _)| existing == name) {
                        defs.push((name.to_string(), item.clone()));
                    }
                }
            }
        }
    }

    defs
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

fn generate_named_shape(
    name: &str,
    value: Option<&Value>,
    unwrap_input_kind: bool,
    null_fallback: &str,
) -> String {
    if let Some(variants) = shape_variants(value) {
        return generate_variant_class(name, &variants);
    }

    if let Some(properties) = shape_properties(value, unwrap_input_kind) {
        return generate_data_class(name, &properties);
    }

    generate_type_alias(name, value, unwrap_input_kind, null_fallback)
}

fn shape_properties(
    value: Option<&Value>,
    unwrap_input_kind: bool,
) -> Option<Map<String, Value>> {
    let value = value?;
    let obj = value.as_object()?;

    if unwrap_input_kind {
        if obj.get("kind").and_then(Value::as_str) == Some("properties") {
            return obj
                .get("fields")
                .and_then(Value::as_object)
                .cloned();
        }
    }

    if obj.get("type").and_then(Value::as_str) == Some("object") {
        return obj
            .get("properties")
            .and_then(Value::as_object)
            .cloned();
    }

    if obj.contains_key("properties") {
        return obj.get("properties").and_then(Value::as_object).cloned();
    }

    if obj.contains_key("fields") {
        return obj.get("fields").and_then(Value::as_object).cloned();
    }

    if obj.contains_key("__variants") {
        return None;
    }

    if !obj.contains_key("type") && !obj.contains_key("kind") {
        return Some(obj.clone());
    }

    None
}

fn shape_variants(value: Option<&Value>) -> Option<Map<String, Value>> {
    value?
        .as_object()?
        .get("__variants")
        .and_then(Value::as_object)
        .cloned()
}

fn generate_data_class(name: &str, properties: &Map<String, Value>) -> String {
    let mut ctor_lines = Vec::new();
    let mut field_lines = Vec::new();
    let mut from_json_lines = Vec::new();
    let mut to_json_lines = Vec::new();

    for (prop_name, prop_info) in properties {
        if prop_name.starts_with('@') || prop_name.starts_with("__") {
            continue;
        }

        let optional = prop_info
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let field_type = field_type_string(prop_info, optional);
        let required_kw = if optional { "" } else { "required " };

        ctor_lines.push(format!("    {required_kw}this.{prop_name},"));
        field_lines.push(format!("  final {field_type} {prop_name};"));
        from_json_lines.push(format!(
            "      {prop_name}: {},",
            decode_field_expr(prop_name, prop_info, optional)
        ));
        to_json_lines.push(format!("      '{prop_name}': {prop_name},"));
    }

    if field_lines.is_empty() {
        return format!(
            "final class {name} {{\n  const {name}();\n\n  factory {name}.fromJson(sdk.JsonMap json) {{\n    return const {name}();\n  }}\n\n  sdk.JsonMap toJson() => const {{}};\n\n  @override\n  String toString() => sdk.prettyJson(toJson());\n}}\n"
        );
    }

    format!(
        "final class {name} {{\n  const {name}({{\n{}\n  }});\n\n{}\n\n  factory {name}.fromJson(sdk.JsonMap json) {{\n    return {name}(\n{}\n    );\n  }}\n\n  sdk.JsonMap toJson() {{\n    return {{\n{}\n    }};\n  }}\n\n  @override\n  String toString() => sdk.prettyJson(toJson());\n}}\n",
        ctor_lines.join("\n"),
        field_lines.join("\n"),
        from_json_lines.join("\n"),
        to_json_lines.join("\n"),
    )
}

fn generate_variant_class(name: &str, variants: &Map<String, Value>) -> String {
    let mut blocks = vec![format!(
        "abstract class {name} {{\n  const {name}();\n\n  String get variant;\n\n  factory {name}.fromJson(sdk.JsonMap json) {{\n{}\n    return {name}Unknown(Map<String, Object?>.from(json));\n  }}\n}}\n",
        variants
            .iter()
            .map(|(variant_name, _variant_props)| {
                format!(
                    "    if (_matches{name}{variant_name}(json)) {{\n      return {name}{variant_name}.fromJson(json);\n    }}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    )];

    for (variant_name, variant_props) in variants {
        let properties = variant_props.as_object().cloned().unwrap_or_default();
        blocks.push(generate_variant_data_class(name, variant_name, &properties));
        blocks.push(generate_variant_matcher(name, variant_name, &properties));
    }

    blocks.push(format!(
        "final class {name}Unknown extends {name} {{\n  const {name}Unknown(this.raw);\n\n  final sdk.JsonMap raw;\n\n  @override\n  String get variant => 'Unknown';\n}}\n"
    ));

    blocks.join("\n")
}

fn generate_variant_data_class(
    base_name: &str,
    variant_name: &str,
    properties: &Map<String, Value>,
) -> String {
    let class_name = format!("{base_name}{variant_name}");
    let mut ctor_lines = Vec::new();
    let mut field_lines = Vec::new();
    let mut from_json_lines = Vec::new();
    let mut to_json_lines = Vec::new();

    for (prop_name, prop_info) in properties {
        if prop_name.starts_with('@') || prop_name.starts_with("__") {
            continue;
        }

        let optional = prop_info
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let field_type = field_type_string(prop_info, optional);
        let required_kw = if optional { "" } else { "required " };

        ctor_lines.push(format!("    {required_kw}this.{prop_name},"));
        field_lines.push(format!("  final {field_type} {prop_name};"));
        from_json_lines.push(format!(
            "      {prop_name}: {},",
            decode_field_expr(prop_name, prop_info, optional)
        ));
        to_json_lines.push(format!("      '{prop_name}': {prop_name},"));
    }

    format!(
        "final class {class_name} extends {base_name} {{\n  const {class_name}({{\n{}\n  }});\n\n{}\n\n  @override\n  String get variant => '{variant_name}';\n\n  factory {class_name}.fromJson(sdk.JsonMap json) {{\n    return {class_name}(\n{}\n    );\n  }}\n\n  sdk.JsonMap toJson() {{\n    return {{\n      'type': '{variant_name}',\n{}\n    }};\n  }}\n\n  @override\n  String toString() => sdk.prettyJson(toJson());\n}}\n",
        ctor_lines.join("\n"),
        field_lines.join("\n"),
        from_json_lines.join("\n"),
        to_json_lines.join("\n"),
    )
}

fn generate_variant_matcher(
    base_name: &str,
    variant_name: &str,
    properties: &Map<String, Value>,
) -> String {
    let checks = properties
        .iter()
        .filter(|(name, _)| !name.starts_with('@') && !name.starts_with("__"))
        .map(|(name, info)| {
            if info.get("optional").and_then(Value::as_bool).unwrap_or(false) {
                "true".to_string()
            } else {
                format!("json.containsKey('{name}')")
            }
        })
        .collect::<Vec<_>>();

    let body = if checks.is_empty() {
        "true".to_string()
    } else {
        checks.join(" && ")
    };

    format!(
        "bool _matches{base_name}{variant_name}(sdk.JsonMap json) {{\n  return {body};\n}}\n"
    )
}

fn field_type_string(prop_info: &Value, optional: bool) -> String {
    let base = type_to_dart_string(Some(prop_info), false, "Object?");
    if optional && !base.ends_with('?') {
        format!("{base}?")
    } else {
        base
    }
}

fn decode_field_expr(prop_name: &str, prop_info: &Value, optional: bool) -> String {
    let access = format!("json['{prop_name}']");

    match type_to_dart_string(Some(prop_info), false, "Object?").as_str() {
        "String" => {
            if optional {
                format!("{access} as String?")
            } else {
                format!("({access} as String?) ?? ''")
            }
        }
        "int" => {
            if optional {
                format!("({access} as num?)?.toInt()")
            } else {
                format!("(({access} as num?)?.toInt()) ?? 0")
            }
        }
        "double" => {
            if optional {
                format!("({access} as num?)?.toDouble()")
            } else {
                format!("(({access} as num?)?.toDouble()) ?? 0")
            }
        }
        "bool" => {
            if optional {
                format!("{access} as bool?")
            } else {
                format!("({access} as bool?) ?? false")
            }
        }
        t if t.starts_with("List<") => {
            if optional {
                format!(
                    "({access} as List?)?.map((item) => item as Object?).toList(growable: false)"
                )
            } else {
                format!(
                    "(({access} as List?) ?? const []).map((item) => item as Object?).toList(growable: false)"
                )
            }
        }
        "sdk.JsonMap" => {
            if optional {
                format!(
                    "{access} == null ? null : Map<String, Object?>.from({access} as Map)"
                )
            } else {
                format!("Map<String, Object?>.from(({access} as Map?) ?? const {{}})")
            }
        }
        other => {
            if optional {
                format!("{access} as {other}?")
            } else {
                format!("{access} as {other}")
            }
        }
    }
}

fn generate_intent_aliases(
    agent_name: &str,
    _has_tools: bool,
    _has_workflows: bool,
    _has_helpers: bool,
    _has_components: bool,
    _custom_intents: &[String],
) -> String {
    let lines = vec![
        format!("typedef {agent_name}IntentValue = Object?;"),
        format!("typedef {agent_name}IntentControl = sdk.IntentControl?;"),
        format!(
            "typedef {agent_name}IntentHandler = FutureOr<sdk.IntentControl?> Function(String name, Object? value, String agentName);"
        ),
        format!(
            "typedef {agent_name}PartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);"
        ),
    ];

    lines.join("\n") + "\n"
}

fn generate_core_intent_models(agent_name: &str, output: Option<&Value>) -> String {
    let response_type = format!("{agent_name}Output");
    let response_expr = decode_named_shape_expr(output, &response_type, "json['response']", "json['response']");

    format!(
        "final class {agent_name}ResponseTextIntent {{\n  const {agent_name}ResponseTextIntent({{\n    required this.text,\n  }});\n\n  final String text;\n\n  factory {agent_name}ResponseTextIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ResponseTextIntent(\n      text: (json['text'] as String?) ?? '',\n    );\n  }}\n\n  @override\n  String toString() => '{agent_name}ResponseTextIntent(text: $text)';\n}}\n\nfinal class {agent_name}ResponseSchemaIntent {{\n  const {agent_name}ResponseSchemaIntent({{\n    required this.type,\n    required this.response,\n  }});\n\n  final String type;\n  final {response_type} response;\n\n  factory {agent_name}ResponseSchemaIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ResponseSchemaIntent(\n      type: (json['type'] as String?) ?? '',\n      response: {response_expr},\n    );\n  }}\n\n  @override\n  String toString() => '{agent_name}ResponseSchemaIntent(type: $type, response: $response)';\n}}\n\nfinal class {agent_name}ErrorIntent {{\n  const {agent_name}ErrorIntent({{\n    required this.message,\n  }});\n\n  final String message;\n\n  factory {agent_name}ErrorIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}ErrorIntent(\n      message: (json['message'] as String?) ?? '',\n    );\n  }}\n\n  @override\n  String toString() => '{agent_name}ErrorIntent(message: $message)';\n}}\n"
    )
}

fn generate_action_intent_models(
    agent_name: &str,
    tools: &[Value],
    workflows: &[Value],
    helpers: &[Value],
    components: &[Value],
    custom_intents: &[(String, Value)],
) -> String {
    let mut blocks = Vec::new();

    if !tools.is_empty() {
        blocks.push(generate_tool_intent_models(agent_name, tools));
    }
    if !workflows.is_empty() {
        blocks.push(generate_workflow_intent_models(agent_name, workflows));
    }
    if !helpers.is_empty() {
        blocks.push(generate_helper_intent_models(agent_name, helpers));
    }
    if !components.is_empty() {
        blocks.push(generate_component_intent_models(agent_name, components));
    }
    for (name, def) in custom_intents {
        blocks.push(generate_named_shape(
            &custom_intent_type_name(agent_name, name),
            Some(def),
            false,
            "sdk.JsonMap",
        ));
    }

    blocks.join("\n")
}

fn generate_tool_intent_models(agent_name: &str, tools: &[Value]) -> String {
    generate_callable_intent_models(
        agent_name,
        "Tool",
        "tool",
        tools,
        "name",
        "params",
        "returns",
        true,
    )
}

fn generate_workflow_intent_models(agent_name: &str, workflows: &[Value]) -> String {
    generate_callable_intent_models(
        agent_name,
        "Workflow",
        "workflow",
        workflows,
        "flowName",
        "flowParams",
        "returns",
        false,
    )
}

fn generate_helper_intent_models(agent_name: &str, helpers: &[Value]) -> String {
    generate_callable_intent_models(
        agent_name,
        "Helper",
        "helper",
        helpers,
        "name",
        "input",
        "output",
        false,
    )
}

fn generate_tools_registry(agent_name: &str, tools: &[Value]) -> String {
    if tools.is_empty() {
        return format!(
            "abstract class {agent_name}Tools {{\n  const {agent_name}Tools();\n\n  Map<String, sdk.ToolHandler> toMap() => const {{}};\n}}\n\nfinal class {agent_name}ToolRegistry extends {agent_name}Tools {{\n  const {agent_name}ToolRegistry();\n}}\n"
        );
    }

    let mut handler_typedefs = Vec::new();
    let mut abstract_methods = Vec::new();
    let mut ctor_fields = Vec::new();
    let mut ctor_init_lines = Vec::new();
    let mut registry_fields = Vec::new();
    let mut registry_methods = Vec::new();
    let mut map_lines = Vec::new();

    for tool in tools {
        let Some(tool_name) = string_at(tool, &["name"]) else {
            continue;
        };
        let pascal = pascal_case_identifier(tool_name);
        let method_name = dart_method_name(tool_name);
        let args_type = if is_empty_shape(tool.get("params"), false) {
            None
        } else {
            Some(format!("{agent_name}{pascal}ToolArgs"))
        };
        let result_type = if tool.get("returns").is_none() || tool.get("returns").is_some_and(Value::is_null) {
            "sdk.NoResult".to_string()
        } else {
            format!("{agent_name}{pascal}ToolResultValue")
        };
        let handler_type = format!("{agent_name}{pascal}ToolHandler");

        let method_signature = if let Some(args_type) = &args_type {
            format!("FutureOr<{result_type}> {method_name}({args_type} args)")
        } else {
            format!("FutureOr<{result_type}> {method_name}()")
        };

        let handler_signature = if let Some(args_type) = &args_type {
            format!("FutureOr<{result_type}> Function({args_type} args)")
        } else {
            format!("FutureOr<{result_type}> Function()")
        };

        handler_typedefs.push(format!("typedef {handler_type} = {handler_signature};"));
        abstract_methods.push(format!("  {method_signature};"));
        ctor_fields.push(format!("    required {handler_type} {method_name},"));
        ctor_init_lines.push(format!("      _{method_name} = {method_name}"));
        registry_fields.push(format!("  final {handler_type} _{method_name};"));

        if let Some(args_type) = &args_type {
            registry_methods.push(format!(
                "  @override\n  FutureOr<{result_type}> {method_name}({args_type} args) => _{method_name}(args);"
            ));
        } else {
            registry_methods.push(format!(
                "  @override\n  FutureOr<{result_type}> {method_name}() => _{method_name}();"
            ));
        }

        if let Some(args_type) = &args_type {
            map_lines.push(format!(
                "      '{tool_name}': (args) => {method_name}({args_type}.fromJson(Map<String, Object?>.from((args as Map?) ?? const {{}}))),"
            ));
        } else {
            map_lines.push(format!(
                "      '{tool_name}': (_) => {method_name}(),"
            ));
        }
    }

    format!(
        "{}\n\nabstract class {agent_name}Tools {{\n  const {agent_name}Tools();\n\n{}\n\n  Map<String, sdk.ToolHandler> toMap() {{\n    return {{\n{}\n    }};\n  }}\n}}\n\nfinal class {agent_name}ToolRegistry extends {agent_name}Tools {{\n  const {agent_name}ToolRegistry({{\n{}\n  }}) :\n{};\n\n{}\n\n{}\n}}\n",
        handler_typedefs.join("\n"),
        abstract_methods.join("\n"),
        map_lines.join("\n"),
        ctor_fields.join("\n"),
        ctor_init_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let suffix = if index + 1 == ctor_init_lines.len() { "" } else { "," };
                format!("{line}{suffix}")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        registry_fields.join("\n"),
        registry_methods.join("\n"),
    )
}

fn generate_component_intent_models(agent_name: &str, components: &[Value]) -> String {
    let mut blocks = Vec::new();
    let mut component_cases = Vec::new();

    for component in components {
        let Some(component_name) = string_at(component, &["name"]) else {
            continue;
        };
        let pascal = pascal_case_identifier(component_name);
        let props_type = format!("{agent_name}{pascal}ComponentProps");
        let action_type = format!("{agent_name}{pascal}ComponentAction");
        let component_class = format!("{agent_name}{pascal}ComponentIntentCase");

        blocks.push(generate_named_shape(
            &props_type,
            component.get("props"),
            false,
            "sdk.JsonMap",
        ));

        let _ = component.get("action");
        blocks.push(format!("typedef {action_type} = sdk.JsonMap;\n"));

        blocks.push(format!(
            "final class {component_class} extends {agent_name}ComponentIntent {{\n  const {component_class}({{\n    required this.cId,\n    required this.props,\n    this.action,\n    this.children,\n  }});\n\n  @override\n  final String cId;\n  @override\n  final {props_type} props;\n  @override\n  final {action_type}? action;\n  @override\n  final List<String>? children;\n\n  @override\n  String get type => '{component_name}';\n\n  factory {component_class}.fromJson(sdk.JsonMap json) {{\n    return {component_class}(\n      cId: (json['c_id'] as String?) ?? '',\n      props: {},\n      action: {},\n      children: (json['children'] as List?)?.map((item) => item.toString()).toList(growable: false),\n    );\n  }}\n}}\n",
            decode_named_shape_expr(component.get("props"), &props_type, "json['props']", "const <String, Object?>{}"),
            decode_named_shape_expr_optional(component.get("action"), &action_type, "json['action']")
        ));

        component_cases.push(format!(
            "    if (kind == '{component_name}') {{\n      return {component_class}.fromJson(json);\n    }}"
        ));
    }

    blocks.insert(
        0,
        format!(
            "abstract class {agent_name}ComponentIntent {{\n  const {agent_name}ComponentIntent();\n\n  String get type;\n  String get cId;\n  Object? get props;\n  Object? get action;\n  List<String>? get children;\n\n  factory {agent_name}ComponentIntent.fromJson(sdk.JsonMap json) {{\n    final kind = (json['type'] as String?) ?? '';\n{}\n    return {agent_name}ComponentIntentUnknown(Map<String, Object?>.from(json));\n  }}\n}}\n\nfinal class {agent_name}ComponentIntentUnknown extends {agent_name}ComponentIntent {{\n  const {agent_name}ComponentIntentUnknown(this.raw);\n\n  final sdk.JsonMap raw;\n\n  @override\n  String get type => (raw['type'] as String?) ?? '';\n\n  @override\n  String get cId => (raw['c_id'] as String?) ?? '';\n\n  @override\n  Object? get props => raw['props'];\n\n  @override\n  Object? get action => raw['action'];\n\n  @override\n  List<String>? get children => (raw['children'] as List?)?.map((item) => item.toString()).toList(growable: false);\n}}\n\nfinal class {agent_name}RenderComponentIntent {{\n  const {agent_name}RenderComponentIntent({{\n    this.root,\n    this.roots,\n    this.components,\n    this.tree,\n    this.trees,\n  }});\n\n  final String? root;\n  final List<String>? roots;\n  final sdk.JsonMap? components;\n  final Object? tree;\n  final List<Object?>? trees;\n\n  factory {agent_name}RenderComponentIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}RenderComponentIntent(\n      root: json['root'] as String?,\n      roots: (json['roots'] as List?)?.map((item) => item.toString()).toList(growable: false),\n      components: json['components'] == null ? null : Map<String, Object?>.from(json['components'] as Map),\n      tree: json['tree'],\n      trees: (json['trees'] as List?)?.map((item) => item as Object?).toList(growable: false),\n    );\n  }}\n}}\n",
            component_cases.join("\n"),
        ),
    );

    blocks.join("\n")
}

fn generate_callable_intent_models(
    agent_name: &str,
    family_name: &str,
    family_key: &str,
    items: &[Value],
    name_key: &str,
    args_key: &str,
    result_key: &str,
    include_error_and_skipped: bool,
) -> String {
    let mut blocks = Vec::new();
    let mut call_cases = Vec::new();
    let mut result_cases = Vec::new();
    let mut skipped_cases = Vec::new();

    for item in items {
        let Some(item_name) = string_at(item, &[name_key]) else {
            continue;
        };
        let pascal = pascal_case_identifier(item_name);
        let args_type = format!("{agent_name}{pascal}{family_name}Args");
        let result_type = format!("{agent_name}{pascal}{family_name}ResultValue");
        let args_shape = item.get(args_key);
        let result_shape = item.get(result_key);
        let args_type_ref = if is_empty_shape(args_shape, false) {
            "sdk.NoArgs".to_string()
        } else {
            blocks.push(generate_named_shape(
                &args_type,
                args_shape,
                false,
                "sdk.JsonMap",
            ));
            args_type.clone()
        };
        let result_type_ref = if result_shape.is_none() || result_shape.is_some_and(Value::is_null) {
            "sdk.NoResult".to_string()
        } else {
            blocks.push(generate_named_shape(
                &result_type,
                result_shape,
                false,
                "Object?",
            ));
            result_type.clone()
        };
        let args_decode_expr = if args_type_ref == "sdk.NoArgs" {
            "const sdk.NoArgs()".to_string()
        } else {
            decode_named_shape_expr(args_shape, &args_type_ref, "json['args']", "json['args']")
        };
        let result_decode_expr = if result_type_ref == "sdk.NoResult" {
            "const sdk.NoResult()".to_string()
        } else {
            decode_named_shape_expr(result_shape, &result_type_ref, "json['result']", "json['result']")
        };

        let call_class = format!("{agent_name}{pascal}{family_name}CallIntentCase");
        let result_class = format!("{agent_name}{pascal}{family_name}ResultIntentCase");

        if args_type_ref == "sdk.NoArgs" {
            blocks.push(format!(
                "final class {call_class} extends {agent_name}{family_name}CallIntent {{\n  const {call_class}();\n\n  @override\n  sdk.NoArgs get args => const sdk.NoArgs();\n\n  @override\n  String get type => '{item_name}';\n\n  factory {call_class}.fromJson(sdk.JsonMap json) {{\n    return const {call_class}();\n  }}\n\n  @override\n  String toString() => '{call_class}(type: {item_name}, args: $args)';\n}}\n",
            ));
        } else {
            blocks.push(format!(
                "final class {call_class} extends {agent_name}{family_name}CallIntent {{\n  const {call_class}({{\n    required this.args,\n  }});\n\n  @override\n  final {args_type_ref} args;\n\n  @override\n  String get type => '{item_name}';\n\n  factory {call_class}.fromJson(sdk.JsonMap json) {{\n    return {call_class}(\n      args: {args_decode_expr},\n    );\n  }}\n\n  @override\n  String toString() => '{call_class}(type: {item_name}, args: $args)';\n}}\n",
            ));
        }

        if args_type_ref == "sdk.NoArgs" {
            blocks.push(format!(
                "final class {result_class} extends {agent_name}{family_name}ResultIntent {{\n  const {result_class}({{\n    required this.result,\n    this.overridden = false,\n  }});\n\n  @override\n  sdk.NoArgs get args => const sdk.NoArgs();\n  @override\n  final {result_type_ref} result;\n  @override\n  final bool overridden;\n\n  @override\n  String get name => '{item_name}';\n\n  factory {result_class}.fromJson(sdk.JsonMap json) {{\n    return {result_class}(\n      result: {result_decode_expr},\n      overridden: (json['overridden'] as bool?) ?? false,\n    );\n  }}\n\n  @override\n  String toString() => '{result_class}(name: {item_name}, result: $result, overridden: $overridden)';\n}}\n",
            ));
        } else {
            blocks.push(format!(
                "final class {result_class} extends {agent_name}{family_name}ResultIntent {{\n  const {result_class}({{\n    required this.args,\n    required this.result,\n    this.overridden = false,\n  }});\n\n  @override\n  final {args_type_ref} args;\n  @override\n  final {result_type_ref} result;\n  @override\n  final bool overridden;\n\n  @override\n  String get name => '{item_name}';\n\n  factory {result_class}.fromJson(sdk.JsonMap json) {{\n    return {result_class}(\n      args: {args_decode_expr},\n      result: {result_decode_expr},\n      overridden: (json['overridden'] as bool?) ?? false,\n    );\n  }}\n\n  @override\n  String toString() => '{result_class}(name: {item_name}, args: $args, result: $result, overridden: $overridden)';\n}}\n",
            ));
        }

        call_cases.push(format!(
            "    if (kind == '{item_name}') {{\n      return {call_class}.fromJson(json);\n    }}"
        ));
        result_cases.push(format!(
            "    if (kind == '{item_name}') {{\n      return {result_class}.fromJson(json);\n    }}"
        ));

        if include_error_and_skipped {
            let skipped_class = format!("{agent_name}{pascal}{family_name}SkippedIntentCase");
            if args_type_ref == "sdk.NoArgs" {
                blocks.push(format!(
                    "final class {skipped_class} extends {agent_name}{family_name}SkippedIntent {{\n  const {skipped_class}();\n\n  @override\n  sdk.NoArgs get args => const sdk.NoArgs();\n\n  @override\n  String get type => '{item_name}';\n\n  factory {skipped_class}.fromJson(sdk.JsonMap json) {{\n    return const {skipped_class}();\n  }}\n\n  @override\n  String toString() => '{skipped_class}(type: {item_name}, args: $args)';\n}}\n",
                ));
            } else {
                blocks.push(format!(
                    "final class {skipped_class} extends {agent_name}{family_name}SkippedIntent {{\n  const {skipped_class}({{\n    required this.args,\n  }});\n\n  @override\n  final {args_type_ref} args;\n\n  @override\n  String get type => '{item_name}';\n\n  factory {skipped_class}.fromJson(sdk.JsonMap json) {{\n    return {skipped_class}(\n      args: {args_decode_expr},\n    );\n  }}\n\n  @override\n  String toString() => '{skipped_class}(type: {item_name}, args: $args)';\n}}\n",
                ));
            }
            skipped_cases.push(format!(
                "    if (kind == '{item_name}') {{\n      return {skipped_class}.fromJson(json);\n    }}"
            ));
        }
    }

    blocks.insert(
        0,
        format!(
            "abstract class {agent_name}{family_name}CallIntent {{\n  const {agent_name}{family_name}CallIntent();\n\n  String get type;\n  Object? get args;\n\n  factory {agent_name}{family_name}CallIntent.fromJson(sdk.JsonMap json) {{\n    final kind = (json['type'] as String?) ?? '';\n{}\n    return {agent_name}{family_name}CallIntentUnknown(Map<String, Object?>.from(json));\n  }}\n}}\n\nabstract class {agent_name}{family_name}ResultIntent {{\n  const {agent_name}{family_name}ResultIntent();\n\n  String get name;\n  Object? get args;\n  Object? get result;\n  bool get overridden;\n\n  factory {agent_name}{family_name}ResultIntent.fromJson(sdk.JsonMap json) {{\n    final kind = (json['name'] as String?) ?? '';\n{}\n    return {agent_name}{family_name}ResultIntentUnknown(Map<String, Object?>.from(json));\n  }}\n}}\n",
            call_cases.join("\n"),
            result_cases.join("\n"),
        ),
    );

    blocks.push(format!(
        "final class {agent_name}{family_name}CallIntentUnknown extends {agent_name}{family_name}CallIntent {{\n  const {agent_name}{family_name}CallIntentUnknown(this.raw);\n\n  final sdk.JsonMap raw;\n\n  @override\n  String get type => (raw['type'] as String?) ?? '';\n\n  @override\n  Object? get args => raw['args'];\n\n  @override\n  String toString() => '{agent_name}{family_name}CallIntentUnknown(raw: $raw)';\n}}\n\nfinal class {agent_name}{family_name}ResultIntentUnknown extends {agent_name}{family_name}ResultIntent {{\n  const {agent_name}{family_name}ResultIntentUnknown(this.raw);\n\n  final sdk.JsonMap raw;\n\n  @override\n  String get name => (raw['name'] as String?) ?? '';\n\n  @override\n  Object? get args => raw['args'];\n\n  @override\n  Object? get result => raw['result'];\n\n  @override\n  bool get overridden => (raw['overridden'] as bool?) ?? false;\n\n  @override\n  String toString() => '{agent_name}{family_name}ResultIntentUnknown(raw: $raw)';\n}}\n"
    ));

    if include_error_and_skipped {
        blocks.push(format!(
            "abstract class {agent_name}{family_name}SkippedIntent {{\n  const {agent_name}{family_name}SkippedIntent();\n\n  String get type;\n  Object? get args;\n\n  factory {agent_name}{family_name}SkippedIntent.fromJson(sdk.JsonMap json) {{\n    final kind = (json['type'] as String?) ?? '';\n{}\n    return {agent_name}{family_name}SkippedIntentUnknown(Map<String, Object?>.from(json));\n  }}\n}}\n\nfinal class {agent_name}{family_name}SkippedIntentUnknown extends {agent_name}{family_name}SkippedIntent {{\n  const {agent_name}{family_name}SkippedIntentUnknown(this.raw);\n\n  final sdk.JsonMap raw;\n\n  @override\n  String get type => (raw['type'] as String?) ?? '';\n\n  @override\n  Object? get args => raw['args'];\n\n  @override\n  String toString() => '{agent_name}{family_name}SkippedIntentUnknown(raw: $raw)';\n}}\n\nfinal class {agent_name}{family_name}ErrorIntent {{\n  const {agent_name}{family_name}ErrorIntent({{\n    required this.tool,\n    required this.message,\n  }});\n\n  final String tool;\n  final String message;\n\n  factory {agent_name}{family_name}ErrorIntent.fromJson(sdk.JsonMap json) {{\n    return {agent_name}{family_name}ErrorIntent(\n      tool: (json['tool'] as String?) ?? '',\n      message: (json['message'] as String?) ?? '',\n    );\n  }}\n\n  @override\n  String toString() => '{agent_name}{family_name}ErrorIntent(tool: $tool, message: $message)';\n}}\n",
            skipped_cases.join("\n"),
        ));
    }

    let _ = family_key;
    blocks.join("\n")
}

fn decode_named_shape_expr(
    value: Option<&Value>,
    type_name: &str,
    access_expr: &str,
    null_fallback_expr: &str,
) -> String {
    if shape_variants(value).is_some() {
        return format!(
            "{type_name}.fromJson(Map<String, Object?>.from(({access_expr} as Map?) ?? const {{}}))"
        );
    }

    if shape_properties(value, false).is_some() {
        return format!(
            "{type_name}.fromJson(Map<String, Object?>.from(({access_expr} as Map?) ?? const {{}}))"
        );
    }

    match type_to_dart_string(value, false, "Object?").as_str() {
        "sdk.NoArgs" => "const sdk.NoArgs()".to_string(),
        "sdk.NoResult" => "const sdk.NoResult()".to_string(),
        "String" => format!("({access_expr} as String?) ?? ''"),
        "int" => format!("(({access_expr} as num?)?.toInt()) ?? 0"),
        "double" => format!("(({access_expr} as num?)?.toDouble()) ?? 0"),
        "bool" => format!("({access_expr} as bool?) ?? false"),
        "sdk.JsonMap" => {
            format!("Map<String, Object?>.from(({access_expr} as Map?) ?? const {{}})")
        }
        other => {
            if other == "Object?" {
                null_fallback_expr.to_string()
            } else {
                format!("{access_expr} as {other}")
            }
        }
    }
}

fn is_empty_shape(value: Option<&Value>, unwrap_input_kind: bool) -> bool {
    if value
        .and_then(Value::as_object)
        .is_some_and(|obj| obj.is_empty())
    {
        return true;
    }

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

fn decode_named_shape_expr_optional(
    value: Option<&Value>,
    type_name: &str,
    access_expr: &str,
) -> String {
    if value.is_none() {
        return "null".to_string();
    }

    if shape_variants(value).is_some() || shape_properties(value, false).is_some() {
        return format!(
            "{access_expr} == null ? null : {type_name}.fromJson(Map<String, Object?>.from({access_expr} as Map))"
        );
    }

    match type_to_dart_string(value, false, "Object?").as_str() {
        "String" => format!("{access_expr} as String?"),
        "int" => format!("({access_expr} as num?)?.toInt()"),
        "double" => format!("({access_expr} as num?)?.toDouble()"),
        "bool" => format!("{access_expr} as bool?"),
        "sdk.JsonMap" => format!(
            "{access_expr} == null ? null : Map<String, Object?>.from({access_expr} as Map)"
        ),
        other => {
            if other == "Object?" {
                access_expr.to_string()
            } else {
                format!("{access_expr} as {other}?")
            }
        }
    }
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
            "  FutureOr<void> responseText({agent_name}ResponseTextIntent intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> responseSchema({agent_name}ResponseSchemaIntent intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> error({agent_name}ErrorIntent intent, String agentName) {{}}"
        ),
    ];

    let mut partial_methods = vec![
        format!(
            "  FutureOr<void> responseText(sdk.PartialTextIntentValue intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> responseSchema(sdk.PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent> intent, String agentName) {{}}"
        ),
        format!(
            "  FutureOr<void> error(sdk.PartialStructuredIntentValue<{agent_name}ErrorIntent> intent, String agentName) {{}}"
        ),
    ];

    if has_tools {
        intent_methods.extend([
            format!(
                "  FutureOr<sdk.IntentControl?> toolCall({agent_name}ToolCallIntent intent, String agentName) => null;"
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

        partial_methods.extend([
            format!(
                "  FutureOr<void> toolCall(sdk.PartialStructuredIntentValue<{agent_name}ToolCallIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolResult(sdk.PartialStructuredIntentValue<{agent_name}ToolResultIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolError(sdk.PartialStructuredIntentValue<{agent_name}ToolErrorIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> toolSkipped(sdk.PartialStructuredIntentValue<{agent_name}ToolSkippedIntent> intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_workflows {
        intent_methods.extend([
            format!(
                "  FutureOr<void> workflowCall({agent_name}WorkflowCallIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> workflowResult({agent_name}WorkflowResultIntent intent, String agentName) {{}}"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> workflowCall(sdk.PartialStructuredIntentValue<{agent_name}WorkflowCallIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> workflowResult(sdk.PartialStructuredIntentValue<{agent_name}WorkflowResultIntent> intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_helpers {
        intent_methods.extend([
            format!(
                "  FutureOr<void> helperCall({agent_name}HelperCallIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> helperResult({agent_name}HelperResultIntent intent, String agentName) {{}}"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> helperCall(sdk.PartialStructuredIntentValue<{agent_name}HelperCallIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> helperResult(sdk.PartialStructuredIntentValue<{agent_name}HelperResultIntent> intent, String agentName) {{}}"
            ),
        ]);
    }

    if has_components {
        intent_methods.extend([
            format!(
                "  FutureOr<void> component({agent_name}ComponentIntent intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> renderComponent({agent_name}RenderComponentIntent intent, String agentName) {{}}"
            ),
        ]);
        partial_methods.extend([
            format!(
                "  FutureOr<void> component(sdk.PartialStructuredIntentValue<{agent_name}ComponentIntent> intent, String agentName) {{}}"
            ),
            format!(
                "  FutureOr<void> renderComponent(sdk.PartialStructuredIntentValue<{agent_name}RenderComponentIntent> intent, String agentName) {{}}"
            ),
        ]);
    }

    for custom_intent in custom_intents {
        let type_name = custom_intent_type_name(agent_name, custom_intent);
        let method_name = dart_method_name(custom_intent);
        intent_methods.push(format!(
            "  FutureOr<void> {method_name}({type_name} intent, String agentName) {{}}"
        ));
        partial_methods.push(format!(
            "  FutureOr<void> {method_name}(sdk.PartialStructuredIntentValue<{type_name}> intent, String agentName) {{}}"
        ));
    }

    format!(
        "abstract class {agent_name}BaseIntentHandler {{\n{}\n}}\n\nabstract class {agent_name}BasePartialIntentHandler {{\n{}\n}}\n\nabstract class {agent_name}Middleware implements sdk.Middleware {{\n  const {agent_name}Middleware();\n\n  @override\n  String get name => runtimeType.toString();\n\n  @override\n  Object? get target => null;\n\n  @override\n  FutureOr<sdk.SessionState> onRunStart(sdk.SessionState session, sdk.MiddlewareContext ctx) => session;\n\n  @override\n  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) => null;\n\n  @override\n  FutureOr<void> onLLMEnd(Object? response, sdk.MiddlewareContext ctx) {{}}\n\n  @override\n  FutureOr<void> onRunComplete(sdk.SessionState finalSession, sdk.MiddlewareContext ctx) {{}}\n\n  @override\n  FutureOr<bool> onError(Object error, sdk.SessionState? session, sdk.MiddlewareContext ctx) => false;\n\n  FutureOr<void> responseText({agent_name}ResponseTextIntent intent, sdk.MiddlewareContext ctx) {{}}\n  FutureOr<void> responseSchema({agent_name}ResponseSchemaIntent intent, sdk.MiddlewareContext ctx) {{}}\n  FutureOr<void> errorIntent({agent_name}ErrorIntent intent, sdk.MiddlewareContext ctx) {{}}\n{}\n{}\n  @override\n  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) => _dispatchMiddlewareIntent(this, name, value, ctx);\n\n  @override\n  FutureOr<void> onIntentPartial(String name, Object? value, sdk.MiddlewareContext ctx) {{\n    _dispatchMiddlewarePartialIntent(this, name, value, ctx);\n  }}\n}}\n",
        intent_methods.join("\n"),
        partial_methods.join("\n"),
        generate_middleware_intent_methods(agent_name, has_tools, has_workflows, has_helpers, has_components, custom_intents),
        generate_middleware_partial_methods(agent_name, has_tools, has_workflows, has_helpers, has_components, custom_intents),
    )
}

fn generate_middleware_intent_methods(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut lines = Vec::new();
    if has_tools {
        lines.extend([
            format!("  FutureOr<sdk.IntentControl?> toolCall({agent_name}ToolCallIntent intent, sdk.MiddlewareContext ctx) => null;"),
            format!("  FutureOr<void> toolResult({agent_name}ToolResultIntent intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> toolError({agent_name}ToolErrorIntent intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> toolSkipped({agent_name}ToolSkippedIntent intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_workflows {
        lines.extend([
            format!("  FutureOr<void> workflowCall({agent_name}WorkflowCallIntent intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> workflowResult({agent_name}WorkflowResultIntent intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_helpers {
        lines.extend([
            format!("  FutureOr<void> helperCall({agent_name}HelperCallIntent intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> helperResult({agent_name}HelperResultIntent intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_components {
        lines.extend([
            format!("  FutureOr<void> component({agent_name}ComponentIntent intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> renderComponent({agent_name}RenderComponentIntent intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    for custom_intent in custom_intents {
        let type_name = custom_intent_type_name(agent_name, custom_intent);
        let method_name = dart_method_name(custom_intent);
        lines.push(format!(
            "  FutureOr<void> {method_name}({type_name} intent, sdk.MiddlewareContext ctx) {{}}"
        ));
    }
    if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) }
}

fn generate_middleware_partial_methods(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
) -> String {
    let mut lines = vec![
        format!("  FutureOr<void> partialResponseText(sdk.PartialTextIntentValue intent, sdk.MiddlewareContext ctx) {{}}"),
        format!("  FutureOr<void> partialResponseSchema(sdk.PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
        format!("  FutureOr<void> partialError(sdk.PartialStructuredIntentValue<{agent_name}ErrorIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
    ];
    if has_tools {
        lines.extend([
            format!("  FutureOr<void> partialToolCall(sdk.PartialStructuredIntentValue<{agent_name}ToolCallIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialToolResult(sdk.PartialStructuredIntentValue<{agent_name}ToolResultIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialToolError(sdk.PartialStructuredIntentValue<{agent_name}ToolErrorIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialToolSkipped(sdk.PartialStructuredIntentValue<{agent_name}ToolSkippedIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_workflows {
        lines.extend([
            format!("  FutureOr<void> partialWorkflowCall(sdk.PartialStructuredIntentValue<{agent_name}WorkflowCallIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialWorkflowResult(sdk.PartialStructuredIntentValue<{agent_name}WorkflowResultIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_helpers {
        lines.extend([
            format!("  FutureOr<void> partialHelperCall(sdk.PartialStructuredIntentValue<{agent_name}HelperCallIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialHelperResult(sdk.PartialStructuredIntentValue<{agent_name}HelperResultIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    if has_components {
        lines.extend([
            format!("  FutureOr<void> partialComponent(sdk.PartialStructuredIntentValue<{agent_name}ComponentIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
            format!("  FutureOr<void> partialRenderComponent(sdk.PartialStructuredIntentValue<{agent_name}RenderComponentIntent> intent, sdk.MiddlewareContext ctx) {{}}"),
        ]);
    }
    for custom_intent in custom_intents {
        let type_name = custom_intent_type_name(agent_name, custom_intent);
        let method_name = dart_method_name(custom_intent);
        lines.push(format!(
            "  FutureOr<void> partial{}(sdk.PartialStructuredIntentValue<{type_name}> intent, sdk.MiddlewareContext ctx) {{}}",
            pascal_case_identifier(&method_name)
        ));
    }
    lines.join("\n")
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

fn generate_config_class(
    agent_name: &str,
    has_tools: bool,
    has_api_keys: bool,
    has_context: bool,
) -> String {
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

    let tools_ctor = if has_tools {
        "    required this.tools,\n".to_string()
    } else {
        format!("    this.tools = const {agent_name}ToolRegistry(),\n")
    };

    let tools_expr = "      tools: tools.toMap(),";

    let context_field = if has_context {
        format!("  final {agent_name}Context? context;\n")
    } else {
        "  final sdk.JsonMap? context;\n".to_string()
    };

    format!(
        "final class {agent_name}Config {{\n  const {agent_name}Config({{\n{tools_ctor}    this.middleware = const [],\n    this.context,\n{api_keys_ctor}    this.libraryPath,\n  }});\n\n  final {agent_name}Tools tools;\n  final List<{agent_name}Middleware> middleware;\n{context_field}{api_keys_field}  final String? libraryPath;\n\n  sdk.AuwgentConfig toAuwgentConfig() {{\n    return sdk.AuwgentConfig(\n{tools_expr}\n      middleware: middleware,\n      context: context,\n{api_keys_expr}\n      libraryPath: libraryPath,\n    );\n  }}\n}}\n"
    )
}

fn generate_agent_class(
    agent_name: &str,
    _has_tools: bool,
    _has_workflows: bool,
    _has_helpers: bool,
    _has_components: bool,
    _custom_intents: &[String],
) -> String {
    format!(
        "final class {agent_name}Agent extends sdk.TypedAuwgent<sdk.JsonMap> {{\n  {agent_name}Agent({agent_name}Config config)\n      : super(decode{}Ir(), config.toAuwgentConfig());\n\n  void onIntentHandler({agent_name}BaseIntentHandler handler) {{\n    onIntent((name, value, agentName) => _dispatchIntent(handler, name, value, agentName));\n  }}\n\n  void onIntentPartialHandler({agent_name}BasePartialIntentHandler handler) {{\n    onIntentPartial((name, value, agentName) {{\n      _dispatchPartialIntent(handler, name, value, agentName);\n    }});\n  }}\n}}\n\nFutureOr<sdk.IntentControl?> _dispatchIntent({agent_name}BaseIntentHandler handler, String name, Object? value, String agentName) {{\n  switch (name) {{\n{}\n    default:\n      return null;\n  }}\n}}\n\nvoid _dispatchPartialIntent({agent_name}BasePartialIntentHandler handler, String name, Object? value, String agentName) {{\n  switch (name) {{\n{}\n    default:\n      return;\n  }}\n}}\n\nFutureOr<sdk.IntentControl?> _dispatchMiddlewareIntent({agent_name}Middleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {{\n  switch (name) {{\n{}\n    default:\n      return null;\n  }}\n}}\n\nvoid _dispatchMiddlewarePartialIntent({agent_name}Middleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {{\n  switch (name) {{\n{}\n    default:\n      return;\n  }}\n}}\n",
        sanitize_identifier(agent_name, false),
        generate_dispatch_cases(
            agent_name,
            _has_tools,
            _has_workflows,
            _has_helpers,
            _has_components,
            _custom_intents,
            false,
        ),
        generate_dispatch_cases(
            agent_name,
            _has_tools,
            _has_workflows,
            _has_helpers,
            _has_components,
            _custom_intents,
            true,
        ),
        generate_middleware_dispatch_cases(
            agent_name,
            _has_tools,
            _has_workflows,
            _has_helpers,
            _has_components,
            _custom_intents,
            false,
        ),
        generate_middleware_dispatch_cases(
            agent_name,
            _has_tools,
            _has_workflows,
            _has_helpers,
            _has_components,
            _custom_intents,
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
            &if partial {
                "sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap)".to_string()
            } else {
                format!("{agent_name}ResponseTextIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
        dispatch_case(
            "response_schema",
            "responseSchema",
            &if partial {
                format!(
                    "sdk.PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent>.fromJson(value as sdk.JsonMap, {agent_name}ResponseSchemaIntent.fromJson)"
                )
            } else {
                format!("{agent_name}ResponseSchemaIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
        dispatch_case(
            "error",
            "error",
            &if partial {
                format!(
                    "sdk.PartialStructuredIntentValue<{agent_name}ErrorIntent>.fromJson(value as sdk.JsonMap, {agent_name}ErrorIntent.fromJson)"
                )
            } else {
                format!("{agent_name}ErrorIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
    ];

    if has_tools {
        lines.extend([
            dispatch_case(
                "tool_call",
                "toolCall",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolCallIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "tool_result",
                "toolResult",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolResultIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "tool_error",
                "toolError",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolErrorIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolErrorIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolErrorIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "tool_skipped",
                "toolSkipped",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolSkippedIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolSkippedIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolSkippedIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }

    if has_workflows {
        lines.extend([
            dispatch_case(
                "workflow_call",
                "workflowCall",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}WorkflowCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}WorkflowCallIntent.fromJson)")
                } else {
                    format!("{agent_name}WorkflowCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "workflow_result",
                "workflowResult",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}WorkflowResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}WorkflowResultIntent.fromJson)")
                } else {
                    format!("{agent_name}WorkflowResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }

    if has_helpers {
        lines.extend([
            dispatch_case(
                "helper_call",
                "helperCall",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}HelperCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}HelperCallIntent.fromJson)")
                } else {
                    format!("{agent_name}HelperCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "helper_result",
                "helperResult",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}HelperResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}HelperResultIntent.fromJson)")
                } else {
                    format!("{agent_name}HelperResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }

    if has_components {
        lines.extend([
            dispatch_case(
                "component",
                "component",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ComponentIntent>.fromJson(value as sdk.JsonMap, {agent_name}ComponentIntent.fromJson)")
                } else {
                    format!("{agent_name}ComponentIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            dispatch_case(
                "render_component",
                "renderComponent",
                &if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}RenderComponentIntent>.fromJson(value as sdk.JsonMap, {agent_name}RenderComponentIntent.fromJson)")
                } else {
                    format!("{agent_name}RenderComponentIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }

    for custom_intent in custom_intents {
        lines.push(dispatch_case(
            custom_intent,
            &dart_method_name(custom_intent),
            &if partial {
                let type_name = custom_intent_type_name(agent_name, custom_intent);
                format!(
                    "sdk.PartialStructuredIntentValue<{type_name}>.fromJson(value as sdk.JsonMap, {type_name}.fromJson)"
                )
            } else {
                format!(
                    "{}.fromJson(value as sdk.JsonMap)",
                    custom_intent_type_name(agent_name, custom_intent)
                )
            },
            partial,
        ));
    }

    lines.join("\n")
}

fn generate_middleware_dispatch_cases(
    agent_name: &str,
    has_tools: bool,
    has_workflows: bool,
    has_helpers: bool,
    has_components: bool,
    custom_intents: &[String],
    partial: bool,
) -> String {
    let mut lines = vec![
        middleware_dispatch_case(
            "response_text",
            if partial { "partialResponseText" } else { "responseText" },
            if partial {
                "sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap)".to_string()
            } else {
                format!("{agent_name}ResponseTextIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
        middleware_dispatch_case(
            "response_schema",
            if partial { "partialResponseSchema" } else { "responseSchema" },
            if partial {
                format!(
                    "sdk.PartialStructuredIntentValue<{agent_name}ResponseSchemaIntent>.fromJson(value as sdk.JsonMap, {agent_name}ResponseSchemaIntent.fromJson)"
                )
            } else {
                format!("{agent_name}ResponseSchemaIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
        middleware_dispatch_case(
            "error",
            if partial { "partialError" } else { "errorIntent" },
            if partial {
                format!(
                    "sdk.PartialStructuredIntentValue<{agent_name}ErrorIntent>.fromJson(value as sdk.JsonMap, {agent_name}ErrorIntent.fromJson)"
                )
            } else {
                format!("{agent_name}ErrorIntent.fromJson(value as sdk.JsonMap)")
            },
            partial,
        ),
    ];
    if has_tools {
        lines.extend([
            middleware_dispatch_case(
                "tool_call",
                if partial { "partialToolCall" } else { "toolCall" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolCallIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "tool_result",
                if partial { "partialToolResult" } else { "toolResult" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolResultIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "tool_error",
                if partial { "partialToolError" } else { "toolError" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolErrorIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolErrorIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolErrorIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "tool_skipped",
                if partial { "partialToolSkipped" } else { "toolSkipped" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ToolSkippedIntent>.fromJson(value as sdk.JsonMap, {agent_name}ToolSkippedIntent.fromJson)")
                } else {
                    format!("{agent_name}ToolSkippedIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }
    if has_workflows {
        lines.extend([
            middleware_dispatch_case(
                "workflow_call",
                if partial { "partialWorkflowCall" } else { "workflowCall" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}WorkflowCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}WorkflowCallIntent.fromJson)")
                } else {
                    format!("{agent_name}WorkflowCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "workflow_result",
                if partial { "partialWorkflowResult" } else { "workflowResult" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}WorkflowResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}WorkflowResultIntent.fromJson)")
                } else {
                    format!("{agent_name}WorkflowResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }
    if has_helpers {
        lines.extend([
            middleware_dispatch_case(
                "helper_call",
                if partial { "partialHelperCall" } else { "helperCall" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}HelperCallIntent>.fromJson(value as sdk.JsonMap, {agent_name}HelperCallIntent.fromJson)")
                } else {
                    format!("{agent_name}HelperCallIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "helper_result",
                if partial { "partialHelperResult" } else { "helperResult" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}HelperResultIntent>.fromJson(value as sdk.JsonMap, {agent_name}HelperResultIntent.fromJson)")
                } else {
                    format!("{agent_name}HelperResultIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }
    if has_components {
        lines.extend([
            middleware_dispatch_case(
                "component",
                if partial { "partialComponent" } else { "component" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}ComponentIntent>.fromJson(value as sdk.JsonMap, {agent_name}ComponentIntent.fromJson)")
                } else {
                    format!("{agent_name}ComponentIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
            middleware_dispatch_case(
                "render_component",
                if partial { "partialRenderComponent" } else { "renderComponent" },
                if partial {
                    format!("sdk.PartialStructuredIntentValue<{agent_name}RenderComponentIntent>.fromJson(value as sdk.JsonMap, {agent_name}RenderComponentIntent.fromJson)")
                } else {
                    format!("{agent_name}RenderComponentIntent.fromJson(value as sdk.JsonMap)")
                },
                partial,
            ),
        ]);
    }
    for custom_intent in custom_intents {
        let method_name = if partial {
            format!("partial{}", pascal_case_identifier(&dart_method_name(custom_intent)))
        } else {
            dart_method_name(custom_intent)
        };
        let type_name = custom_intent_type_name(agent_name, custom_intent);
        let value_expr = if partial {
            format!(
                "sdk.PartialStructuredIntentValue<{type_name}>.fromJson(value as sdk.JsonMap, {type_name}.fromJson)"
            )
        } else {
            format!("{type_name}.fromJson(value as sdk.JsonMap)")
        };
        lines.push(middleware_dispatch_case(custom_intent, &method_name, value_expr, partial));
    }
    lines.join("\n")
}

fn middleware_dispatch_case(intent_name: &str, method_name: &str, value_expr: String, partial: bool) -> String {
    if partial {
        format!("    case '{intent_name}':\n      middleware.{method_name}({value_expr}, ctx);\n      return;")
    } else if intent_name == "tool_call" {
        format!("    case '{intent_name}':\n      return middleware.{method_name}({value_expr}, ctx);")
    } else {
        format!("    case '{intent_name}':\n      middleware.{method_name}({value_expr}, ctx);\n      return null;")
    }
}

fn dispatch_case(intent_name: &str, method_name: &str, value_expr: &str, partial: bool) -> String {
    if partial {
        format!("    case '{intent_name}':\n      handler.{method_name}({value_expr}, agentName);\n      return;")
    } else if intent_name == "tool_call" {
        format!("    case '{intent_name}':\n      return handler.{method_name}({value_expr}, agentName);")
    } else {
        format!("    case '{intent_name}':\n      handler.{method_name}({value_expr}, agentName);\n      return null;")
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
        format!("typedef AuwgentToolRegistry = {agent_name}ToolRegistry;"),
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

    #[test]
    fn emits_dart_variant_output_models() {
        let ir = json!({
            "name": "Hello",
            "input": null,
            "output": {
                "__variants": {
                    "Simple": {
                        "simple": {
                            "type": "string",
                            "optional": false
                        }
                    },
                    "Person": {
                        "name": {
                            "type": "string",
                            "optional": false
                        },
                        "age": {
                            "type": "number",
                            "optional": false
                        }
                    }
                }
            },
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [],
            "modelConfig": []
        });

        let output = generate(&ir, "hello");
        assert!(output.contains("abstract class HelloOutput"));
        assert!(output.contains("final class HelloOutputSimple extends HelloOutput"));
        assert!(output.contains("final class HelloOutputPerson extends HelloOutput"));
        assert!(output.contains("String get variant => 'Person';"));
        assert!(output.contains("final HelloOutput response;"));
    }

    #[test]
    fn emits_dart_no_args_for_empty_tool_params() {
        let ir = json!({
            "name": "Hello",
            "input": null,
            "output": null,
            "context": null,
            "tools": [{
                "name": "get_details",
                "params": {},
                "returns": "string"
            }],
            "workflows": [],
            "helpers": [],
            "components": [],
            "modelConfig": []
        });

        let output = generate(&ir, "hello");
        assert!(output.contains("sdk.NoArgs get args => const sdk.NoArgs();"));
        assert!(output.contains("return const HelloGetDetailsToolCallIntentCase();"));
        assert!(!output.contains("final class HelloGetDetailsToolArgs"));
    }
}
