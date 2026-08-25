//! Lower an agent body into an `AgentGraph`.

use std::collections::HashSet;
use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{
    AgentDecl, BinaryOp as AstBinaryOp, ElseClause, Expr, ForStmt, IdentExpr, ReturnMode,
    ReturnStmt, Stmt, WhileStmt,
};
use quew_checker::CheckResult;
use quew_checker::resolved::CallKind;
use quew_interner::{InternedStr, Interner};
use quew_lexer::AnnotationKind;

use crate::defs::{Definitions, ToolKind};
use crate::graph::{
    AgentCallMode, AgentGraph, CheckpointPolicy, DataRef, Edge, IrNode, NodeId, NodeKind,
};

use super::config::lower_reply_config;
use super::ctx::LowerCtx;
use super::defs::lower_type_expr;
use super::expr::{lower_expr, lower_expr_as_ref};

/// Collect all variable names that are explicitly mutated via `=` in a statement list.
fn assigned_vars_in_body(body: &[Stmt]) -> HashSet<InternedStr> {
    let mut vars = HashSet::new();
    for stmt in body {
        match stmt {
            Stmt::Expr(quew_ast::stmt::ExprStmt {
                expr: Expr::Binary(b),
                ..
            }) if b.op == AstBinaryOp::Assign => {
                if let Expr::Ident(ident) = b.left.as_ref() {
                    vars.insert(ident.name);
                }
            }
            Stmt::If(if_stmt) => {
                vars.extend(assigned_vars_in_body(&if_stmt.then_body));
                match &if_stmt.else_clause {
                    ElseClause::Else(body, _) => {
                        vars.extend(assigned_vars_in_body(body));
                    }
                    ElseClause::ElseIf(nested) => {
                        vars.extend(assigned_vars_in_body(&[Stmt::If((**nested).clone())]));
                    }
                    ElseClause::None => {}
                }
            }
            Stmt::While(while_stmt) => {
                vars.extend(assigned_vars_in_body(&while_stmt.body));
            }
            Stmt::For(for_stmt) => {
                vars.extend(assigned_vars_in_body(&for_stmt.body));
            }
            _ => {}
        }
    }
    vars
}

/// Lower one agent declaration's body into an immutable execution graph.
pub fn lower_agent(
    agent: &AgentDecl,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
) -> AgentGraph {
    let mut builder = GraphBuilder::new(
        format!("agent:{}", interner.resolve(agent.name)),
        interner,
        graphs,
    );

    let input_id = builder.push(
        NodeKind::Input {
            input_ty: lower_type_expr(&agent.param.ty, interner),
        },
        CheckpointPolicy::Never,
    );
    builder
        .ctx
        .bind(agent.param.name, DataRef::scalar(input_id));

    for ann in &agent.annotations {
        if ann.kind == AnnotationKind::Context {
            if let quew_ast::AnnotationArgs::Type(quew_ast::TypeExpr::Named(context_ty, _)) =
                &ann.args
            {
                let ctx_id = builder.push(
                    NodeKind::Context {
                        context_ty: *context_ty,
                    },
                    CheckpointPolicy::Never,
                );
                builder
                    .ctx
                    .bind(interner.intern("ctx"), DataRef::scalar(ctx_id));
            }
        }
    }

    let mut result = DataRef::scalar(input_id);
    for stmt in &agent.body {
        if let Some(value) = lower_stmt(stmt, check, interner, definitions, &mut builder) {
            result = value;
            break;
        }
    }

    let output_id = builder.push(
        NodeKind::Output {
            value: result.clone(),
        },
        CheckpointPolicy::Never,
    );
    builder.edge(result.node, output_id, interner.intern("value"));

    AgentGraph {
        graph_id: builder.graph_id,
        entry_node: input_id,
        return_node: output_id,
        nodes: builder.nodes,
        edges: builder.edges,
        bindings: builder.ctx.slots.into_iter().collect(),
    }
}

