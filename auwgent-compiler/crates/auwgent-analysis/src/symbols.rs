use crate::completion::{contains_offset, statement_span, ActiveWorkflow};
use auwgent_ast::{
    AgentConfig, Condition, Element, Expr, HelperCall, Model, PromptStatement, Statement, TypeExpr,
    WorkflowConfig,
};
use auwgent_errors::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SymbolTargetKind {
    Identifier(String),
    Callable(String),
    ContextField(String),
    Helper(String),
    Prompt(String),
    Type(String),
    Model(String),
    Member { root: String, path: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolTarget {
    pub kind: SymbolTargetKind,
    pub span: Span,
}

pub(crate) fn symbol_at_offset(model: &Model, offset: usize) -> Option<SymbolTarget> {
    for element in &model.elements {
        if let Some(symbol) = symbol_in_element(element, offset) {
            return Some(symbol);
        }
    }
    None
}

pub(crate) fn symbol_occurrences(model: &Model) -> Vec<SymbolTarget> {
    let mut symbols = Vec::new();
    for element in &model.elements {
        collect_element_symbols(element, &mut symbols);
    }
    symbols
}

pub(crate) fn workflow_symbol_occurrences(workflow: &ActiveWorkflow<'_>) -> Vec<SymbolTarget> {
    let mut symbols = Vec::new();
    for config in workflow.parent_configs {
        if !matches!(config, AgentConfig::Workflow(_)) {
            collect_agent_config_symbols(config, &mut symbols);
        }
    }
    collect_workflow_symbols(workflow.workflow, &mut symbols);
    symbols
}

pub(crate) fn find_local_variable_definition(
    workflow: &ActiveWorkflow<'_>,
    offset: usize,
    name: &str,
) -> Option<Span> {
    for config in workflow.parent_configs {
        match config {
            AgentConfig::Input(input) => match &input.shape {
                auwgent_ast::InputShape::Properties(properties) => {
                    if let Some(property) = properties
                        .iter()
                        .find(|property| property.name.value == name)
                    {
                        return Some(property.name.span);
                    }
                }
                auwgent_ast::InputShape::Direct(_) if name == "input" => {
                    return Some(input.span);
                }
                _ => {}
            },
            AgentConfig::Context(context) => {
                if let Some(property) = context
                    .properties
                    .iter()
                    .find(|property| property.name.value == name)
                {
                    return Some(property.name.span);
                }
            }
            _ => {}
        }
    }

    if let Some(param) = workflow
        .workflow
        .params
        .iter()
        .find(|param| param.name.value == name)
    {
        return Some(param.name.span);
    }

    find_variable_definition_in_statements(&workflow.workflow.body, offset, name)
}

pub(crate) fn find_context_field_definition(
    workflow: &ActiveWorkflow<'_>,
    name: &str,
) -> Option<Span> {
    for config in workflow.parent_configs {
        if let AgentConfig::Context(context) = config {
            if let Some(property) = context
                .properties
                .iter()
                .find(|property| property.name.value == name)
            {
                return Some(property.name.span);
            }
        }
    }

    None
}

pub(crate) fn find_tool_definition(workflow: &ActiveWorkflow<'_>, name: &str) -> Option<Span> {
    for config in workflow.parent_configs {
        match config {
            AgentConfig::Tool(tool) if tool.name.value == name => return Some(tool.name.span),
            AgentConfig::Tools(tools) => {
                if let Some(tool) = tools.iter().find(|tool| tool.name.value == name) {
                    return Some(tool.name.span);
                }
            }
            _ => {}
        }
    }

    workflow
        .workflow
        .tool_configs
        .iter()
        .find(|tool| tool.name.value == name)
        .map(|tool| tool.name.span)
}

fn symbol_in_element(element: &Element, offset: usize) -> Option<SymbolTarget> {
    match element {
        Element::Agent(agent) => {
            for config in &agent.configs {
                if let Some(symbol) = symbol_in_agent_config(config, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        Element::Helper(helper) => {
            if contains_offset(helper.name.span.start, helper.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Helper(helper.name.value.clone()),
                    span: helper.name.span,
                });
            }

            for config in &helper.configs {
                if let Some(symbol) = symbol_in_agent_config(config, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        Element::ComponentDecl(_) => None,
        Element::TypeDecl(ty) => {
            if contains_offset(ty.name.span.start, ty.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Type(ty.name.value.clone()),
                    span: ty.name.span,
                });
            }

            for field in &ty.fields {
                if let Some(symbol) = symbol_in_type_expr(&field.ty, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        Element::NamedPrompt(prompt) => {
            if contains_offset(prompt.name.span.start, prompt.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Prompt(prompt.name.value.clone()),
                    span: prompt.name.span,
                });
            }

            for param in &prompt.params {
                if let Some(symbol) = symbol_in_type_expr(&param.ty, offset) {
                    return Some(symbol);
                }
            }

            for statement in &prompt.body {
                if let Some(symbol) = symbol_in_prompt_statement(statement, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        Element::ModelDef(model) => {
            if contains_offset(model.name.span.start, model.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Model(model.name.value.clone()),
                    span: model.name.span,
                });
            }
            None
        }
        Element::IntentDecl(intent) => {
            if contains_offset(intent.name.span.start, intent.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Type(intent.name.value.clone()),
                    span: intent.name.span,
                });
            }
            for field in &intent.fields {
                if let Some(symbol) = symbol_in_type_expr(&field.ty, offset) {
                    return Some(symbol);
                }
            }
            None
        }
    }
}

fn symbol_in_agent_config(config: &AgentConfig, offset: usize) -> Option<SymbolTarget> {
    match config {
        AgentConfig::Input(input) => match &input.shape {
            auwgent_ast::InputShape::Properties(properties) => {
                for property in properties {
                    if contains_offset(property.name.span.start, property.name.span.end, offset) {
                        return Some(SymbolTarget {
                            kind: SymbolTargetKind::Identifier(property.name.value.clone()),
                            span: property.name.span,
                        });
                    }

                    if let Some(symbol) = symbol_in_type_expr(&property.ty, offset) {
                        return Some(symbol);
                    }
                }
                None
            }
            auwgent_ast::InputShape::Direct(ty) => symbol_in_type_expr(ty, offset),
        },
        AgentConfig::Context(context) => {
            for property in &context.properties {
                if contains_offset(property.name.span.start, property.name.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Identifier(property.name.value.clone()),
                        span: property.name.span,
                    });
                }

                if let Some(symbol) = symbol_in_type_expr(&property.ty, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        AgentConfig::Output(output) => symbol_in_output(output, offset),
        AgentConfig::Tool(tool) => symbol_in_tool(tool, offset),
        AgentConfig::Tools(tools) => tools.iter().find_map(|tool| symbol_in_tool(tool, offset)),
        AgentConfig::Workflow(workflow) => symbol_in_workflow(workflow, offset),
        AgentConfig::Helpers(helpers) => helpers.helpers.iter().find_map(|helper| {
            contains_offset(helper.name.span.start, helper.name.span.end, offset).then(|| {
                SymbolTarget {
                    kind: SymbolTargetKind::Helper(helper.name.value.clone()),
                    span: helper.name.span,
                }
            })
        }),
        AgentConfig::Model(model) => {
            // ... (existing model logic)
            // (Note: keeping current implementation as is)
            if let auwgent_ast::ModelProviderRef::Ref(name) = &model.default_config.model {
                if contains_offset(name.span.start, name.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Model(name.value.clone()),
                        span: name.span,
                    });
                }
            }

            for config in &model.named_configs {
                if let auwgent_ast::ModelProviderRef::Ref(name) = &config.config.model {
                    if contains_offset(name.span.start, name.span.end, offset) {
                        return Some(SymbolTarget {
                            kind: SymbolTargetKind::Model(name.value.clone()),
                            span: name.span,
                        });
                    }
                }
            }
            None
        }
        AgentConfig::Intent(intent) => {
            // Handle IntentExpr symbols
            symbol_in_intent_expr(&intent.expr, offset)
        }
        _ => None,
    }
}

fn symbol_in_intent_expr(expr: &auwgent_ast::IntentExpr, offset: usize) -> Option<SymbolTarget> {
    match expr {
        auwgent_ast::IntentExpr::Ref(name) => {
            if contains_offset(name.span.start, name.span.end, offset) {
                Some(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(name.value.clone()),
                    span: name.span,
                })
            } else {
                None
            }
        }
        auwgent_ast::IntentExpr::Inline(intents) => {
            for intent in intents {
                if contains_offset(intent.name.span.start, intent.name.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Identifier(intent.name.value.clone()),
                        span: intent.name.span,
                    });
                }
                for field in &intent.fields {
                    if let Some(symbol) = symbol_in_type_expr(&field.ty, offset) {
                        return Some(symbol);
                    }
                }
            }
            None
        }
        auwgent_ast::IntentExpr::Compose(left, right) => {
            symbol_in_intent_expr(left, offset).or_else(|| symbol_in_intent_expr(right, offset))
        }
    }
}

