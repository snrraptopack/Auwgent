use crate::types::TypeDefinition;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// IR TYPE → JSON SCHEMA CONVERTER
// ═══════════════════════════════════════════════════════════════════════════

/// Build a JSON Schema object from a map of IR parameter definitions.
///
/// `params` is a JSON object where each value is an IR param shape:
/// `{ "type": <type-value>, "optional": bool, "description": string|null }`
///
/// When `strict` is true (OpenAI strict mode):
/// - All objects get `additionalProperties: false`
/// - All properties go into `required`
/// - Optional properties get nullable types: `{ "type": ["T", "null"] }`
pub fn build_schema_from_params(
    params: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();

    if let Some(params_obj) = params.as_object() {
        for (key, param_def) in params_obj {
            let schema_prop = param_def_to_schema_property(param_def, types, strict);
            properties.insert(key.clone(), schema_prop);

            let is_optional = param_def
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if strict || !is_optional {
                required.push(json!(key));
            }
        }
    }

    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "required": required,
    });

    if let Some(obj) = schema.as_object_mut() {
        obj.insert("additionalProperties".to_string(), json!(false));
    }

    schema
}

/// Build JSON Schema for a helper's input.
///
/// Mirrors `flat_args::flatten_helper_input_specs` semantics:
/// - `null` / absent → `{"input": {"type": "string"}}`
/// - `{"kind": "properties", "fields": {...}}` → schema from fields
/// - `{"kind": "direct", "type": <type>}` → single `input` param of that type
/// - bare object → schema from its filtered properties
pub fn build_helper_input_schema(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    let input = match input_ir {
        Some(v) if !v.is_null() => v,
        _ => {
            return json!({
                "type": "object",
                "properties": { "input": { "type": "string" } },
                "required": ["input"],
                "additionalProperties": false,
            });
        }
    };

    // kind: properties
    if input.get("kind").and_then(|v| v.as_str()) == Some("properties") {
        if let Some(fields) = input.get("fields") {
            return build_schema_from_params(fields, types, strict);
        }
    }

    // kind: direct
    if input.get("kind").and_then(|v| v.as_str()) == Some("direct") {
        if let Some(ty) = input.get("type") {
            if let Some(props) = resolve_nested_properties(ty, types) {
                return build_schema_from_params(&Value::Object(props), types, strict);
            }
            let mut schema = json!({
                "type": "object",
                "properties": { "input": type_value_to_schema(ty, types, strict) },
                "required": ["input"],
            });
            if let Some(obj) = schema.as_object_mut() {
                obj.insert("additionalProperties".to_string(), json!(false));
            }
            return schema;
        }
    }

    // Try resolving as a nested object / typeRef
    if let Some(props) = resolve_nested_properties(input, types) {
        return build_schema_from_params(&Value::Object(props), types, strict);
    }

    // Bare object — filter metadata keys and use as params
    if let Some(obj) = input.as_object() {
        let filtered: Map<String, Value> = obj
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__") && *key != "kind")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        if !filtered.is_empty() {
            return build_schema_from_params(&Value::Object(filtered), types, strict);
        }
    }

    // Fallback: empty object
    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false,
    })
}

/// Build JSON Schema for agent final output.
///
/// Supports `__variants` for discriminated output shapes.
pub fn build_output_schema(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Option<Value> {
    if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
        // Build an anyOf / oneOf schema with discriminant
        let mut variant_schemas = Vec::new();
        for (name, schema_value) in variants {
            let mut variant_schema = build_schema_from_output_variant(schema_value, types, strict);
            if let Some(obj) = variant_schema.as_object_mut() {
                // Inject discriminant property so model knows which variant
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    props.insert(
                        "__variant".to_string(),
                        json!({ "type": "string", "enum": [name] }),
                    );
                }
                if let Some(req) = obj.get_mut("required").and_then(|v| v.as_array_mut()) {
                    if !req.iter().any(|v| v.as_str() == Some("__variant")) {
                        req.push(json!("__variant"));
                    }
                }
            }
            variant_schemas.push(variant_schema);
        }

        if variant_schemas.len() == 1 {
            return Some(variant_schemas.into_iter().next().unwrap());
        }

        return Some(json!({
            "anyOf": variant_schemas,
        }));
    }

    let schema = build_schema_from_output_variant(output, types, strict);
    if schema
        .get("properties")
        .and_then(|v| v.as_object())
        .map(|o| o.is_empty())
        .unwrap_or(true)
    {
        return None;
    }
    Some(schema)
}

