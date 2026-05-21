use std::sync::Arc;

use quew_interner::Interner;
use quew_ir::graph::{AgentGraph, CheckpointPolicy, DataRef, IrNode, NodeId, NodeKind};
use quew_ir::{Definitions, ProgramMeta, QuewGraphIR};

use super::*;
use super::utils::compile_source_with_prelude;

#[test]
fn execute_native_builtin_dispatch() {
    let interner = Arc::new(Interner::new());

    let mut nodes = indexmap::IndexMap::new();

    nodes.insert(
        NodeId(0),
        IrNode {
            id: NodeId(0),
            kind: NodeKind::Input {
                input_ty: quew_ir::types::IrType::String,
            },
            checkpoint: CheckpointPolicy::Never,
        },
    );

    let func_name = interner.intern("std.string.len");
    nodes.insert(
        NodeId(1),
        IrNode {
            id: NodeId(1),
            kind: NodeKind::LetBind {
                name: interner.intern("len"),
                value: quew_ir::graph::IrExpr::Call {
                    function: func_name,
                    args: {
                        let mut m = indexmap::IndexMap::new();
                        m.insert(
                            interner.intern("self"),
                            quew_ir::graph::IrExpr::Ref(DataRef::scalar(NodeId(0))),
                        );
                        m
                    },
                },
            },
            checkpoint: CheckpointPolicy::Optional,
        },
    );

    nodes.insert(
        NodeId(2),
        IrNode {
            id: NodeId(2),
            kind: NodeKind::Output {
                value: DataRef::scalar(NodeId(1)),
            },
            checkpoint: CheckpointPolicy::Never,
        },
    );

    let graph = AgentGraph {
        graph_id: "function:len_test".to_string(),
        entry_node: NodeId(0),
        return_node: NodeId(2),
        nodes,
        edges: Vec::new(),
        bindings: std::collections::HashMap::new(),
    };

    let mut graphs = indexmap::IndexMap::new();
    graphs.insert("function:len_test".to_string(), graph);

    let ir = QuewGraphIR {
        program: ProgramMeta {
            name: interner.intern("LenTest"),
            entry_agent: interner.intern("LenTest"),
        },
        definitions: Definitions::default(),
        graphs,
    };

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.string.len",
        crate::native::NativeHandler::Sync(|args| {
            let s = args[0].as_str().ok_or("len: expected string")?;
            Ok(Value::Number(s.len() as i64))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec
        .run("function:len_test", Value::String("hello".into()))
        .unwrap();
    assert_eq!(result, Value::Number(5));
}

#[test]
fn execute_string_len_native_from_compiled_code() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    return len("hello")
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.string.len",
        crate::native::NativeHandler::Sync(|args| {
            let s = args[0].as_str().ok_or("expected string")?;
            Ok(Value::Number(s.len() as i64))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(5));
}

#[test]
fn execute_array_len_native_from_compiled_code() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let arr = [1, 2, 3]
    return array_len(arr)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.array.len",
        crate::native::NativeHandler::Sync(|args| {
            let arr = args[0].as_array().ok_or("expected array")?;
            Ok(Value::Number(arr.len() as i64))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(3));
}

#[test]
fn execute_array_get_native_from_compiled_code() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let arr = [10, 20, 30]
    return array_get(arr, 1)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.array.get",
        crate::native::NativeHandler::Sync(|args| {
            let arr = args[0].as_array().ok_or("expected array")?;
            let idx = args[1].as_number().ok_or("expected number")?;
            Ok(arr.get(idx as usize).cloned().unwrap_or(Value::Null))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(20));
}

#[test]
fn execute_array_push_native_from_compiled_code() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number[] {
    let arr = [1, 2]
    return array_push(arr, 3)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.array.push",
        crate::native::NativeHandler::Sync(|args| {
            let mut arr = args[0].as_array().map(|a| a.to_vec()).unwrap_or_default();
            arr.push(args[1].clone());
            Ok(Value::Array(arr))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(
        result,
        Value::Array(vec![Value::Number(1), Value::Number(2), Value::Number(3)])
    );
}

#[test]
fn execute_array_pop_native_from_compiled_code() {
    let (interner, ir) = compile_source_with_prelude(
        r#"
function test(): number {
    let arr = [1, 2, 3]
    return array_pop(arr)
}
agent Main(input: string) {
    reply(input) with { prompt: "hi", model: gemini("gemini-pro") }
}
"#,
    );

    let mut natives = crate::native::NativeRegistry::new();
    natives.register(
        "std.array.pop",
        crate::native::NativeHandler::Sync(|args| {
            let arr = args[0].as_array().ok_or("expected array")?;
            Ok(arr.last().cloned().unwrap_or(Value::Null))
        }),
    );
    let exec = Execution::new(&ir, &interner, &natives);
    let result = exec.run("function:test", Value::Null).unwrap();
    assert_eq!(result, Value::Number(3));
}
