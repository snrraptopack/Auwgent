use quew_macros::quew_builtin;
use quew_runtime::value::Value;

#[quew_builtin(
    id = "std.number.abs",
    decl = r#"!@@function abs(value: number | float): number | float"#,
)]
pub fn number_abs(value: &Value) -> Value {
    match value {
        Value::Number(n) => Value::Number(n.abs()),
        Value::Float(f) => Value::Float(f.abs()),
        _ => Value::Null,
    }
}

#[quew_builtin(
    id = "std.number.clamp",
    decl = r#"!@@function clamp(value: number, min: number, max: number): number"#,
)]
pub fn number_clamp(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}
