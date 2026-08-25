//! Safety-limit regression tests: loop iteration caps, recursion depth caps,
//! float division-by-zero, and richer data-ref error details.

use super::utils::compile_source_with_prelude;
use super::*;
use crate::execution::ExecutionLimits;

#[test]
fn infinite_while_loop_hits_iteration_limit() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    while true {
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
    let err = exec.run("function:test", Value::Null).unwrap_err();

    match err {
        ExecutionError::LoopLimitExceeded { limit, .. } => {
            assert_eq!(limit, 100_000, "default limit should be 100_000");
        }
        other => panic!("expected LoopLimitExceeded, got {other:?}"),
    }
}

#[test]
fn loop_limit_is_configurable() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let count = 0
    while true {
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
    let limits = ExecutionLimits {
        max_loop_iterations: 10,
        max_call_depth: 64,
    };
    let exec = Execution::with_limits(&ir, &interner, &natives, limits);
    let err = exec.run("function:test", Value::Null).unwrap_err();

    match err {
        ExecutionError::LoopLimitExceeded { limit, .. } => assert_eq!(limit, 10),
        other => panic!("expected LoopLimitExceeded, got {other:?}"),
    }
}

#[test]
fn unbounded_recursion_hits_depth_limit() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function spin(n: number): number {
    return spin(n)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let args = Value::Object(indexmap::IndexMap::from([(
        "n".to_string(),
        Value::Number(1),
    )]));
    let err = exec.run("function:spin", args).unwrap_err();
    let rendered = err.to_string();

    // Each recursion level wraps the previous error, so the root cause is
    // buried in a chain. Assert the limit fired anywhere in the chain.
    assert!(
        rendered.contains("call-depth limit of 64"),
        "expected call-depth limit of 64 to fire, got: {}",
        &rendered[..rendered.len().min(400)]
    );
}

#[test]
fn missing_field_error_names_the_field() {
    // A function whose parameter object lacks the accessed field.
    let (interner, ir) = compile_source_with_prelude(
        r#"
function read_name(payload: UserLike): string {
    return payload.name
}
type UserLike = {
    id: string
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    // Functions receive their arguments as an object keyed by param name.
    let mut inner = indexmap::IndexMap::new();
    inner.insert("id".to_string(), Value::String("u1".to_string()));
    let input = Value::Object(indexmap::IndexMap::from([(
        "payload".to_string(),
        Value::Object(inner),
    )]));
    let err = exec.run("function:read_name", input).unwrap_err();
    let rendered = err.to_string();

    assert!(
        rendered.contains("name"),
        "error should name the missing field, got: {rendered}"
    );
}

#[test]
fn float_division_by_zero_is_an_error() {
    use crate::value::{Value, ValueError};

    let zero_float = Value::Float(0.0);
    let zero_int = Value::Number(0);
    let one = Value::Float(1.0);

    for divisor in [&zero_float, &zero_int] {
        let err = one.div(divisor).unwrap_err();
        assert!(
            matches!(err, ValueError::DivisionByZero),
            "expected DivisionByZero, got {err:?}"
        );
    }
}