fn symbol_in_output(output: &auwgent_ast::OutputConfig, offset: usize) -> Option<SymbolTarget> {
    match &output.shape {
        auwgent_ast::OutputShape::Properties(properties) => {
            for property in properties {
                if let Some(symbol) = symbol_in_type_expr(&property.decl.ty, offset) {
                    return Some(symbol);
                }
            }
            None
        }
        auwgent_ast::OutputShape::Union(options) => options.iter().find_map(|option| {
            contains_offset(option.span.start, option.span.end, offset).then(|| SymbolTarget {
                kind: SymbolTargetKind::Type(option.value.clone()),
                span: option.span,
            })
        }),
        auwgent_ast::OutputShape::Direct { ty, .. } => symbol_in_type_expr(ty, offset),
    }
}

fn symbol_in_tool(tool: &auwgent_ast::ToolFunction, offset: usize) -> Option<SymbolTarget> {
    if contains_offset(tool.name.span.start, tool.name.span.end, offset) {
        return Some(SymbolTarget {
            kind: SymbolTargetKind::Callable(tool.name.value.clone()),
            span: tool.name.span,
        });
    }

    for param in &tool.params {
        if let Some(symbol) = symbol_in_type_expr(&param.ty, offset) {
            return Some(symbol);
        }
    }

    tool.returns
        .as_ref()
        .and_then(|ty| symbol_in_type_expr(ty, offset))
}

