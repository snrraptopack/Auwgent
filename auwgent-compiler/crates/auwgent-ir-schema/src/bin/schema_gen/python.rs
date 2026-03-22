use auwgent_ir_schema::AgentIR;
use schemars::schema_for;
use std::path::PathBuf;

pub fn generate(out_dir: &PathBuf) {
    let root = schema_for!(AgentIR);
    let py = emit_python(&root);
    
    let path = out_dir.join("ir_schema.py");
    std::fs::write(&path, py).expect("Failed to write ir_schema.py");
}

fn emit_python(root: &schemars::schema::RootSchema) -> String {
    let mut lines = vec![
        "# Auto-generated from auwgent-ir-schema — do not edit manually.".to_string(),
        "# Source of truth: auwgent-compiler/crates/auwgent-ir-schema/src/lib.rs".to_string(),
        "from __future__ import annotations".to_string(),
        "from typing import Any, Dict, List, Literal, Optional, Union".to_string(),
        "try:".to_string(),
        "    from typing import TypedDict, NotRequired".to_string(),
        "except ImportError:".to_string(),
        "    from typing_extensions import TypedDict, NotRequired".to_string(),
        String::new(),
    ];

    // Emit all definitions in the schema
    let defs = &root.definitions;
    for (name, schema) in defs {
        let class = emit_class(name, schema, defs);
        if !class.is_empty() {
            lines.push(class);
            lines.push(String::new());
        }
    }

    // Emit the root type itself
    let root_class = emit_class("AgentIR", &root.schema.clone().into(), defs);
    if !root_class.is_empty() {
        lines.push(root_class);
    }

    lines.join("\n")
}

fn emit_class(
    name: &str,
    schema: &schemars::schema::Schema,
    defs: &schemars::Map<String, schemars::schema::Schema>,
) -> String {
    let schema = match schema {
        schemars::schema::Schema::Object(obj) => obj,
        _ => return String::new(),
    };

    // Skip string enums - emit as aliases
    if let Some(enum_values) = &schema.enum_values {
        let literals: Vec<String> = enum_values
            .iter()
            .filter_map(|v| v.as_str().map(|s| format!("Literal[\"{s}\"]")))
            .collect();
        if !literals.is_empty() {
            return format!("{name} = Union[{}]", literals.join(", "));
        }
    }

    // Tagged unions
    let any_of = schema.subschemas.as_ref().and_then(|s| s.any_of.as_ref().or(s.one_of.as_ref()));
    if let Some(variants) = any_of {
        let variant_types: Vec<String> = variants
            .iter()
            .filter_map(|v| resolve_ref_name(v, defs))
            .collect();
        if !variant_types.is_empty() {
            return format!("{name} = Union[{}]", variant_types.join(", "));
        }
    }

    // Regular Object - emit TypedDict
    let properties = schema.object.as_ref().map(|o| &o.properties);
    let required = schema.object.as_ref().map(|o| o.required.clone()).unwrap_or_default();

    let mut prop_lines = Vec::new();
    if let Some(props) = properties {
        for (field_name, field_schema) in props {
            let py_type = schema_to_python_type(field_schema, defs);
            let is_required = required.contains(field_name);
            if is_required {
                prop_lines.push(format!("    {field_name}: {py_type}"));
            } else {
                prop_lines.push(format!("    {field_name}: NotRequired[{py_type}]"));
            }
        }
    }

    if prop_lines.is_empty() {
        prop_lines.push("    pass".to_string());
    }

    format!("class {name}(TypedDict, total=False):\n{}", prop_lines.join("\n"))
}

fn schema_to_python_type(
    schema: &schemars::schema::Schema,
    defs: &schemars::Map<String, schemars::schema::Schema>,
) -> String {
    match schema {
        schemars::schema::Schema::Bool(true) => "Any".to_string(),
        schemars::schema::Schema::Bool(false) => "Any".to_string(),
        schemars::schema::Schema::Object(obj) => {
            if let Some(ref_name) = obj.reference.as_deref().and_then(|r| r.strip_prefix("#/definitions/")) {
                return ref_name.to_string();
            }

            if let Some(instance_type) = &obj.instance_type {
                match instance_type {
                    schemars::schema::SingleOrVec::Single(t) => {
                        return primitive_type_to_python(t);
                    }
                    schemars::schema::SingleOrVec::Vec(types) => {
                        let non_null: Vec<_> = types
                            .iter()
                            .filter(|t| !matches!(t, schemars::schema::InstanceType::Null))
                            .collect();
                        if non_null.len() == 1 {
                            let py = primitive_type_to_python(non_null[0]);
                            let has_null = types.len() != non_null.len();
                            if has_null {
                                return format!("Optional[{py}]");
                            }
                            return py;
                        }
                        return "Any".to_string();
                    }
                }
            }

            if let Some(items) = obj.array.as_ref().and_then(|a| a.items.as_ref()) {
                if let schemars::schema::SingleOrVec::Single(item_schema) = items {
                    let item_type = schema_to_python_type(item_schema, defs);
                    return format!("List[{item_type}]");
                }
            }

            let any_of = obj.subschemas.as_ref().and_then(|s| s.any_of.as_ref().or(s.one_of.as_ref()));
            if let Some(variants) = any_of {
                let non_null: Vec<String> = variants
                    .iter()
                    .filter(|v| !is_null_schema(v))
                    .map(|v| schema_to_python_type(v, defs))
                    .collect();
                let has_null = variants.len() != non_null.len();
                if non_null.len() == 1 && has_null {
                    return format!("Optional[{}]", non_null[0]);
                }
                if non_null.len() > 1 {
                    let union = format!("Union[{}]", non_null.join(", "));
                    if has_null {
                        return format!("Optional[{union}]");
                    }
                    return union;
                }
            }

            "Any".to_string()
        }
    }
}

fn primitive_type_to_python(t: &schemars::schema::InstanceType) -> String {
    match t {
        schemars::schema::InstanceType::String => "str".to_string(),
        schemars::schema::InstanceType::Number | schemars::schema::InstanceType::Integer => "float".to_string(),
        schemars::schema::InstanceType::Boolean => "bool".to_string(),
        schemars::schema::InstanceType::Array => "List[Any]".to_string(),
        schemars::schema::InstanceType::Object => "Dict[str, Any]".to_string(),
        schemars::schema::InstanceType::Null => "None".to_string(),
    }
}

fn is_null_schema(schema: &schemars::schema::Schema) -> bool {
    match schema {
        schemars::schema::Schema::Object(obj) => {
            matches!(
                &obj.instance_type,
                Some(schemars::schema::SingleOrVec::Single(t)) if matches!(t.as_ref(), schemars::schema::InstanceType::Null)
            )
        }
        _ => false,
    }
}

fn resolve_ref_name(
    schema: &schemars::schema::Schema,
    _defs: &schemars::Map<String, schemars::schema::Schema>,
) -> Option<String> {
    match schema {
        schemars::schema::Schema::Object(obj) => {
            obj.reference.as_deref()
                .and_then(|r| r.strip_prefix("#/definitions/"))
                .map(|s| s.to_string())
        }
        _ => None,
    }
}