/// Lower a function or extension-method body into an `AgentGraph`.
///
/// The first parameter is bound to the graph's `Input` node.  Additional
/// parameters are bound as fields of that input (a simplification until the
/// runtime supports multi-input graphs).
pub fn lower_function_graph(
    graph_id: String,
    params: &IndexMap<InternedStr, crate::types::IrType>,
    body: &[Stmt],
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    graphs: &mut IndexMap<String, AgentGraph>,
) -> AgentGraph {
    let mut builder = GraphBuilder::new(graph_id, interner, graphs);

    let input_ty = params
        .first()
        .map(|(_, ty)| ty.clone())
        .unwrap_or(crate::types::IrType::Void);
    let input_id = builder.push(NodeKind::Input { input_ty }, CheckpointPolicy::Never);

    // All parameters are bound as fields of the Input node.  The runtime
    // always packages arguments into an object, even for single-parameter
    // functions, so every parameter resolves to `input.<name>`.
    for (name, _ty) in params.iter() {
        builder.ctx.bind(*name, DataRef::field(input_id, *name));
    }

    let mut result = DataRef::scalar(input_id);
    for stmt in body {
        if let Some(value) = lower_stmt(stmt, check, interner, definitions, &mut builder) {
            result = value;
            break;
        }
    }

    let output_id = builder.push(
        NodeKind::Output {
            value: result.clone(),
        },
        CheckpointPolicy::Never,
    );
    builder.edge(result.node, output_id, interner.intern("value"));

    AgentGraph {
        graph_id: builder.graph_id,
        entry_node: input_id,
        return_node: output_id,
        nodes: builder.nodes,
        edges: builder.edges,
        bindings: builder.ctx.slots.into_iter().collect(),
    }
}

