use indexmap::IndexMap;

use super::*;
use super::utils::compile_source;

#[test]
fn execute_string_interpolation() {
    let (interner, ir) = compile_source(
        r#"
function greet(name: string): string {
    return "hello {name}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("name".to_string(), Value::String("world".into()));
    let result = exec.run("function:greet", Value::Object(input)).unwrap();
    assert_eq!(result, Value::String("hello world".into()));
}

#[test]
fn execute_string_interpolation_multiple_segments() {
    let (interner, ir) = compile_source(
        r#"
function format(a: string, b: string): string {
    return "{a} and {b}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("a".to_string(), Value::String("hello".into()));
    input.insert("b".to_string(), Value::String("world".into()));
    let result = exec.run("function:format", Value::Object(input)).unwrap();
    assert_eq!(result, Value::String("hello and world".into()));
}

#[test]
fn execute_string_interpolation_with_escaped_braces() {
    let (interner, ir) = compile_source(
        r#"
function braces(x: string): string {
    return "{{literal}} {x}"
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let mut input = IndexMap::new();
    input.insert("x".to_string(), Value::String("value".into()));
    let result = exec.run("function:braces", Value::Object(input)).unwrap();
    assert_eq!(result, Value::String("{literal} value".into()));
}