fn symbol_in_workflow(workflow: &WorkflowConfig, offset: usize) -> Option<SymbolTarget> {
    for param in &workflow.params {
        if contains_offset(param.name.span.start, param.name.span.end, offset) {
            return Some(SymbolTarget {
                kind: SymbolTargetKind::Identifier(param.name.value.clone()),
                span: param.name.span,
            });
        }

        if let Some(symbol) = symbol_in_type_expr(&param.ty, offset) {
            return Some(symbol);
        }
    }

    if let Some(symbol) = symbol_in_type_expr(&workflow.return_type, offset) {
        return Some(symbol);
    }

    for tool in &workflow.tool_configs {
        if let Some(symbol) = symbol_in_tool(tool, offset) {
            return Some(symbol);
        }
    }

    for statement in &workflow.body {
        if let Some(symbol) = symbol_in_statement(statement, offset) {
            return Some(symbol);
        }
    }

    None
}

fn symbol_in_prompt_statement(statement: &PromptStatement, offset: usize) -> Option<SymbolTarget> {
    match statement {
        PromptStatement::Expr(expr) => symbol_in_expr(expr, offset),
        PromptStatement::If(condition) => {
            symbol_in_statement(&Statement::If(condition.clone()), offset)
        }
        PromptStatement::Statement(statement) => symbol_in_statement(statement, offset),
        PromptStatement::Example(_) => None,
    }
}

fn symbol_in_statement(statement: &Statement, offset: usize) -> Option<SymbolTarget> {
    match statement {
        Statement::Let(statement) => {
            if contains_offset(statement.name.span.start, statement.name.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(statement.name.value.clone()),
                    span: statement.name.span,
                });
            }

            if let Some(ty) = &statement.ty {
                if let Some(symbol) = symbol_in_type_expr(ty, offset) {
                    return Some(symbol);
                }
            }

            symbol_in_expr(&statement.value, offset)
        }
        Statement::Assign(statement) => {
            if contains_offset(
                statement.variable.span.start,
                statement.variable.span.end,
                offset,
            ) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(statement.variable.value.clone()),
                    span: statement.variable.span,
                });
            }

            symbol_in_expr(&statement.value, offset)
        }
        Statement::Return(statement) => symbol_in_expr(&statement.value, offset),
        Statement::If(statement) => {
            if let Some(symbol) = symbol_in_condition(&statement.condition, offset) {
                return Some(symbol);
            }

            for nested in &statement.then_block {
                if let Some(symbol) = symbol_in_statement(nested, offset) {
                    return Some(symbol);
                }
            }

            for nested in &statement.else_block {
                if let Some(symbol) = symbol_in_statement(nested, offset) {
                    return Some(symbol);
                }
            }

            None
        }
        Statement::Transfer(statement) => symbol_in_helper_call(&statement.call, offset),
        Statement::Parallel(statement) => statement
            .body
            .iter()
            .find_map(|nested| symbol_in_statement(nested, offset)),
    }
}

