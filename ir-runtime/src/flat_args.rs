use crate::schema;
use crate::types::TypeDefinition;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatFieldSpec {
    pub alias: String,
    pub path: Vec<String>,
    pub type_repr: String,
    pub optional: bool,
    pub description: Option<String>,
}

pub fn flatten_named_field_specs(
    schema_value: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<FlatFieldSpec> {
    let mut specs = Vec::new();
    let mut seen_aliases = HashSet::new();

    if let Some(obj) = schema_value.as_object() {
        let mut keys: Vec<_> = obj.keys().cloned().collect();
        keys.sort();

        for key in keys {
            if let Some(def) = obj.get(&key) {
                collect_specs_for_field(
                    vec![key.clone()],
                    vec![key],
                    def,
                    false,
                    types,
                    &mut seen_aliases,
                    &mut specs,
                );
            }
        }
    }

    specs
}

pub fn flatten_helper_input_specs(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Vec<FlatFieldSpec> {
    let Some(input) = input_ir else {
        return vec![default_text_input_spec()];
    };

    if input.is_null() {
        return vec![default_text_input_spec()];
    }

    if input.get("kind").and_then(|v| v.as_str()) == Some("properties") {
        if let Some(fields) = input.get("fields") {
            return flatten_named_field_specs(fields, types);
        }
    }

    if input.get("kind").and_then(|v| v.as_str()) == Some("direct") {
        if let Some(ty) = input.get("type") {
            if let Some(props) = resolve_nested_properties(ty, types) {
                return flatten_named_field_specs(&Value::Object(props), types);
            }

            return vec![FlatFieldSpec {
                alias: "input".to_string(),
                path: vec!["input".to_string()],
                type_repr: schema::format_type_value(ty, types),
                optional: false,
                description: input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }];
        }
    }

    if let Some(props) = resolve_nested_properties(input, types) {
        return flatten_named_field_specs(&Value::Object(props), types);
    }

    if input.as_object().is_some() {
        let filtered: Map<String, Value> = input
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__") && *key != "kind")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        if !filtered.is_empty() {
            return flatten_named_field_specs(&Value::Object(filtered), types);
        }
    }

    Vec::new()
}

fn default_text_input_spec() -> FlatFieldSpec {
    FlatFieldSpec {
        alias: "input".to_string(),
        path: vec!["input".to_string()],
        type_repr: "string".to_string(),
        optional: false,
        description: None,
    }
}

pub fn alias_map_from_specs(specs: &[FlatFieldSpec]) -> HashMap<String, Vec<String>> {
    specs
        .iter()
        .map(|spec| (spec.alias.clone(), spec.path.clone()))
        .collect()
}

pub fn flatten_example_object(example: &Value, specs: &[FlatFieldSpec]) -> Vec<(String, Value)> {
    let mut flattened = Vec::new();

    for spec in specs {
        if let Some(value) = get_value_at_path(example, &spec.path) {
            flattened.push((spec.alias.clone(), value.clone()));
        }
    }

    flattened
}

pub fn flatten_output_specs(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> HashMap<String, Vec<FlatFieldSpec>> {
    let mut schemas = HashMap::new();

    if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
        for (schema_name, schema_value) in variants {
            let specs = flatten_named_field_specs(
                &Value::Object(normalize_output_fields(schema_value)),
                types,
            );
            if !specs.is_empty() {
                schemas.insert(schema_name.clone(), specs);
            }
        }
        return schemas;
    }

    let specs = flatten_named_field_specs(&Value::Object(normalize_output_fields(output)), types);
    if !specs.is_empty() {
        schemas.insert("Output".to_string(), specs);
    }

    schemas
}

pub fn unflatten_object(flat: &Value, alias_map: &HashMap<String, Vec<String>>) -> Value {
    let Some(obj) = flat.as_object() else {
        return flat.clone();
    };

    let mut rebuilt = Map::new();

    for (key, value) in obj {
        let path = alias_map
            .get(key)
            .cloned()
            .unwrap_or_else(|| vec![key.clone()]);
        insert_value_at_path(&mut rebuilt, &path, value.clone());
    }

    Value::Object(rebuilt)
}

fn collect_specs_for_field(
    alias_segments: Vec<String>,
    path_segments: Vec<String>,
    def: &Value,
    inherited_optional: bool,
    types: Option<&HashMap<String, TypeDefinition>>,
    seen_aliases: &mut HashSet<String>,
    specs: &mut Vec<FlatFieldSpec>,
) {
    let optional = inherited_optional
        || def
            .get("optional")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

    if let Some(props) = resolve_nested_properties(def, types) {
        let mut keys: Vec<_> = props.keys().cloned().collect();
        keys.sort();

        for key in keys {
            if let Some(child_def) = props.get(&key) {
                let mut next_alias = alias_segments.clone();
                next_alias.push(key.clone());
                let mut next_path = path_segments.clone();
                next_path.push(key);

                collect_specs_for_field(
                    next_alias,
                    next_path,
                    child_def,
                    optional,
                    types,
                    seen_aliases,
                    specs,
                );
            }
        }

        return;
    }

    let alias = unique_alias(alias_segments.join("_"), seen_aliases);
    specs.push(FlatFieldSpec {
        alias,
        path: path_segments,
        type_repr: schema::format_type_value(def, types),
        optional,
        description: def
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    });
}

fn unique_alias(base: String, seen_aliases: &mut HashSet<String>) -> String {
    if seen_aliases.insert(base.clone()) {
        return base;
    }

    let mut index = 2usize;
    loop {
        let candidate = format!("{}__{}", base, index);
        if seen_aliases.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

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

fn normalize_output_fields(output: &Value) -> Map<String, Value> {
    if let Some(obj) = output.as_object() {
        if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
            return props.clone();
        }

        if let Some(inner) = obj.get("type") {
            let nested = normalize_output_fields(inner);
            if !nested.is_empty() {
                return nested;
            }
        }

        return obj
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
    }

    Map::new()
}

fn get_value_at_path<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = value;

    for segment in path {
        current = current.as_object()?.get(segment)?;
    }

    Some(current)
}

fn insert_value_at_path(target: &mut Map<String, Value>, path: &[String], value: Value) {
    if path.is_empty() {
        return;
    }

    if path.len() == 1 {
        target.insert(path[0].clone(), value);
        return;
    }

    let entry = target
        .entry(path[0].clone())
        .or_insert_with(|| Value::Object(Map::new()));

    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }

    if let Value::Object(obj) = entry {
        insert_value_at_path(obj, &path[1..], value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flatten_helper_input_specs_unpacks_direct_object() {
        let specs = flatten_helper_input_specs(
            Some(&json!({
                "kind": "direct",
                "type": {
                    "type": "object",
                    "properties": {
                        "analysis_request": {
                            "type": "string",
                            "optional": false
                        }
                    }
                }
            })),
            None,
        );

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].alias, "analysis_request");
    }

    #[test]
    fn test_flatten_helper_input_specs_defaults_null_to_text_input() {
        let specs = flatten_helper_input_specs(None, None);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].alias, "input");
        assert_eq!(specs[0].path, vec!["input"]);
        assert_eq!(specs[0].type_repr, "string");
    }

    #[test]
    fn test_flatten_output_specs_uses_flat_aliases_for_nested_output() {
        let specs = flatten_output_specs(
            &json!({
                "success": { "type": "boolean", "optional": false },
                "profile": {
                    "type": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "optional": false },
                            "contact": {
                                "type": {
                                    "type": "object",
                                    "properties": {
                                        "email": { "type": "string", "optional": false }
                                    }
                                },
                                "optional": false
                            }
                        }
                    },
                    "optional": false
                }
            }),
            None,
        );

        let output_specs = specs.get("Output").expect("missing Output specs");
        assert!(output_specs.iter().any(|spec| spec.alias == "profile_name"));
        assert!(
            output_specs
                .iter()
                .any(|spec| spec.alias == "profile_contact_email")
        );
    }
}
