use quew_macros::quew_builtin;
use quew_runtime::value::Value;

#[quew_builtin(
    id = "std.array.len",
    decl = r#"!@@function array_len<T>(array: T[]): number"#,
)]
pub fn array_len(array: &Value) -> i64 {
    array.as_array().map(|a| a.len() as i64).unwrap_or(0)
}

#[quew_builtin(
    id = "std.array.get",
    decl = r#"!@@function array_get<T>(array: T[], index: number): T?"#,
)]
pub fn array_get(array: &Value, index: i64) -> Value {
    array
        .as_array()
        .and_then(|a| {
            if index < 0 {
                None
            } else {
                a.get(index as usize).cloned()
            }
        })
        .unwrap_or(Value::Null)
}

#[quew_builtin(
    id = "std.array.push",
    decl = r#"!@@function array_push<T>(array: T[], item: T): T[]"#,
)]
pub fn array_push(array: &Value, item: &Value) -> Value {
    let mut new_array = array.as_array().map(|a| a.to_vec()).unwrap_or_default();
    new_array.push(item.clone());
    Value::Array(new_array)
}

#[quew_builtin(
    id = "std.array.pop",
    decl = r#"!@@function array_pop<T>(array: T[]): T?"#,
)]
pub fn array_pop(array: &Value) -> Value {
    array
        .as_array()
        .and_then(|a| a.last().cloned())
        .unwrap_or(Value::Null)
}