fn symbol_in_condition(condition: &Condition, offset: usize) -> Option<SymbolTarget> {
    match condition {
        Condition::Comparison { left, right, .. } => {
            symbol_in_expr(left, offset).or_else(|| symbol_in_expr(right, offset))
        }
        Condition::Logical { left, right, .. } => {
            symbol_in_condition(left, offset).or_else(|| symbol_in_condition(right, offset))
        }
        Condition::Boolean { value, .. } => symbol_in_expr(value, offset),
    }
}

fn symbol_in_expr(expr: &Expr, offset: usize) -> Option<SymbolTarget> {
    match expr {
        Expr::VarRef(name) => {
            contains_offset(name.span.start, name.span.end, offset).then(|| SymbolTarget {
                kind: SymbolTargetKind::Identifier(name.value.clone()),
                span: name.span,
            })
        }
        Expr::FunctionCall(call) => {
            for arg in &call.args {
                if let Some(symbol) = symbol_in_expr(arg, offset) {
                    return Some(symbol);
                }
            }

            contains_offset(call.name.span.start, call.name.span.end, offset).then(|| {
                SymbolTarget {
                    kind: SymbolTargetKind::Callable(call.name.value.clone()),
                    span: call.name.span,
                }
            })
        }
        Expr::HelperCall(call) => symbol_in_helper_call(call, offset),
        Expr::PromptCall(call) => {
            for arg in &call.args {
                if let Some(symbol) = symbol_in_expr(arg, offset) {
                    return Some(symbol);
                }
            }

            contains_offset(call.prompt.span.start, call.prompt.span.end, offset).then(|| {
                SymbolTarget {
                    kind: SymbolTargetKind::Prompt(call.prompt.value.clone()),
                    span: call.prompt.span,
                }
            })
        }
        Expr::ContextRef(context) => contains_offset(
            context.property.span.start,
            context.property.span.end,
            offset,
        )
        .then(|| SymbolTarget {
            kind: SymbolTargetKind::ContextField(context.property.value.clone()),
            span: context.property.span,
        }),
        Expr::MemberAccess(access) => {
            if contains_offset(access.object.span.start, access.object.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(access.object.value.clone()),
                    span: access.object.span,
                });
            }

            if contains_offset(access.property.span.start, access.property.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Member {
                        root: access.object.value.clone(),
                        path: vec![access.property.value.clone()],
                    },
                    span: access.property.span,
                });
            }

            let mut path = vec![access.property.value.clone()];
            for segment in &access.chain {
                path.push(segment.value.clone());
                if contains_offset(segment.span.start, segment.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Member {
                            root: access.object.value.clone(),
                            path: path.clone(),
                        },
                        span: segment.span,
                    });
                }
            }

            None
        }
        Expr::IndexAccess(index) => {
            if contains_offset(index.object.span.start, index.object.span.end, offset) {
                return Some(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(index.object.value.clone()),
                    span: index.object.span,
                });
            }

            if let Some(symbol) = symbol_in_expr(&index.index, offset) {
                return Some(symbol);
            }

            if let Some(property) = &index.property {
                if contains_offset(property.span.start, property.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Member {
                            root: index.object.value.clone(),
                            path: vec![property.value.clone()],
                        },
                        span: property.span,
                    });
                }
            }

            let mut path = index
                .property
                .iter()
                .map(|property| property.value.clone())
                .collect::<Vec<_>>();
            for segment in &index.chain {
                path.push(segment.value.clone());
                if contains_offset(segment.span.start, segment.span.end, offset) {
                    return Some(SymbolTarget {
                        kind: SymbolTargetKind::Member {
                            root: index.object.value.clone(),
                            path: path.clone(),
                        },
                        span: segment.span,
                    });
                }
            }

            None
        }
        Expr::BinaryOp(binary) => {
            symbol_in_expr(&binary.left, offset).or_else(|| symbol_in_expr(&binary.right, offset))
        }
        Expr::Array(array) => array
            .elements
            .iter()
            .find_map(|element| symbol_in_expr(element, offset)),
        Expr::Object(object) => object.properties.iter().find_map(|property| {
            property
                .value
                .as_ref()
                .and_then(|value| symbol_in_expr(value, offset))
        }),
        Expr::Grouped(inner, _) => symbol_in_expr(inner, offset),
        Expr::InlinePrompt(prompt) => prompt
            .parts
            .iter()
            .find_map(|part| symbol_in_prompt_statement(part, offset)),
        Expr::StringLit(_)
        | Expr::MultilineStringLit(_)
        | Expr::NumberLit(_)
        | Expr::BooleanLit(_) => None,
    }
}