fn lower_stmt(
    stmt: &Stmt,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    builder: &mut GraphBuilder,
) -> Option<DataRef> {
    match stmt {
        Stmt::Let(let_stmt) => {
            let (kind, checkpoint, args) =
                lower_value_node(&let_stmt.init, check, definitions, builder);
            let id = builder.push(kind, checkpoint);
            for (slot, data) in args {
                builder.edge(data.node, id, slot);
            }
            builder.ctx.bind(let_stmt.name, DataRef::scalar(id));
            None
        }
        Stmt::Reply(reply) => {
            let message = ensure_ref(&reply.input, check, definitions, builder);
            let config = lower_reply_config(
                &reply.with_block,
                check,
                interner,
                definitions,
                &mut builder.ctx,
            );
            let id = builder.push(
                NodeKind::Reply {
                    message: message.clone(),
                    config,
                },
                CheckpointPolicy::Required,
            );
            builder.edge(message.node, id, interner.intern("message"));
            Some(DataRef::scalar(id))
        }
        Stmt::Return(ret) => {
            let Some(value) = ret.value.as_ref() else {
                // Bare `return` — nothing to produce.
                return None;
            };

            // Evaluate the returned value (agent call, tool call, or pure expr).
            let (kind, checkpoint, args) = if let Some((agent, args)) =
                agent_call(value, check, definitions, builder)
            {
                let mode = match ret.mode {
                    ReturnMode::Normal => AgentCallMode::BlackBox,
                    ReturnMode::WithTurns => AgentCallMode::WithTurns,
                };
                (
                    NodeKind::AgentCall {
                        agent,
                        args: args.clone(),
                        mode,
                    },
                    CheckpointPolicy::Required,
                    args,
                )
            } else if matches!(value, Expr::Call(_)) {
                lower_value_node(value, check, definitions, builder)
            } else {
                (
                    NodeKind::LetBind {
                        name: builder.interner.intern("_"),
                        value: lower_expr(
                            value,
                            check,
                            definitions,
                            builder.interner,
                            &mut builder.ctx,
                        ),
                    },
                    CheckpointPolicy::Optional,
                    IndexMap::new(),
                )
            };

            let value_id = builder.push(kind, checkpoint);
            for (slot, data) in args {
                builder.edge(data.node, value_id, slot);
            }

            // Terminate the graph from here. The runtime records this value
            // and skips all remaining nodes, so early returns inside branch
            // bodies short-circuit correctly.
            let return_id = builder.push(
                NodeKind::Return {
                    value: DataRef::scalar(value_id),
                },
                CheckpointPolicy::Never,
            );
            builder.edge(value_id, return_id, interner.intern("value"));

            None
        }
        Stmt::If(if_stmt) => {
            let condition = ensure_ref(&if_stmt.condition, check, definitions, builder);

            // Determine which variables are mutated inside either branch.
            let then_assigned = assigned_vars_in_body(&if_stmt.then_body);
            let else_assigned = match &if_stmt.else_clause {
                ElseClause::Else(body, _) => assigned_vars_in_body(body),
                ElseClause::ElseIf(nested) => {
                    assigned_vars_in_body(&[Stmt::If((**nested).clone())])
                }
                ElseClause::None => HashSet::new(),
            };
            let mut merged: Vec<InternedStr> =
                then_assigned.union(&else_assigned).copied().collect();
            // Only merge variables that exist in the outer scope.
            merged.retain(|name| builder.ctx.slots.contains_key(name));

            // Snapshot bindings before the branch — always restore so that
            // let-declarations inside a branch do not leak into the outer scope.
            let snapshot = builder.ctx.slots.clone();

            // Push a placeholder Branch node. Targets and arm spans are
            // back-patched after the bodies are lowered, because predicting
            // node counts ahead of time is unreliable (nested ifs push extra
            // merge nodes, non-ident conditions push temp LetBind nodes).
            let branch_id = builder.next_id();
            let id = builder.push(
                NodeKind::Branch {
                    condition: condition.clone(),
                    then_node: condition.node,
                    else_node: None,
                    then_span: None,
                    else_span: None,
                },
                CheckpointPolicy::Optional,
            );
            builder.edge(condition.node, id, interner.intern("condition"));

            // Lower then-branch, recording its exact node span.
            let then_span = if if_stmt.then_body.is_empty() {
                None
            } else {
                let start = builder.next_id();
                for stmt in &if_stmt.then_body {
                    let _ = lower_stmt(stmt, check, interner, definitions, builder);
                }
                span_from(NodeId(start), builder.next_id())
            };
            let then_slots = builder.ctx.slots.clone();

            // Restore snapshot and lower else-branch, recording its span.
            builder.ctx.slots = snapshot.clone();
            let else_start = match &if_stmt.else_clause {
                ElseClause::Else(body, _) if !body.is_empty() => Some(builder.next_id()),
                ElseClause::ElseIf(nested) if !nested.then_body.is_empty() => {
                    Some(builder.next_id())
                }
                _ => None,
            };
            match &if_stmt.else_clause {
                ElseClause::Else(body, _) => {
                    for stmt in body {
                        let _ = lower_stmt(stmt, check, interner, definitions, builder);
                    }
                }
                ElseClause::ElseIf(nested) => {
                    let _ = lower_stmt(
                        &Stmt::If((**nested).clone()),
                        check,
                        interner,
                        definitions,
                        builder,
                    );
                }
                _ => {}
            }
            let else_span =
                else_start.and_then(|start| span_from(NodeId(start), builder.next_id()));
            let else_slots = builder.ctx.slots.clone();

            // Back-patch the branch targets now that real node ids exist.
            let then_node = if if_stmt.then_body.is_empty() {
                condition.node
            } else {
                NodeId(branch_id + 1)
            };
            let else_node = else_span.map(|(start, _)| start);
            if let Some(node) = builder.nodes.get_mut(&id) {
                if let NodeKind::Branch {
                    then_node: patch_then,
                    else_node: patch_else,
                    then_span: patch_then_span,
                    else_span: patch_else_span,
                    ..
                } = &mut node.kind
                {
                    *patch_then = then_node;
                    *patch_else = else_node;
                    *patch_then_span = then_span;
                    *patch_else_span = else_span;
                }
            }

            // Restore snapshot and create merge nodes for variables assigned in either branch.
            builder.ctx.slots = snapshot.clone();
            for name in merged {
                let then_ref = then_slots.get(&name);
                let else_ref = else_slots.get(&name);
                let snapshot_ref = &snapshot[&name];
                let changed = then_ref != Some(snapshot_ref) || else_ref != Some(snapshot_ref);
                if changed {
                    let merge_value = crate::graph::IrExpr::Ternary {
                        cond: Box::new(crate::graph::IrExpr::Ref(condition.clone())),
                        then: Box::new(crate::graph::IrExpr::Ref(
                            then_ref.cloned().unwrap_or_else(|| snapshot_ref.clone()),
                        )),
                        else_: Box::new(crate::graph::IrExpr::Ref(
                            else_ref.cloned().unwrap_or_else(|| snapshot_ref.clone()),
                        )),
                    };
                    let merge_id = builder.push(
                        NodeKind::LetBind {
                            name,
                            value: merge_value,
                        },
                        CheckpointPolicy::Optional,
                    );
                    builder.ctx.bind(name, DataRef::scalar(merge_id));
                }
            }

            None
        }
        Stmt::Expr(expr_stmt) => {
            if let Expr::Binary(binary) = &expr_stmt.expr {
                if binary.op == AstBinaryOp::Assign {
                    return lower_assignment(binary, check, interner, definitions, builder);
                }
            }
            let _ = ensure_ref(&expr_stmt.expr, check, definitions, builder);
            None
        }
        Stmt::For(for_stmt) => lower_for_loop(for_stmt, check, interner, definitions, builder),
        Stmt::While(while_stmt) => {
            lower_while_loop(while_stmt, check, interner, definitions, builder)
        }
        Stmt::Break(_) => {
            builder.push(NodeKind::Break, CheckpointPolicy::Never);
            None
        }
        Stmt::Continue(_) => {
            builder.push(NodeKind::Continue, CheckpointPolicy::Never);
            None
        }
    }
}

