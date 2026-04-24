use crate::types::TypeDefinition;
use serde_json::Value;
use std::collections::HashMap;

pub fn format_output_schema_blocks(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let mut entries: Vec<(String, Vec<crate::flat_args::FlatFieldSpec>)> =
        crate::flat_args::flatten_output_specs(output, types)
            .into_iter()
            .collect();

    if entries.is_empty() {
        return "{}".to_string();
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    entries
        .into_iter()
        .map(|(schema_name, specs)| {
            format!(
                "[schema: {}]\n{}\n[/schema]",
                schema_name,
                format_flat_schema_fields(&specs)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_flat_schema_fields(specs: &[crate::flat_args::FlatFieldSpec]) -> String {
    specs
        .iter()
        .map(|spec| {
            let mut line = if spec.optional {
                format!("{}?: {}", spec.alias, spec.type_repr)
            } else {
                format!("{}: {}", spec.alias, spec.type_repr)
            };

            if let Some(desc) = &spec.description {
                line.push_str(" // ");
                line.push_str(desc);
            }

            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_schema(schema: &Value, types: Option<&HashMap<String, TypeDefinition>>) -> String {
    if let Some(obj) = schema.as_object() {
        let mut fields = Vec::new();
        for (name, def) in obj {
            let is_optional = def["optional"].as_bool().unwrap_or(false);
            let name_tag = if is_optional {
                format!("{}?", name)
            } else {
                name.clone()
            };
            let field_type = format_type_value(def, types);

            let mut field_str = format!("{}:{}", name_tag, field_type);
            if let Some(desc) = def["description"].as_str() {
                field_str.push_str(" // ");
                field_str.push_str(desc);
            }
            fields.push(field_str);
        }
        format!("schema: {{ {} }}", fields.join(", "))
    } else {
        "{}".to_string()
    }
}



pub fn format_type_value(def: &Value, types: Option<&HashMap<String, TypeDefinition>>) -> String {
    if let Some(obj) = def.as_object() {
        if let Some(type_str) = obj.get("type").and_then(|v| v.as_str()) {
            if type_str == "typeRef" {
                return format_type(def, types);
            }
        }
        if let Some(type_val) = obj.get("type") {
            return format_type(type_val, types);
        }
    }
    format_type(def, types)
}

fn format_type(type_val: &Value, types: Option<&HashMap<String, TypeDefinition>>) -> String {
    let mut visited = std::collections::HashSet::new();
    format_type_internal(type_val, types, &mut visited)
}

fn format_type_internal(
    type_val: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    visited: &mut std::collections::HashSet<String>,
) -> String {
    if let Some(s) = type_val.as_str() {
        return normalize_type_name(s);
    }

    if let Some(obj) = type_val.as_object() {
        if let Some(type_tag) = obj.get("type").and_then(|v| v.as_str()) {
            match type_tag {
                "array" => {
                    if let Some(items) = obj.get("items") {
                        return format!("{}[]", format_type_internal(items, types, visited));
                    }
                }
                "union" => {
                    if let Some(options) = obj.get("options").and_then(|v| v.as_array()) {
                        let rendered: Vec<String> = options
                            .iter()
                            .map(|v| format_type_internal(v, types, visited))
                            .collect();
                        return rendered.join(" | ");
                    }
                }
                "object" => {
                    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
                        let rendered: Vec<String> = props
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, format_type_internal(v, types, visited)))
                            .collect();
                        return format!("{{ {} }}", rendered.join(", "));
                    }
                }
                "typeRef" => {
                    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                        // Expand the object directly to reveal array item schemas
                        if visited.insert(name.to_string()) {
                            if let Some(types_map) = types {
                                if let Some(custom_type) = types_map.get(name) {
                                    if let Ok(props_val) = serde_json::to_value(&custom_type.properties) {
                                        if let Some(props) = props_val.as_object() {
                                            let rendered: Vec<String> = props
                                                .iter()
                                                .map(|(k, v)| format!("{}: {}", k, format_type_internal(v, types, visited)))
                                                .collect();
                                            visited.remove(name);
                                            return format!("{{ {} }}", rendered.join(", "));
                                        }
                                    }
                                }
                            }
                            visited.remove(name);
                        }
                        return name.to_string();
                    }
                }
                other => return normalize_type_name(other),
            }
        }

        if let Some(inner) = obj.get("type") {
            return format_type_internal(inner, types, visited);
        }
    }

    "any".to_string()
}

fn normalize_type_name(t: &str) -> String {
    match t.to_lowercase().as_str() {
        "int" | "float" | "number" => "number".to_string(),
        "bool" | "boolean" => "boolean".to_string(),
        "string" => "string".to_string(),
        other => other.to_string(),
    }
}