fn symbol_in_helper_call(call: &HelperCall, offset: usize) -> Option<SymbolTarget> {
    for arg in &call.args {
        if let Some(symbol) = symbol_in_expr(arg, offset) {
            return Some(symbol);
        }
    }

    contains_offset(call.helper.span.start, call.helper.span.end, offset).then(|| SymbolTarget {
        kind: SymbolTargetKind::Helper(call.helper.value.clone()),
        span: call.helper.span,
    })
}

fn symbol_in_type_expr(ty: &TypeExpr, offset: usize) -> Option<SymbolTarget> {
    match ty {
        TypeExpr::Array { element, .. } => symbol_in_type_expr(element, offset),
        TypeExpr::Object { properties, .. } => properties
            .iter()
            .find_map(|property| symbol_in_type_expr(&property.ty, offset)),
        TypeExpr::TypeRef(name) => {
            contains_offset(name.span.start, name.span.end, offset).then(|| SymbolTarget {
                kind: SymbolTargetKind::Type(name.value.clone()),
                span: name.span,
            })
        }
        TypeExpr::Union { options, .. } => options.iter().find_map(|option| {
            contains_offset(option.span.start, option.span.end, offset).then(|| SymbolTarget {
                kind: SymbolTargetKind::Type(option.value.clone()),
                span: option.span,
            })
        }),
        TypeExpr::String(_)
        | TypeExpr::Number(_)
        | TypeExpr::Boolean(_)
        | TypeExpr::Text(_)
        | TypeExpr::Image(_)
        | TypeExpr::File(_)
        | TypeExpr::Audio(_)
        | TypeExpr::Video(_) => None,
    }
}

fn find_variable_definition_in_statements(
    statements: &[Statement],
    offset: usize,
    name: &str,
) -> Option<Span> {
    let mut latest = None;

    for statement in statements {
        let span = statement_span(statement);
        if span.end <= offset {
            if let Some(found) = variable_definition_in_completed_statement(statement, name) {
                latest = Some(found);
            }
            continue;
        }

        if contains_offset(span.start, span.end, offset) {
            if let Some(found) = variable_definition_in_active_statement(statement, offset, name) {
                latest = Some(found);
            }
        }
        break;
    }

    latest
}

fn variable_definition_in_completed_statement(statement: &Statement, name: &str) -> Option<Span> {
    match statement {
        Statement::Let(statement) if statement.name.value == name => Some(statement.name.span),
        Statement::Parallel(statement) => {
            find_variable_definition_in_statements(&statement.body, usize::MAX, name)
        }
        _ => None,
    }
}

fn variable_definition_in_active_statement(
    statement: &Statement,
    offset: usize,
    name: &str,
) -> Option<Span> {
    match statement {
        Statement::Let(statement) if statement.name.value == name => Some(statement.name.span),
        Statement::If(statement) => {
            if let Some(found) =
                find_variable_definition_in_statements(&statement.then_block, offset, name)
            {
                return Some(found);
            }
            find_variable_definition_in_statements(&statement.else_block, offset, name)
        }
        Statement::Parallel(statement) => {
            find_variable_definition_in_statements(&statement.body, offset, name)
        }
        _ => None,
    }
}

