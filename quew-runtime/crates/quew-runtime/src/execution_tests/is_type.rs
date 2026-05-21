use super::*;
use super::utils::compile_source;

#[test]
fn execute_is_string_true() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return "hello" is string
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn execute_is_string_false() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return 42 is string
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn execute_is_number_true() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return 42 is number
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn execute_is_bool_true() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return true is bool
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn execute_is_null_true() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return null is null
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn execute_is_array_true() {
    let (interner, ir) = compile_source(
        r#"
function test(): bool {
    return [1, 2] is array
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn execute_is_record_true() {
    let (interner, ir) = compile_source(
        r#"
type Person = {
    name: string
}
function test(): bool {
    let p: Person = { name: "Alice" }
    return p is Person
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Bool(true));
}