fn lower_assignment(
    binary: &quew_ast::expr::BinaryExpr,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &Definitions,
    builder: &mut GraphBuilder,
) -> Option<DataRef> {
    let name = match binary.left.as_ref() {
        Expr::Ident(ident) => ident.name,
        _ => panic!("lowering bug: assignment target is not an identifier"),
    };

    // If RHS is a call, reuse lower_value_node to get the proper node kind.
    if matches!(binary.right.as_ref(), Expr::Call(_)) {
        let (kind, checkpoint, args) = lower_value_node(&binary.right, check, definitions, builder);
        let id = builder.push(kind, checkpoint);
        for (slot, data) in args {
            builder.edge(data.node, id, slot);
        }
        builder.ctx.bind(name, DataRef::scalar(id));
    } else {
        let value = lower_expr(
            &binary.right,
            check,
            definitions,
            interner,
            &mut builder.ctx,
        );
        let id = builder.push(
            NodeKind::LetBind { name, value },
            CheckpointPolicy::Optional,
        );
        builder.ctx.bind(name, DataRef::scalar(id));
    }

    None
}

fn lower_for_loop(
    for_stmt: &ForStmt,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    builder: &mut GraphBuilder,
) -> Option<DataRef> {
    // Lower the iterable expression.
    let iterable_ref = ensure_ref(&for_stmt.iterable, check, definitions, builder);

    // Collect captured variables (all bindings except the loop variable and index).
    let skip: std::collections::HashSet<InternedStr> = [Some(for_stmt.value), for_stmt.index]
        .into_iter()
        .flatten()
        .collect();

    let captured: Vec<(InternedStr, DataRef)> = builder
        .ctx
        .slots
        .iter()
        .filter(|(name, _)| !skip.contains(name))
        .map(|(name, data_ref)| (*name, data_ref.clone()))
        .collect();

    // Generate a unique body graph ID.
    let body_graph_id = format!(
        "{}:for:{}",
        builder.graph_id,
        interner.resolve(for_stmt.value)
    );

    // Build body parameters.
    let mut body_params: IndexMap<InternedStr, crate::types::IrType> = IndexMap::new();
    body_params.insert(for_stmt.value, crate::types::IrType::Void);
    if let Some(idx) = for_stmt.index {
        body_params.insert(idx, crate::types::IrType::Void);
    }
    for (name, _) in &captured {
        body_params.insert(*name, crate::types::IrType::Void);
    }

    // Lower the body into a graph.
    let body_graph = lower_function_graph(
        body_graph_id.clone(),
        &body_params,
        &for_stmt.body,
        check,
        interner,
        definitions,
        builder.graphs,
    );
    builder.graphs.insert(body_graph_id.clone(), body_graph);

    // Create the Loop node.
    let loop_id = builder.push(
        NodeKind::Loop {
            iterable: iterable_ref.clone(),
            body_graph: body_graph_id,
            value_name: for_stmt.value,
            index_name: for_stmt.index,
            captured: captured.clone(),
        },
        CheckpointPolicy::Optional,
    );
    builder.edge(iterable_ref.node, loop_id, interner.intern("iterable"));

    // Bind captured variables as edges so the lowerer knows they're used.
    for (name, data_ref) in &captured {
        builder.edge(data_ref.node, loop_id, *name);
    }

    None
}

