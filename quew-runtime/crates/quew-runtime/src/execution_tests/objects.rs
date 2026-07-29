use indexmap::IndexMap;

use super::utils::compile_source;
use super::*;

#[test]
fn execute_typed_object_literal() {
    let (interner, ir) = compile_source(
        r#"
type Person = {
    name: string
    age: number
}
function test(): Person {
    let obj: Person = { name: "Alice", age: 30 }
    return obj
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    let mut expected = IndexMap::new();
    expected.insert("name".to_string(), Value::String("Alice".into()));
    expected.insert("age".to_string(), Value::Number(30));
    assert_eq!(result, Value::Object(expected));
}

#[test]
fn execute_object_literal_field_access() {
    let (interner, ir) = compile_source(
        r#"
type Person = {
    name: string
    age: number
}
function test(): string {
    let obj: Person = { name: "Bob", age: 25 }
    return obj.name
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::String("Bob".into()));
}