// ═══════════════════════════════════════════════════════════════════════════
// INTERNAL — recursive type resolution
// ═══════════════════════════════════════════════════════════════════════════

fn param_def_to_schema_property(
    param_def: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    let optional = param_def
        .get("optional")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Direct typeRef at param level: { "type": "typeRef", "name": "Foo" }
    let mut schema = if param_def.get("type").and_then(|v| v.as_str()) == Some("typeRef") {
        if let Some(name) = param_def.get("name").and_then(|v| v.as_str()) {
            resolve_type_ref(name, types, strict)
        } else {
            json!({ "type": "object" })
        }
    } else {
        let type_val = param_def.get("type").unwrap_or(&Value::Null);
        type_value_to_schema(type_val, types, strict)
    };

    // Copy description
    if let Some(desc) = param_def.get("description").and_then(|v| v.as_str()) {
        if let Some(obj) = schema.as_object_mut() {
            obj.insert("description".to_string(), json!(desc));
        }
    }

    // Strict mode: optional fields become nullable
    if strict && optional {
        make_schema_nullable(&mut schema);
    }

    schema
}

fn type_value_to_schema(
    type_val: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    match type_val {
        Value::String(s) => primitive_to_schema(s),
        Value::Object(obj) => {
            if let Some(type_tag) = obj.get("type").and_then(|v| v.as_str()) {
                match type_tag {
                    "array" => array_to_schema(obj, types, strict),
                    "object" => object_to_schema(obj, types, strict),
                    "union" => union_to_schema(obj),
                    "literal" => literal_to_schema(obj),
                    "typeRef" => type_ref_to_schema(obj, types, strict),
                    _ => json!({ "type": type_tag }),
                }
            } else {
                // e.g. { "type": { "type": "object", ... } } — unwrap inner
                if let Some(inner) = obj.get("type") {
                    type_value_to_schema(inner, types, strict)
                } else {
                    json!({})
                }
            }
        }
        _ => json!({}),
    }
}

fn primitive_to_schema(type_name: &str) -> Value {
    match type_name {
        "any" => json!({}),
        "int" | "float" | "number" => json!({ "type": "number" }),
        "bool" | "boolean" => json!({ "type": "boolean" }),
        "string" => json!({ "type": "string" }),
        other => json!({ "type": other }),
    }
}

fn array_to_schema(
    obj: &Map<String, Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    let items = obj.get("items").unwrap_or(&Value::Null);
    json!({
        "type": "array",
        "items": type_value_to_schema(items, types, strict),
    })
}

fn object_to_schema(
    obj: &Map<String, Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        build_schema_from_params(&Value::Object(props.clone()), types, strict)
    } else {
        json!({ "type": "object", "additionalProperties": false })
    }
}

fn union_to_schema(obj: &Map<String, Value>) -> Value {
    if let Some(options) = obj.get("options").and_then(|v| v.as_array()) {
        // All options should be scalar literals → produce an enum
        let values: Vec<Value> = options.clone();
        if let Some(first) = values.first() {
            let inferred_type = infer_json_schema_type(first);
            return json!({
                "type": inferred_type,
                "enum": values,
            });
        }
    }
    json!({})
}

fn literal_to_schema(obj: &Map<String, Value>) -> Value {
    let val = obj.get("value").unwrap_or(&Value::Null);
    let t = infer_json_schema_type(val);
    json!({
        "type": t,
        "enum": [val.clone()],
    })
}

fn type_ref_to_schema(
    obj: &Map<String, Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        resolve_type_ref(name, types, strict)
    } else {
        json!({ "type": "object" })
    }
}

