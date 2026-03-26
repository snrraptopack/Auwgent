use crate::types::TypeDefinition;
use serde_json::Value;
use std::collections::HashMap;

pub fn format_schema_yaml(
    schema: &Value,
    indent_level: usize,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let indent = " ".repeat(indent_level);
    let mut lines = Vec::new();
    if let Some(obj) = schema.as_object() {
        for (name, def) in obj {
            let is_optional = def["optional"].as_bool().unwrap_or(false);
            let name_tag = if is_optional {
                format!("{}?", name)
            } else {
                name.clone()
            };

            let mut resolved_props: Option<Value> = None;

            // 1. Is the type an object describing an inline schema or typeRef?
            if let Some(t_obj) = def.get("type").and_then(|t| t.as_object()) {
                if t_obj.get("type").and_then(|t| t.as_str()) == Some("object") {
                    if let Some(p) = t_obj.get("properties") {
                        resolved_props = Some(p.clone());
                    }
                } else if t_obj.get("type").and_then(|t| t.as_str()) == Some("typeRef") {
                    if let Some(ref_name) = t_obj.get("name").and_then(|n| n.as_str()) {
                        if let Some(types_map) = types {
                            if let Some(custom_type) = types_map.get(ref_name) {
                                if let Ok(val) = serde_json::to_value(&custom_type.properties) {
                                    resolved_props = Some(val);
                                }
                            }
                        }
                    }
                }
            }

            // 2. Is the property itself a flat typeRef? (e.g., {"type": "typeRef", "name": "..."})
            if resolved_props.is_none() {
                if def.get("type").and_then(|t| t.as_str()) == Some("typeRef") {
                    if let Some(ref_name) = def.get("name").and_then(|n| n.as_str()) {
                        if let Some(types_map) = types {
                            if let Some(custom_type) = types_map.get(ref_name) {
                                if let Ok(val) = serde_json::to_value(&custom_type.properties) {
                                    resolved_props = Some(val);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(props) = resolved_props {
                let mut line = format!("{}{}:", indent, name_tag);
                if let Some(desc) = def.get("description").and_then(|d| d.as_str()) {
                    line.push_str(" // ");
                    line.push_str(desc);
                }
                lines.push(line);
                let nested = format_schema_yaml(&props, indent_level + 2, types);
                if !nested.is_empty() {
                    lines.push(nested);
                }
            } else {
                let field_type = format_type_value(def, types);
                let mut line = format!("{}{}: {}", indent, name_tag, field_type);
                if let Some(desc) = def.get("description").and_then(|d| d.as_str()) {
                    line.push_str(" // ");
                    line.push_str(desc);
                }
                lines.push(line);
            }
        }
    }
    lines.join("\n")
}

pub fn format_schema_function(
    schema: &Value,
    indent_level: usize,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> String {
    let indent = " ".repeat(indent_level);
    let mut lines = Vec::new();
    if let Some(obj) = schema.as_object() {
        for (name, def) in obj {
            let is_optional = def["optional"].as_bool().unwrap_or(false);
            let name_tag = if is_optional {
                format!("{}?", name)
            } else {
                name.clone()
            };

            let field_type = format_type_value(def, types);

            // Format as a Function Composition field assignment
            let mut line = format!("{}{} = /* {} */", indent, name_tag, field_type);

            if let Some(desc) = def.get("description").and_then(|d| d.as_str()) {
                line.push_str(" // ");
                line.push_str(desc);
            }
            lines.push(line);
        }
    }
    lines.join("\n")
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

pub fn format_type_definitions_yaml(
    types: &HashMap<String, TypeDefinition>,
    indent_level: usize,
) -> String {
    let indent = " ".repeat(indent_level);
    let mut lines = Vec::new();
    for (name, def) in types {
        lines.push(format!("{}{}:", indent, name));
        for (prop_name, prop) in &def.properties {
            let name_tag = if prop.optional {
                format!("{}?", prop_name)
            } else {
                prop_name.to_string()
            };
            let field_type = format_type(&prop.type_value.0, Some(types));
            let mut line = format!("{}  {}: {}", indent, name_tag, field_type);
            if let Some(desc) = &prop.description {
                line.push_str(" // ");
                line.push_str(desc);
            }
            lines.push(line);
        }
    }
    lines.join("\n")
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
    if let Some(s) = type_val.as_str() {
        return normalize_type_name(s);
    }

    if let Some(obj) = type_val.as_object() {
        if let Some(type_tag) = obj.get("type").and_then(|v| v.as_str()) {
            match type_tag {
                "array" => {
                    if let Some(items) = obj.get("items") {
                        return format!("{}[]", format_type(items, types));
                    }
                }
                "union" => {
                    if let Some(options) = obj.get("options").and_then(|v| v.as_array()) {
                        let rendered: Vec<String> =
                            options.iter().map(|v| format_type(v, types)).collect();
                        return rendered.join(" | ");
                    }
                }
                "object" => {
                    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
                        let rendered: Vec<String> = props
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, format_type(v, types)))
                            .collect();
                        return format!("{{ {} }}", rendered.join(", "));
                    }
                }
                "typeRef" => {
                    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                        // For pure string formatting (like in array[] signatures) we just put the name.
                        // Full object expansion happens in format_schema_yaml where we can indent.
                        return name.to_string();
                    }
                }
                other => return normalize_type_name(other),
            }
        }

        if let Some(inner) = obj.get("type") {
            return format_type(inner, types);
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
