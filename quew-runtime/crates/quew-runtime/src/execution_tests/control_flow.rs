use std::sync::Arc;

use quew_interner::Interner;
use quew_ir::graph::{AgentGraph, CheckpointPolicy, DataRef, Edge, IrNode, NodeId, NodeKind};
use quew_ir::{Definitions, ProgramMeta, QuewGraphIR};

use super::*;

#[test]
fn execute_branch_routing() {
    let interner = Arc::new(Interner::new());

    let mut nodes = indexmap::IndexMap::new();
    let mut edges = Vec::new();

    // n0: Input (bool)
    nodes.insert(
        NodeId(0),
        IrNode {
            id: NodeId(0),
            kind: NodeKind::Input {
                input_ty: quew_ir::types::IrType::Bool,
            },
            checkpoint: CheckpointPolicy::Never,
        },
    );

    // n1: Branch on input
    nodes.insert(
        NodeId(1),
        IrNode {
            id: NodeId(1),
            kind: NodeKind::Branch {
                condition: DataRef::scalar(NodeId(0)),
                then_node: NodeId(2),
                else_node: Some(NodeId(3)),
            },
            checkpoint: CheckpointPolicy::Optional,
        },
    );

    // n2: Then arm — bind 42
    nodes.insert(
        NodeId(2),
        IrNode {
            id: NodeId(2),
            kind: NodeKind::LetBind {
                name: interner.intern("then_val"),
                value: quew_ir::graph::IrExpr::Lit(quew_ir::graph::IrLit::Int(42)),
            },
            checkpoint: CheckpointPolicy::Optional,
        },
    );

    // n3: Else arm — bind 0
    nodes.insert(
        NodeId(3),
        IrNode {
            id: NodeId(3),
            kind: NodeKind::LetBind {
                name: interner.intern("else_val"),
                value: quew_ir::graph::IrExpr::Lit(quew_ir::graph::IrLit::Int(0)),
            },
            checkpoint: CheckpointPolicy::Optional,
        },
    );

    // n4: Output — returns the then_val
    nodes.insert(
        NodeId(4),
        IrNode {
            id: NodeId(4),
            kind: NodeKind::Output {
                value: DataRef::scalar(NodeId(2)),
            },
            checkpoint: CheckpointPolicy::Never,
        },
    );

    edges.push(Edge {
        from: NodeId(0),
        to: NodeId(1),
        slot: interner.intern("condition"),
    });

    let graph = AgentGraph {
        graph_id: "agent:BranchTest".to_string(),
        entry_node: NodeId(0),
        return_node: NodeId(4),
        nodes,
        edges,
        bindings: std::collections::HashMap::new(),
    };

    let mut graphs = indexmap::IndexMap::new();
    graphs.insert("agent:BranchTest".to_string(), graph);

    let ir = QuewGraphIR {
        program: ProgramMeta {
            name: interner.intern("BranchTest"),
            entry_agent: interner.intern("BranchTest"),
        },
        definitions: Definitions::default(),
        graphs,
    };

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);

    let result = exec.run("agent:BranchTest", Value::Bool(true)).unwrap();
    assert_eq!(result, Value::Number(42));
}

#[test]
fn graph_not_found() {
    let interner = Arc::new(Interner::new());
    let ir = QuewGraphIR {
        program: ProgramMeta {
            name: interner.intern("Test"),
            entry_agent: interner.intern("Test"),
        },
        definitions: Definitions::default(),
        graphs: indexmap::IndexMap::new(),
    };

    let natives = crate::native::NativeRegistry::new();
    let exec = Execution::new(&ir, &interner, &natives);
    let err = exec.run("function:missing", Value::Null).unwrap_err();
    assert!(matches!(err, ExecutionError::GraphNotFound { .. }));
}