fn collect_element_symbols(element: &Element, symbols: &mut Vec<SymbolTarget>) {
    match element {
        Element::Agent(agent) => {
            for config in &agent.configs {
                collect_agent_config_symbols(config, symbols);
            }
        }
        Element::Helper(helper) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Helper(helper.name.value.clone()),
                span: helper.name.span,
            });
            for config in &helper.configs {
                collect_agent_config_symbols(config, symbols);
            }
        }
        Element::ComponentDecl(_) => {}
        Element::TypeDecl(ty) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Type(ty.name.value.clone()),
                span: ty.name.span,
            });
            for field in &ty.fields {
                collect_type_expr_symbols(&field.ty, symbols);
            }
        }
        Element::NamedPrompt(prompt) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Prompt(prompt.name.value.clone()),
                span: prompt.name.span,
            });
            for param in &prompt.params {
                collect_type_expr_symbols(&param.ty, symbols);
            }
            for statement in &prompt.body {
                collect_prompt_statement_symbols(statement, symbols);
            }
        }
        Element::ModelDef(model) => symbols.push(SymbolTarget {
            kind: SymbolTargetKind::Model(model.name.value.clone()),
            span: model.name.span,
        }),
        Element::IntentDecl(intent) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Type(intent.name.value.clone()),
                span: intent.name.span,
            });
            for field in &intent.fields {
                collect_type_expr_symbols(&field.ty, symbols);
            }
        }
    }
}

fn collect_agent_config_symbols(config: &AgentConfig, symbols: &mut Vec<SymbolTarget>) {
    match config {
        AgentConfig::Input(input) => match &input.shape {
            auwgent_ast::InputShape::Properties(properties) => {
                for property in properties {
                    symbols.push(SymbolTarget {
                        kind: SymbolTargetKind::Identifier(property.name.value.clone()),
                        span: property.name.span,
                    });
                    collect_type_expr_symbols(&property.ty, symbols);
                }
            }
            auwgent_ast::InputShape::Direct(ty) => {
                collect_type_expr_symbols(ty, symbols);
            }
        },
        AgentConfig::Context(context) => {
            for property in &context.properties {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(property.name.value.clone()),
                    span: property.name.span,
                });
                collect_type_expr_symbols(&property.ty, symbols);
            }
        }
        AgentConfig::Output(output) => collect_output_symbols(output, symbols),
        AgentConfig::Tool(tool) => collect_tool_symbols(tool, symbols),
        AgentConfig::Tools(tools) => {
            for tool in tools {
                collect_tool_symbols(tool, symbols);
            }
        }
        AgentConfig::Workflow(workflow) => collect_workflow_symbols(workflow, symbols),
        AgentConfig::Helpers(helpers) => {
            for helper in &helpers.helpers {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Helper(helper.name.value.clone()),
                    span: helper.name.span,
                });
            }
        }
        AgentConfig::Model(model) => {
            if let auwgent_ast::ModelProviderRef::Ref(name) = &model.default_config.model {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Model(name.value.clone()),
                    span: name.span,
                });
            }

            for config in &model.named_configs {
                if let auwgent_ast::ModelProviderRef::Ref(name) = &config.config.model {
                    symbols.push(SymbolTarget {
                        kind: SymbolTargetKind::Model(name.value.clone()),
                        span: name.span,
                    });
                }
            }
        }
        AgentConfig::Intent(intent) => {
            collect_intent_expr_symbols(&intent.expr, symbols);
        }
        _ => {}
    }
}

fn collect_intent_expr_symbols(expr: &auwgent_ast::IntentExpr, symbols: &mut Vec<SymbolTarget>) {
    match expr {
        auwgent_ast::IntentExpr::Ref(name) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Identifier(name.value.clone()),
                span: name.span,
            });
        }
        auwgent_ast::IntentExpr::Inline(intents) => {
            for intent in intents {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Identifier(intent.name.value.clone()),
                    span: intent.name.span,
                });
                for field in &intent.fields {
                    collect_type_expr_symbols(&field.ty, symbols);
                }
            }
        }
        auwgent_ast::IntentExpr::Compose(left, right) => {
            collect_intent_expr_symbols(left, symbols);
            collect_intent_expr_symbols(right, symbols);
        }
    }
}

