use super::*;
use super::utils::compile_source_with_prelude;

#[test]
fn execute_mutable_assignment() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    count = 5
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(5));
}

#[test]
fn execute_mutable_assignment_with_expression() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 10
    count = count + 1
    return count
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(11));
}

#[test]
fn execute_assignment_inside_branch_then_taken() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let x = 0
    if true {
        x = 42
    }
    return x
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(42));
}

#[test]
fn execute_assignment_inside_branch_else_taken() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let x = 0
    if false {
        x = 99
    } else {
        x = 77
    }
    return x
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(77));
}
