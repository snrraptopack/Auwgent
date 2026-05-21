use super::*;
use super::utils::compile_source_with_prelude;

#[test]
fn execute_for_loop_over_literal_array() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    for x in [1, 2, 3] {
        let y = x
    }
    return 42
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
fn execute_for_loop_with_index() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    for item, idx in [10, 20, 30] {
        let y = item
    }
    return 99
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(99));
}

#[test]
fn execute_while_loop_with_mutation() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    while count < 3 {
        count = count + 1
    }
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
    assert_eq!(result, Value::Number(3));
}

#[test]
fn execute_while_loop_zero_iterations() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 5
    while count < 3 {
        let count = count + 1
    }
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
fn execute_break_in_while() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    while true {
        count = count + 1
        if count >= 3 {
            break
        }
    }
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
    assert_eq!(result, Value::Number(3));
}

#[test]
fn execute_continue_in_for() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let sum = 0
    for x in [1, 2, 3, 4, 5] {
        if x == 3 {
            continue
        }
        sum = sum + x
    }
    return sum
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(12));
}

#[test]
fn execute_break_inside_if_in_while() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    while count < 10 {
        count = count + 1
        if count == 4 {
            break
        }
    }
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
    assert_eq!(result, Value::Number(4));
}