fn collect_output_symbols(output: &auwgent_ast::OutputConfig, symbols: &mut Vec<SymbolTarget>) {
    match &output.shape {
        auwgent_ast::OutputShape::Properties(properties) => {
            for property in properties {
                collect_type_expr_symbols(&property.decl.ty, symbols);
            }
        }
        auwgent_ast::OutputShape::Union(options) => {
            for option in options {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Type(option.value.clone()),
                    span: option.span,
                });
            }
        }
        auwgent_ast::OutputShape::Direct { ty, .. } => collect_type_expr_symbols(ty, symbols),
    }
}

fn collect_tool_symbols(tool: &auwgent_ast::ToolFunction, symbols: &mut Vec<SymbolTarget>) {
    symbols.push(SymbolTarget {
        kind: SymbolTargetKind::Callable(tool.name.value.clone()),
        span: tool.name.span,
    });
    for param in &tool.params {
        collect_type_expr_symbols(&param.ty, symbols);
    }
    if let Some(ty) = &tool.returns {
        collect_type_expr_symbols(ty, symbols);
    }
}

fn collect_workflow_symbols(workflow: &WorkflowConfig, symbols: &mut Vec<SymbolTarget>) {
    for param in &workflow.params {
        symbols.push(SymbolTarget {
            kind: SymbolTargetKind::Identifier(param.name.value.clone()),
            span: param.name.span,
        });
        collect_type_expr_symbols(&param.ty, symbols);
    }

    collect_type_expr_symbols(&workflow.return_type, symbols);

    for tool in &workflow.tool_configs {
        collect_tool_symbols(tool, symbols);
    }

    for statement in &workflow.body {
        collect_statement_symbols(statement, symbols);
    }
}

fn collect_prompt_statement_symbols(statement: &PromptStatement, symbols: &mut Vec<SymbolTarget>) {
    match statement {
        PromptStatement::Expr(expr) => collect_expr_symbols(expr, symbols),
        PromptStatement::If(condition) => {
            collect_statement_symbols(&Statement::If(condition.clone()), symbols)
        }
        PromptStatement::Statement(statement) => collect_statement_symbols(statement, symbols),
        PromptStatement::Example(_) => {}
    }
}

fn collect_statement_symbols(statement: &Statement, symbols: &mut Vec<SymbolTarget>) {
    match statement {
        Statement::Let(statement) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Identifier(statement.name.value.clone()),
                span: statement.name.span,
            });
            if let Some(ty) = &statement.ty {
                collect_type_expr_symbols(ty, symbols);
            }
            collect_expr_symbols(&statement.value, symbols);
        }
        Statement::Assign(statement) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Identifier(statement.variable.value.clone()),
                span: statement.variable.span,
            });
            collect_expr_symbols(&statement.value, symbols);
        }
        Statement::Return(statement) => collect_expr_symbols(&statement.value, symbols),
        Statement::If(statement) => {
            collect_condition_symbols(&statement.condition, symbols);
            for nested in &statement.then_block {
                collect_statement_symbols(nested, symbols);
            }
            for nested in &statement.else_block {
                collect_statement_symbols(nested, symbols);
            }
        }
        Statement::Transfer(statement) => collect_helper_call_symbols(&statement.call, symbols),
        Statement::Parallel(statement) => {
            for nested in &statement.body {
                collect_statement_symbols(nested, symbols);
            }
        }
    }
}

fn collect_condition_symbols(condition: &Condition, symbols: &mut Vec<SymbolTarget>) {
    match condition {
        Condition::Comparison { left, right, .. } => {
            collect_expr_symbols(left, symbols);
            collect_expr_symbols(right, symbols);
        }
        Condition::Logical { left, right, .. } => {
            collect_condition_symbols(left, symbols);
            collect_condition_symbols(right, symbols);
        }
        Condition::Boolean { value, .. } => collect_expr_symbols(value, symbols),
    }
}