fn lower_while_loop(
    while_stmt: &WhileStmt,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
    builder: &mut GraphBuilder,
) -> Option<DataRef> {
    // Collect captured variables.
    let captured: Vec<(InternedStr, DataRef)> = builder
        .ctx
        .slots
        .iter()
        .map(|(name, data_ref)| (*name, data_ref.clone()))
        .collect();

    // Generate a unique body graph ID.
    let body_graph_id = format!("{}:while", builder.graph_id);

    // Build synthetic body:
    //   let __cond = condition
    //   if __cond { body... }
    //   return { __cond: __cond, captured_var1: captured_var1, ... }
    let cond_name = interner.intern("__cond");
    let cond_span = while_stmt.condition.span();

    let mut return_fields = vec![quew_ast::expr::ObjectField {
        name: cond_name,
        value: Box::new(Expr::Ident(IdentExpr {
            name: cond_name,
            span: cond_span,
        })),
        span: cond_span,
    }];
    for (name, _) in &captured {
        return_fields.push(quew_ast::expr::ObjectField {
            name: *name,
            value: Box::new(Expr::Ident(IdentExpr {
                name: *name,
                span: cond_span,
            })),
            span: cond_span,
        });
    }

    let synthetic_body = vec![
        Stmt::Let(quew_ast::stmt::LetStmt {
            name: cond_name,
            ty: None,
            init: while_stmt.condition.clone(),
            span: cond_span,
        }),
        Stmt::If(quew_ast::stmt::IfStmt {
            condition: Expr::Ident(IdentExpr {
                name: cond_name,
                span: cond_span,
            }),
            then_body: while_stmt.body.clone(),
            else_clause: ElseClause::None,
            span: cond_span,
        }),
        Stmt::Return(ReturnStmt {
            value: Some(Expr::Object(quew_ast::expr::ObjectExpr {
                fields: return_fields,
                span: cond_span,
            })),
            mode: ReturnMode::Normal,
            span: cond_span,
        }),
    ];

    // Build body parameters.
    let mut body_params: IndexMap<InternedStr, crate::types::IrType> = IndexMap::new();
    for (name, _) in &captured {
        body_params.insert(*name, crate::types::IrType::Void);
    }

    // Lower the synthetic body into a graph.
    let body_graph = lower_function_graph(
        body_graph_id.clone(),
        &body_params,
        &synthetic_body,
        check,
        interner,
        definitions,
        builder.graphs,
    );
    builder.graphs.insert(body_graph_id.clone(), body_graph);

    // Create the WhileLoop node.
    let while_id = builder.push(
        NodeKind::WhileLoop {
            body_graph: body_graph_id,
            captured: captured.clone(),
        },
        CheckpointPolicy::Optional,
    );

    // Bind captured variables as edges.
    for (name, data_ref) in &captured {
        builder.edge(data_ref.node, while_id, *name);
    }

    None
}