fn resolve_type_ref(
    name: &str,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    if let Some(type_map) = types {
        if let Some(type_def) = type_map.get(name) {
            if let Ok(props_value) = serde_json::to_value(&type_def.properties) {
                if let Some(props_obj) = props_value.as_object() {
                    return build_schema_from_params(
                        &Value::Object(props_obj.clone()),
                        types,
                        strict,
                    );
                }
            }
        }
    }
    json!({ "type": "object" })
}

// ═══════════════════════════════════════════════════════════════════════════
// INTERNAL — helpers
// ═══════════════════════════════════════════════════════════════════════════

fn resolve_nested_properties(
    def: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Option<Map<String, Value>> {
    if def.get("type").and_then(|v| v.as_str()) == Some("object") {
        return def.get("properties").and_then(|v| v.as_object()).cloned();
    }

    if let Some(type_obj) = def.get("type").and_then(|v| v.as_object()) {
        if type_obj.get("type").and_then(|v| v.as_str()) == Some("object") {
            return type_obj
                .get("properties")
                .and_then(|v| v.as_object())
                .cloned();
        }

        if type_obj.get("type").and_then(|v| v.as_str()) == Some("typeRef") {
            let ref_name = type_obj.get("name").and_then(|v| v.as_str())?;
            let custom_type = types?.get(ref_name)?;
            let props_value = serde_json::to_value(&custom_type.properties).ok()?;
            return props_value.as_object().cloned();
        }
    }

    if def.get("type").and_then(|v| v.as_str()) == Some("typeRef") {
        let ref_name = def.get("name").and_then(|v| v.as_str())?;
        let custom_type = types?.get(ref_name)?;
        let props_value = serde_json::to_value(&custom_type.properties).ok()?;
        return props_value.as_object().cloned();
    }

    None
}

fn build_schema_from_output_variant(
    schema_value: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,
) -> Value {
    if let Some(obj) = schema_value.as_object() {
        if let Some(properties) = obj.get("properties").and_then(|v| v.as_object()) {
            return build_schema_from_params(&Value::Object(properties.clone()), types, strict);
        }

        if let Some(type_value) = obj.get("type") {
            if let Some(properties) = resolve_nested_properties(type_value, types) {
                return build_schema_from_params(&Value::Object(properties), types, strict);
            }
        }

        // Filter metadata keys and treat as direct properties
        let filtered: Map<String, Value> = obj
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        if !filtered.is_empty() {
            return build_schema_from_params(&Value::Object(filtered), types, strict);
        }
    }

    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false,
    })
}

fn infer_json_schema_type(val: &Value) -> String {
    match val {
        Value::String(_) => "string".to_string(),
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        _ => "string".to_string(),
    }
}

