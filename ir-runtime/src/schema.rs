use serde_json::Value;

pub fn format_schema_yaml(schema: &Value, indent_level: usize) -> String {
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
            let field_type = def["type"].as_str().unwrap_or("any");

            let mut line = format!("{}{}: {}", indent, name_tag, field_type);
            if let Some(desc) = def["description"].as_str() {
                line.push_str(" // ");
                line.push_str(desc);
            }
            lines.push(line);
        }
    }
    lines.join("\n")
}

pub fn format_schema(schema: &Value) -> String {
    if let Some(obj) = schema.as_object() {
        let mut fields = Vec::new();
        for (name, def) in obj {
            let is_optional = def["optional"].as_bool().unwrap_or(false);
            let name_tag = if is_optional {
                format!("{}?", name)
            } else {
                name.clone()
            };
            let field_type = def["type"].as_str().unwrap_or("any");

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