fn lower_value_node(
    expr: &Expr,
    check: &CheckResult,
    definitions: &Definitions,
    builder: &mut GraphBuilder,
) -> (NodeKind, CheckpointPolicy, IndexMap<InternedStr, DataRef>) {
    if let Expr::Call(call) = expr {
        if let Expr::Ident(callee) = call.callee.as_ref() {
            let args = call_args(callee.name, &call.args, check, definitions, builder);
            if let Some(tool) = definitions.tools.get(&callee.name) {
                let checkpoint = match tool.kind {
                    ToolKind::Host { .. } | ToolKind::Dsl { .. } => CheckpointPolicy::Required,
                    ToolKind::Group { .. } => CheckpointPolicy::Optional,
                };
                return (
                    NodeKind::HostToolCall {
                        tool: callee.name,
                        args: args.clone(),
                    },
                    checkpoint,
                    args,
                );
            }
            if definitions.functions.contains_key(&callee.name) {
                return (
                    NodeKind::FuncCall {
                        function: callee.name,
                        args: args.clone(),
                    },
                    CheckpointPolicy::Optional,
                    args,
                );
            }
            if definitions.agents.contains_key(&callee.name) {
                return (
                    NodeKind::AgentCall {
                        agent: callee.name,
                        args: args.clone(),
                        mode: AgentCallMode::BlackBox,
                    },
                    CheckpointPolicy::Required,
                    args,
                );
            }
        } else if let Expr::Member(member) = call.callee.as_ref() {
            // Extension method call: value.method()
            if let Some(resolved) = check.resolved.get_call(call.span) {
                if resolved.kind == CallKind::ExtensionMethod {
                    let graph_ref = extension_graph_ref(resolved, builder.interner);
                    let function = builder.interner.intern(&graph_ref);

                    let mut args = IndexMap::new();
                    let receiver_ref = ensure_ref(&member.object, check, definitions, builder);
                    args.insert(builder.interner.intern("self"), receiver_ref);

                    // Map explicit args using param names from the extension def.
                    if let Some(ext) = definitions
                        .extensions
                        .iter()
                        .find(|e| e.graph_ref == graph_ref)
                    {
                        for (idx, (param_name, _)) in ext.params.iter().enumerate() {
                            if let Some(arg) = call.args.get(idx) {
                                args.insert(
                                    *param_name,
                                    ensure_ref(arg, check, definitions, builder),
                                );
                            }
                        }
                    } else {
                        // Fallback: positional args with generated names.
                        for (idx, arg) in call.args.iter().enumerate() {
                            let name = builder.interner.intern(&format!("arg{idx}"));
                            args.insert(name, ensure_ref(arg, check, definitions, builder));
                        }
                    }

                    return (
                        NodeKind::FuncCall {
                            function,
                            args: args.clone(),
                        },
                        CheckpointPolicy::Optional,
                        args,
                    );
                }
            }
        }
    }

    (
        NodeKind::LetBind {
            name: builder.interner.intern("_"),
            value: lower_expr(expr, check, definitions, builder.interner, &mut builder.ctx),
        },
        CheckpointPolicy::Optional,
        IndexMap::new(),
    )
}

fn ensure_ref(
    expr: &Expr,
    check: &CheckResult,
    definitions: &Definitions,
    builder: &mut GraphBuilder,
) -> DataRef {
    match expr {
        Expr::Ident(_) | Expr::Member(_) => lower_expr_as_ref(expr, check, &mut builder.ctx),
        _ => {
            let value = lower_expr(expr, check, definitions, builder.interner, &mut builder.ctx);
            let id = builder.push(
                NodeKind::LetBind {
                    name: builder.interner.intern("_"),
                    value,
                },
                CheckpointPolicy::Optional,
            );
            DataRef::scalar(id)
        }
    }
}

fn agent_call(
    expr: &Expr,
    check: &CheckResult,
    definitions: &Definitions,
    builder: &mut GraphBuilder,
) -> Option<(InternedStr, IndexMap<InternedStr, DataRef>)> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Ident(callee) = call.callee.as_ref() else {
        return None;
    };
    if !definitions.agents.contains_key(&callee.name) {
        return None;
    }
    Some((
        callee.name,
        call_args(callee.name, &call.args, check, definitions, builder),
    ))
}

