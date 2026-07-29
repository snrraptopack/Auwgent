use indexmap::IndexMap;

use super::utils::compile_source;
use super::*;

#[test]
fn execute_literal_return_function() {
    let (interner, ir) = compile_source(
        r#"
function answer(): number { return 42 }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:answer", Value::Null).unwrap();
    assert_eq!(result, Value::Number(42));
}

#[test]
fn execute_identity_function() {
    let (interner, ir) = compile_source(
        r#"
function identity(x: number): number { return x }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("x".to_string(), Value::Number(7));
    let result = exec.run("function:identity", Value::Object(input)).unwrap();
    assert_eq!(result, Value::Number(7));
}

#[test]
fn execute_arithmetic_function() {
    let (interner, ir) = compile_source(
        r#"
function double(x: number): number { return x + x }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("x".to_string(), Value::Number(5));
    let result = exec.run("function:double", Value::Object(input)).unwrap();
    assert_eq!(result, Value::Number(10));
}

#[test]
fn execute_function_calling_another_function() {
    let (interner, ir) = compile_source(
        r#"
function add(a: number, b: number): number { return a + b }
function add_three(x: number): number { return add(x, 3) }
agent Main(input: number) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("x".to_string(), Value::Number(4));
    let result = exec
        .run("function:add_three", Value::Object(input))
        .unwrap();
    assert_eq!(result, Value::Number(7));
}

#[test]
fn execute_extension_method_call() {
    let (interner, ir) = compile_source(
        r#"
extend string {
    function withPrefix(prefix: string): string { return prefix + self }
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("self".to_string(), Value::String("world".into()));
    input.insert("prefix".to_string(), Value::String("Hello, ".into()));
    let result = exec
        .run("extension:string:withPrefix", Value::Object(input))
        .unwrap();
    assert_eq!(result, Value::String("Hello, world".into()));
}
