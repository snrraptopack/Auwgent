use quew_runtime::value::Value;
use quew_macros::quew_builtin;
/// Parse a JSON string into a runtime `Value`.
///
/// At the DSL level the return type is declared as `string` because the
/// type system does not yet have an `any` type.  At runtime the result
/// is a real `Value::Object`, `Value::Array`, `Value::Number`, etc.
#[quew_builtin(
    id = "std.json.parse",
    decl = r#"!@@function json_parse(value: string): string"#,
)]
pub fn json_parse(value: &str) -> Value {
    match serde_json::from_str(value) {
        Ok(json) => serde_to_value(json),
        Err(e) => Value::String(format!("parse error: {}", e)),
    }
}

/// Serialize a runtime `Value` into a JSON string.
#[quew_builtin(
    id = "std.json.stringify",
    decl = r#"!@@function json_stringify(value: string): string"#,
)]
pub fn json_stringify(value: &Value) -> String {
    match value_to_serde(value) {
        Ok(json) => json.to_string(),
        Err(e) => format!("stringify error: {}", e),
    }
}

/// Walk a dot-separated path through a JSON value and return the element at
/// that path as a real runtime value (`any`). Missing paths yield null.
#[quew_builtin(
    id = "std.json.get",
    decl = r#"!@@function json_get(value: any, path: string): any"#,
)]
pub fn json_get(value: &Value, path: &str) -> Value {
    let mut current = value;
    for segment in path.split('.') {
        match current {
            Value::Object(map) => {
                current = match map.get(segment) {
                    Some(v) => v,
                    None => return Value::Null,
                };
            }
            Value::Array(arr) => {
                let idx: usize = match segment.parse() {
                    Ok(i) => i,
                    Err(_) => return Value::Null,
                };
                current = match arr.get(idx) {
                    Some(v) => v,
                    None => return Value::Null,
                };
            }
            _ => return Value::Null,
        }
    }
    current.clone()
}

// ── Helpers: serde_json <-> Value ───────────────────────────────────────────

fn serde_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(serde_to_value).collect()),
        serde_json::Value::Object(map) => {
            let mut result = indexmap::IndexMap::new();
            for (k, v) in map {
                result.insert(k, serde_to_value(v));
            }
            Value::Object(result)
        }
    }
}

fn value_to_serde(v: &Value) -> Result<serde_json::Value, String> {
    match v {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Number(n) => Ok(serde_json::Value::Number((*n).into())),
        Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .ok_or_else(|| "cannot serialize NaN/Inf float".to_string())
        }
        Value::String(s) => Ok(serde_json::Value::String(s.clone())),
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(value_to_serde(item)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), value_to_serde(v)?);
            }
            Ok(serde_json::Value::Object(out))
        }
    }
}
