use crate::types::{AgentIR, TypeDefinition};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn build_native_tools(ir: &AgentIR) -> Vec<Value> {
    let mut native_tools = Vec::new();
    
    // 1. Add standard tools
    for tool in &ir.tools {
        let parameters = build_schema_from_params(&tool.params.0, ir.types.as_ref());
        native_tools.push(create_function_schema(&tool.name, &tool.description, parameters));
    }
    
    // 2. Add workflows
    for workflow in &ir.workflows {
        let parameters = build_schema_from_params(&workflow.params.0, ir.types.as_ref());
        native_tools.push(create_function_schema(&workflow.name, &workflow.description, parameters));
    }

    // 3. Add helpers
    for helper in &ir.helpers {
        let empty_input = json!({});
        let input_params = helper.input.as_ref().map(|v| &v.0).unwrap_or(&empty_input);
        let parameters = build_schema_from_params(input_params, ir.types.as_ref());
        native_tools.push(create_function_schema(&helper.name, &helper.description, parameters));
    }

    native_tools
}

fn create_function_schema(name: &str, description: &Option<String>, parameters: Value) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description.clone().unwrap_or_default(),
            "parameters": parameters
        }
    })
}

fn build_schema_from_params(
    params: &Value, 
    types: Option<&HashMap<String, TypeDefinition>>
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    if let Some(params_obj) = params.as_object() {
        for (key, type_property) in params_obj {
            let mut schema_prop = resolve_type_value(
                type_property.get("type").unwrap_or(&Value::Null), 
                types
            );

            if let Some(desc) = type_property.get("description").and_then(|v| v.as_str()) {
                if let Some(obj) = schema_prop.as_object_mut() {
                    obj.insert("description".to_string(), json!(desc));
                }
            }

            let is_optional = type_property.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);
            if !is_optional {
                required.push(json!(key));
            }

            properties.insert(key.clone(), schema_prop);
        }
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn resolve_type_value(
    type_val: &Value,
    types: Option<&HashMap<String, TypeDefinition>>
) -> Value {
    match type_val {
        Value::String(s) => {
            if s == "any" {
                json!({}) 
            } else {
                json!({ "type": s })
            }
        }
        Value::Object(obj) => {
            let t = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match t {
                "array" => {
                    let items = obj.get("items").unwrap_or(&Value::Null);
                    json!({
                        "type": "array",
                        "items": resolve_type_value(items, types)
                    })
                }
                "literal" => {
                    let val = obj.get("value").unwrap_or(&Value::Null);
                    json!({
                        "type": "string",
                        "enum": [val]
                    })
                }
                "ref" => {
                    if let Some(target) = obj.get("target").and_then(|v| v.as_str()) {
                        if let Some(type_map) = types {
                            if let Some(custom_type) = type_map.get(target) {
                                let mut props_json = serde_json::Map::new();
                                for (prop_name, prop_def) in &custom_type.properties {
                                    let prop_val = serde_json::to_value(prop_def).unwrap_or(json!({}));
                                    props_json.insert(prop_name.clone(), prop_val);
                                }
                                return build_schema_from_params(&Value::Object(props_json), types);
                            }
                        }
                    }
                    json!({ "type": "object" })
                }
                _ => json!({ "type": "object" })
            }
        }
        _ => json!({ "type": "string" }) 
    }
}