fn make_schema_nullable(schema: &mut Value) {
    if let Some(obj) = schema.as_object_mut() {
        if let Some(type_val) = obj.get("type").cloned() {
            if let Some(type_str) = type_val.as_str() {
                obj.insert("type".to_string(), json!([type_str, "null"]));
            } else if let Some(arr) = type_val.as_array() {
                let mut types: Vec<Value> = arr.iter().cloned().collect();
                if !types.iter().any(|v| v.as_str() == Some("null")) {
                    types.push(json!("null"));
                }
                obj.insert("type".to_string(), Value::Array(types));
            }
        } else if obj.contains_key("enum") {
            // Enum without explicit type — infer from first value
            if let Some(first) = obj
                .get("enum")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
            {
                let inferred = infer_json_schema_type(first);
                obj.insert("type".to_string(), json!([inferred, "null"]));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn primitive_params() {
        let params = json!({
            "name": { "type": "string", "optional": false },
            "age": { "type": "number", "optional": false },
            "active": { "type": "boolean", "optional": false },
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["age"]["type"], "number");
        assert_eq!(schema["properties"]["active"]["type"], "boolean");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("name"))
        );
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("age"))
        );
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn optional_fields_non_strict() {
        let params = json!({
            "req": { "type": "string", "optional": false },
            "opt": { "type": "number", "optional": true },
        });

        let schema = build_schema_from_params(&params, None, false);
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("req")));
        assert!(!required.contains(&json!("opt")));
        assert_eq!(schema["properties"]["opt"]["type"], "number");
    }

    #[test]
    fn optional_fields_strict() {
        let params = json!({
            "req": { "type": "string", "optional": false },
            "opt": { "type": "number", "optional": true },
        });

        let schema = build_schema_from_params(&params, None, true);
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("req")));
        assert!(required.contains(&json!("opt")));
        assert_eq!(
            schema["properties"]["opt"]["type"],
            json!(["number", "null"])
        );
    }

    #[test]
    fn union_type_becomes_enum() {
        let params = json!({
            "priority": {
                "type": { "type": "union", "options": ["low", "medium", "high"] },
                "optional": false
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(schema["properties"]["priority"]["type"], "string");
        assert_eq!(
            schema["properties"]["priority"]["enum"],
            json!(["low", "medium", "high"])
        );
    }

    #[test]
    fn literal_type_infers_type() {
        let params = json!({
            "count": {
                "type": { "type": "literal", "value": 42 },
                "optional": false
            },
            "label": {
                "type": { "type": "literal", "value": "default" },
                "optional": false
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(schema["properties"]["count"]["type"], "integer");
        assert_eq!(schema["properties"]["count"]["enum"], json!([42]));
        assert_eq!(schema["properties"]["label"]["type"], "string");
        assert_eq!(schema["properties"]["label"]["enum"], json!(["default"]));
    }

    #[test]
    fn array_of_primitives() {
        let params = json!({
            "tags": {
                "type": { "type": "array", "items": "string" },
                "optional": false
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(schema["properties"]["tags"]["type"], "array");
        assert_eq!(schema["properties"]["tags"]["items"]["type"], "string");
    }

    #[test]
    fn array_of_objects() {
        let params = json!({
            "items": {
                "type": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "optional": false }
                        }
                    }
                },
                "optional": false
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(schema["properties"]["items"]["type"], "array");
        assert_eq!(schema["properties"]["items"]["items"]["type"], "object");
        assert_eq!(
            schema["properties"]["items"]["items"]["properties"]["name"]["type"],
            "string"
        );
    }

    #[test]
    fn nested_object_type() {
        let params = json!({
            "address": {
                "type": {
                    "type": "object",
                    "properties": {
                        "street": { "type": "string", "optional": false },
                        "city": { "type": "string", "optional": true }
                    }
                },
                "optional": false
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        let addr = &schema["properties"]["address"];
        assert_eq!(addr["type"], "object");
        assert_eq!(addr["properties"]["street"]["type"], "string");
        assert_eq!(addr["properties"]["city"]["type"], "string");
        assert!(
            addr["required"]
                .as_array()
                .unwrap()
                .contains(&json!("street"))
        );
        assert!(
            !addr["required"]
                .as_array()
                .unwrap()
                .contains(&json!("city"))
        );
        assert_eq!(addr["additionalProperties"], false);
    }

    #[test]
    fn type_ref_resolution() {
        let mut types = HashMap::new();
        types.insert(
            "UserProfile".to_string(),
            TypeDefinition {
                is_output: false,
                properties: {
                    let mut m = HashMap::new();
                    m.insert(
                        "name".to_string(),
                        crate::types::TypeProperty {
                            type_value: crate::types::JsonValue(json!("string")),
                            optional: false,
                            description: None,
                        },
                    );
                    m.insert(
                        "age".to_string(),
                        crate::types::TypeProperty {
                            type_value: crate::types::JsonValue(json!("number")),
                            optional: true,
                            description: None,
                        },
                    );
                    m
                },
                examples: vec![],
            },
        );

        // Direct typeRef at param level
        let params = json!({
            "user": { "type": "typeRef", "name": "UserProfile", "optional": false }
        });
        let schema = build_schema_from_params(&params, Some(&types), false);
        assert_eq!(schema["properties"]["user"]["type"], "object");
        assert_eq!(
            schema["properties"]["user"]["properties"]["name"]["type"],
            "string"
        );

        // Nested typeRef in type object
        let params2 = json!({
            "user": {
                "type": { "type": "typeRef", "name": "UserProfile" },
                "optional": false
            }
        });
        let schema2 = build_schema_from_params(&params2, Some(&types), false);
        assert_eq!(schema2["properties"]["user"]["type"], "object");
        assert_eq!(
            schema2["properties"]["user"]["properties"]["name"]["type"],
            "string"
        );
    }

    #[test]
    fn helper_null_input_defaults_to_text() {
        let schema = build_helper_input_schema(None, None, false);
        assert_eq!(schema["properties"]["input"]["type"], "string");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("input"))
        );
    }

    #[test]
    fn helper_properties_input() {
        let input = json!({
            "kind": "properties",
            "fields": {
                "text": { "type": "string", "optional": false },
                "max_length": { "type": "number", "optional": true }
            }
        });

        let schema = build_helper_input_schema(Some(&input), None, false);
        assert_eq!(schema["properties"]["text"]["type"], "string");
        assert_eq!(schema["properties"]["max_length"]["type"], "number");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("text"))
        );
        assert!(
            !schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("max_length"))
        );
    }

    #[test]
    fn helper_direct_input_with_object_type() {
        let input = json!({
            "kind": "direct",
            "type": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "optional": false }
                }
            }
        });

        let schema = build_helper_input_schema(Some(&input), None, false);
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    #[test]
    fn helper_direct_input_with_primitive_type() {
        let input = json!({
            "kind": "direct",
            "type": "string"
        });

        let schema = build_helper_input_schema(Some(&input), None, false);
        assert_eq!(schema["properties"]["input"]["type"], "string");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("input"))
        );
    }

    #[test]
    fn output_schema_with_variants() {
        let output = json!({
            "__variants": {
                "Success": {
                    "status": { "type": "string", "optional": false },
                    "data": { "type": "string", "optional": false }
                },
                "Error": {
                    "message": { "type": "string", "optional": false }
                }
            }
        });

        let schema = build_output_schema(&output, None, false).unwrap();
        let variants = schema["anyOf"].as_array().unwrap();
        assert_eq!(variants.len(), 2);

        // Check discriminant injection
        let success = &variants[0];
        assert_eq!(
            success["properties"]["__variant"]["enum"],
            json!(["Success"])
        );
        assert!(
            success["required"]
                .as_array()
                .unwrap()
                .contains(&json!("__variant"))
        );
    }

    #[test]
    fn output_schema_single_variant_no_anyof() {
        let output = json!({
            "__variants": {
                "Result": {
                    "value": { "type": "string", "optional": false }
                }
            }
        });

        let schema = build_output_schema(&output, None, false).unwrap();
        assert!(schema.get("anyOf").is_none());
        assert_eq!(schema["properties"]["value"]["type"], "string");
    }

    #[test]
    fn description_copied_to_schema_property() {
        let params = json!({
            "query": {
                "type": "string",
                "optional": false,
                "description": "Search query string"
            }
        });

        let schema = build_schema_from_params(&params, None, false);
        assert_eq!(
            schema["properties"]["query"]["description"],
            "Search query string"
        );
    }

    #[test]
    fn strict_mode_nested_object_optional() {
        let params = json!({
            "config": {
                "type": {
                    "type": "object",
                    "properties": {
                        "timeout": { "type": "number", "optional": true }
                    }
                },
                "optional": true
            }
        });

        let schema = build_schema_from_params(&params, None, true);
        // Parent is optional → nullable
        assert_eq!(
            schema["properties"]["config"]["type"],
            json!(["object", "null"])
        );
        // Nested object is strict → all its fields required
        let nested = &schema["properties"]["config"]["properties"]["timeout"];
        assert_eq!(nested["type"], json!(["number", "null"]));
    }

    #[test]
    fn union_optional_strict() {
        let params = json!({
            "priority": {
                "type": { "type": "union", "options": ["low", "medium", "high"] },
                "optional": true
            }
        });

        let schema = build_schema_from_params(&params, None, true);
        let prop = &schema["properties"]["priority"];
        assert_eq!(prop["type"], json!(["string", "null"]));
        assert_eq!(prop["enum"], json!(["low", "medium", "high"]));
    }
}
