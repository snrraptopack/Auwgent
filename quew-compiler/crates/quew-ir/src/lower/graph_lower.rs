//! Lower an agent body into an `AgentGraph`.

use std::sync::Arc;

use indexmap::IndexMap;
use quew_ast::{AgentDecl, ElseClause, Expr, ReturnMode, Stmt};
use quew_checker::resolved::CallKind;
use quew_checker::CheckResult;
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

/// Lower one agent declaration's body into an immutable execution graph.
pub fn lower_agent(
    agent: &AgentDecl,
    check: &CheckResult,
    interner: &Arc<Interner>,
    definitions: &mut Definitions,
) -> AgentGraph {
    let mut builder =
        GraphBuilder::new(format!("agent:{}", interner.resolve(agent.name)), interner);

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
) -> AgentGraph {
    let mut builder = GraphBuilder::new(graph_id, interner);

    let input_ty = params.first().map(|(_, ty)| ty.clone()).unwrap_or(crate::types::IrType::Void);
    let input_id = builder.push(
        NodeKind::Input { input_ty },
        CheckpointPolicy::Never,
    );

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
            let value = ret.value.as_ref()?;
            if let Some((agent, args)) = agent_call(value, check, definitions, builder) {
                let mode = match ret.mode {
                    ReturnMode::Normal => AgentCallMode::BlackBox,
                    ReturnMode::WithTurns => AgentCallMode::WithTurns,
                };
                let id = builder.push(
                    NodeKind::AgentCall {
                        agent,
                        args: args.clone(),
                        mode,
                    },
                    CheckpointPolicy::Required,
                );
                for (slot, data) in args {
                    builder.edge(data.node, id, slot);
                }
                Some(DataRef::scalar(id))
            } else {
                Some(ensure_ref(value, check, definitions, builder))
            }
        }
        Stmt::If(if_stmt) => {
            let condition = ensure_ref(&if_stmt.condition, check, definitions, builder);
            let branch_id = builder.next_id();
            let then_node = if if_stmt.then_body.is_empty() {
                condition.node
            } else {
                NodeId(branch_id + 1)
            };
            let else_node = match &if_stmt.else_clause {
                ElseClause::Else(body, _) if !body.is_empty() => Some(NodeId(
                    branch_id + 1 + estimated_body_nodes(&if_stmt.then_body),
                )),
                ElseClause::ElseIf(if_stmt) if !if_stmt.then_body.is_empty() => Some(NodeId(
                    branch_id + 1 + estimated_body_nodes(&if_stmt.then_body),
                )),
                ElseClause::None => None,
                _ => None,
            };
            let id = builder.push(
                NodeKind::Branch {
                    condition: condition.clone(),
                    then_node,
                    else_node,
                },
                CheckpointPolicy::Optional,
            );
            builder.edge(condition.node, id, interner.intern("condition"));
            for stmt in &if_stmt.then_body {
                let _ = lower_stmt(stmt, check, interner, definitions, builder);
            }
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
                ElseClause::None => {}
            }
            None
        }
        Stmt::Expr(expr_stmt) => {
            let _ = ensure_ref(&expr_stmt.expr, check, definitions, builder);
            None
        }
        Stmt::For(_) => None,
    }
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
                                args.insert(*param_name, ensure_ref(arg, check, definitions, builder));
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

fn estimated_body_nodes(body: &[Stmt]) -> u32 {
    body.iter()
        .map(|stmt| match stmt {
            Stmt::If(if_stmt) => {
                1 + estimated_body_nodes(&if_stmt.then_body)
                    + match &if_stmt.else_clause {
                        ElseClause::Else(body, _) => estimated_body_nodes(body),
                        ElseClause::ElseIf(stmt) => 1 + estimated_body_nodes(&stmt.then_body),
                        ElseClause::None => 0,
                    }
            }
            _ => 1,
        })
        .sum()
}

struct GraphBuilder<'a> {
    graph_id: String,
    interner: &'a Arc<Interner>,
    ctx: LowerCtx,
    nodes: IndexMap<crate::graph::NodeId, IrNode>,
    edges: Vec<Edge>,
}

impl<'a> GraphBuilder<'a> {
    fn new(graph_id: String, interner: &'a Arc<Interner>) -> Self {
        Self {
            graph_id,
            interner,
            ctx: LowerCtx::new(),
            nodes: IndexMap::new(),
            edges: Vec::new(),
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
