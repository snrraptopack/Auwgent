use quew_macros::quew_builtin;
use quew_runtime::value::Value;

#[quew_builtin(
    id = "std.io.print",
    decl = r#"!@@function print<T>(value: T): null"#,
)]
pub fn print_value(value: &Value) -> Value {
    println!("{}", value);
    Value::Null
}