fn collect_expr_symbols(expr: &Expr, symbols: &mut Vec<SymbolTarget>) {
    match expr {
        Expr::VarRef(name) => symbols.push(SymbolTarget {
            kind: SymbolTargetKind::Identifier(name.value.clone()),
            span: name.span,
        }),
        Expr::FunctionCall(call) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Callable(call.name.value.clone()),
                span: call.name.span,
            });
            for arg in &call.args {
                collect_expr_symbols(arg, symbols);
            }
        }
        Expr::HelperCall(call) => collect_helper_call_symbols(call, symbols),
        Expr::PromptCall(call) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Prompt(call.prompt.value.clone()),
                span: call.prompt.span,
            });
            for arg in &call.args {
                collect_expr_symbols(arg, symbols);
            }
        }
        Expr::ContextRef(context) => symbols.push(SymbolTarget {
            kind: SymbolTargetKind::ContextField(context.property.value.clone()),
            span: context.property.span,
        }),
        Expr::MemberAccess(access) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Identifier(access.object.value.clone()),
                span: access.object.span,
            });
            let mut path = vec![access.property.value.clone()];
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Member {
                    root: access.object.value.clone(),
                    path: path.clone(),
                },
                span: access.property.span,
            });
            for segment in &access.chain {
                path.push(segment.value.clone());
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Member {
                        root: access.object.value.clone(),
                        path: path.clone(),
                    },
                    span: segment.span,
                });
            }
        }
        Expr::IndexAccess(index) => {
            symbols.push(SymbolTarget {
                kind: SymbolTargetKind::Identifier(index.object.value.clone()),
                span: index.object.span,
            });
            collect_expr_symbols(&index.index, symbols);
            if let Some(property) = &index.property {
                let mut path = vec![property.value.clone()];
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Member {
                        root: index.object.value.clone(),
                        path: path.clone(),
                    },
                    span: property.span,
                });
                for segment in &index.chain {
                    path.push(segment.value.clone());
                    symbols.push(SymbolTarget {
                        kind: SymbolTargetKind::Member {
                            root: index.object.value.clone(),
                            path: path.clone(),
                        },
                        span: segment.span,
                    });
                }
            }
        }
        Expr::BinaryOp(binary) => {
            collect_expr_symbols(&binary.left, symbols);
            collect_expr_symbols(&binary.right, symbols);
        }
        Expr::Array(array) => {
            for element in &array.elements {
                collect_expr_symbols(element, symbols);
            }
        }
        Expr::Object(object) => {
            for property in &object.properties {
                if let Some(value) = &property.value {
                    collect_expr_symbols(value, symbols);
                }
            }
        }
        Expr::Grouped(inner, _) => collect_expr_symbols(inner, symbols),
        Expr::InlinePrompt(prompt) => {
            for part in &prompt.parts {
                collect_prompt_statement_symbols(part, symbols);
            }
        }
        Expr::StringLit(_)
        | Expr::MultilineStringLit(_)
        | Expr::NumberLit(_)
        | Expr::BooleanLit(_) => {}
    }
}

fn collect_helper_call_symbols(call: &HelperCall, symbols: &mut Vec<SymbolTarget>) {
    symbols.push(SymbolTarget {
        kind: SymbolTargetKind::Helper(call.helper.value.clone()),
        span: call.helper.span,
    });
    for arg in &call.args {
        collect_expr_symbols(arg, symbols);
    }
}

fn collect_type_expr_symbols(ty: &TypeExpr, symbols: &mut Vec<SymbolTarget>) {
    match ty {
        TypeExpr::Array { element, .. } => collect_type_expr_symbols(element, symbols),
        TypeExpr::Object { properties, .. } => {
            for property in properties {
                collect_type_expr_symbols(&property.ty, symbols);
            }
        }
        TypeExpr::TypeRef(name) => symbols.push(SymbolTarget {
            kind: SymbolTargetKind::Type(name.value.clone()),
            span: name.span,
        }),
        TypeExpr::Union { options, .. } => {
            for option in options {
                symbols.push(SymbolTarget {
                    kind: SymbolTargetKind::Type(option.value.clone()),
                    span: option.span,
                });
            }
        }
        TypeExpr::String(_)
        | TypeExpr::Number(_)
        | TypeExpr::Boolean(_)
        | TypeExpr::Text(_)
        | TypeExpr::Image(_)
        | TypeExpr::File(_)
        | TypeExpr::Audio(_)
        | TypeExpr::Video(_) => {}
    }
}