fn call_args(
    callee: InternedStr,
    args: &[Expr],
    check: &CheckResult,
    definitions: &Definitions,
    builder: &mut GraphBuilder,
) -> IndexMap<InternedStr, DataRef> {
    let names = param_names(callee, definitions);
    args.iter()
        .enumerate()
        .map(|(idx, arg)| {
            let name = names
                .get(idx)
                .copied()
                .unwrap_or_else(|| builder.interner.intern(&format!("arg{idx}")));
            (name, ensure_ref(arg, check, definitions, builder))
        })
        .collect()
}

fn param_names(callee: InternedStr, definitions: &Definitions) -> Vec<InternedStr> {
    if definitions.agents.contains_key(&callee) {
        return Vec::new();
    }
    if let Some(function) = definitions.functions.get(&callee) {
        return function.params.keys().copied().collect();
    }
    if let Some(tool) = definitions.tools.get(&callee) {
        return match &tool.kind {
            ToolKind::Host { params, .. } => params.keys().copied().collect(),
            ToolKind::Dsl {
                model_params,
                host_params,
                ..
            } => model_params
                .keys()
                .chain(host_params.keys())
                .copied()
                .collect(),
            ToolKind::Group { .. } => Vec::new(),
        };
    }
    Vec::new()
}

/// Build the graph_ref string for an extension method from checker resolution.
fn extension_graph_ref(
    resolved: &quew_checker::resolved::ResolvedCall,
    interner: &Arc<Interner>,
) -> String {
    let receiver_ty = resolved
        .receiver_ty
        .as_ref()
        .expect("extension method resolved call missing receiver_ty");
    let receiver_name = ty_to_string(receiver_ty, interner);
    format!(
        "extension:{}:{}",
        receiver_name,
        interner.resolve(resolved.target)
    )
}

/// Convert a checker `Ty` into the string form used in IR `graph_ref`s.
fn ty_to_string(ty: &quew_types::Ty, interner: &Arc<Interner>) -> String {
    use quew_types::{PrimTy, Ty};
    match ty {
        Ty::Primitive(PrimTy::String) => "string".into(),
        Ty::Primitive(PrimTy::Number) => "number".into(),
        Ty::Primitive(PrimTy::Float) => "float".into(),
        Ty::Primitive(PrimTy::Bool) => "bool".into(),
        Ty::Primitive(PrimTy::Null) => "null".into(),
        Ty::Primitive(PrimTy::Void) => "void".into(),
        Ty::Named(name) => interner.resolve(*name).to_string(),
        _ => "unknown".into(),
    }
}

/// Inclusive node span `[start, next_free)` — `None` when the arm lowered
/// zero nodes.
fn span_from(start: NodeId, next_free: u32) -> Option<(NodeId, NodeId)> {
    if next_free > start.0 {
        Some((start, NodeId(next_free - 1)))
    } else {
        None
    }
}

struct GraphBuilder<'a> {
    graph_id: String,
    interner: &'a Arc<Interner>,
    ctx: LowerCtx,
    nodes: IndexMap<crate::graph::NodeId, IrNode>,
    edges: Vec<Edge>,
    graphs: &'a mut IndexMap<String, AgentGraph>,
}

impl<'a> GraphBuilder<'a> {
    fn new(
        graph_id: String,
        interner: &'a Arc<Interner>,
        graphs: &'a mut IndexMap<String, AgentGraph>,
    ) -> Self {
        Self {
            graph_id,
            interner,
            ctx: LowerCtx::new(),
            nodes: IndexMap::new(),
            edges: Vec::new(),
            graphs,
        }
    }

    fn push(&mut self, kind: NodeKind, checkpoint: CheckpointPolicy) -> NodeId {
        let id = self.ctx.next_node();
        self.nodes.insert(
            id,
            IrNode {
                id,
                kind,
                checkpoint,
            },
        );
        id
    }

    fn edge(&mut self, from: NodeId, to: NodeId, slot: InternedStr) {
        self.edges.push(Edge { from, to, slot });
    }

    fn next_id(&self) -> u32 {
        self.nodes.len() as u32
    }
}
