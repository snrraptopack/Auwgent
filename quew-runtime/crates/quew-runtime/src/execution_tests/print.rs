use super::utils::compile_source_with_prelude;
use super::*;

#[test]
fn execute_print_builtin_returns_null() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): null {
    return print("hello")
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::collect();
    if !natives.contains("std.io.print") {
        natives.register(
            "std.io.print",
            crate::native::NativeHandler::Sync(|args| {
                println!("{}", args[0]);
                Ok(Value::Null)
            }),
        );
    }
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Null);
}
