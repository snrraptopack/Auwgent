use crate::source::{canonicalize_best_effort, load_import_elements_best_effort, parse_source};
use auwgent_ast::{
    AgentConfig, BinOperator, Element, Expr, Helper, IfStatement, MemberAccess, Model, NamedPrompt,
    OutputConfig, OutputShape, Statement, ToolFunction, TypeConfigDecl, TypeDeclaration, TypeExpr,
    WorkflowConfig,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompletionItemKind {
    Keyword,
    Variable,
    Field,
    Tool,
    Helper,
    Prompt,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: CompletionItemKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ValueType {
    Scalar(String),
    Array(Box<ValueType>),
    Object(BTreeMap<String, ValueType>),
    Union(Vec<ValueType>),
    Unknown,
}

impl ValueType {
    pub(crate) fn format(&self) -> String {
        match self {
            ValueType::Scalar(name) => name.clone(),
            ValueType::Array(inner) => format!("{}[]", inner.format()),
            ValueType::Object(fields) => {
                if fields.is_empty() {
                    "{}".to_string()
                } else {
                    let rendered = fields
                        .iter()
                        .map(|(name, ty)| format!("{}: {}", name, ty.format()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{{ {} }}", rendered)
                }
            }
            ValueType::Union(options) => options
                .iter()
                .map(ValueType::format)
                .collect::<Vec<_>>()
                .join(" | "),
            ValueType::Unknown => "unknown".to_string(),
        }
    }

    pub(crate) fn member(&self, name: &str) -> Option<&ValueType> {
        match self {
            ValueType::Object(fields) => fields.get(name),
            ValueType::Array(inner) => inner.member(name),
            ValueType::Union(options) => options.iter().find_map(|option| option.member(name)),
            _ => None,
        }
    }

    pub(crate) fn members(&self) -> Vec<(String, ValueType)> {
        match self {
            ValueType::Object(fields) => fields
                .iter()
                .map(|(name, ty)| (name.clone(), ty.clone()))
                .collect(),
            ValueType::Array(inner) => inner.members(),
            ValueType::Union(options) => {
                let mut merged = BTreeMap::new();
                for option in options {
                    for (name, ty) in option.members() {
                        merged.entry(name).or_insert(ty);
                    }
                }
                merged.into_iter().collect()
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Scope {
    pub variables: BTreeMap<String, ValueType>,
    pub context_fields: BTreeMap<String, ValueType>,
    pub tools: BTreeMap<String, String>,
    pub helpers: BTreeMap<String, String>,
    pub prompts: BTreeMap<String, String>,
}

struct CompletionQuery {
    start: usize,
    end: usize,
    prefix: String,
    member_root: Option<String>,
    member_path: Vec<String>,
}

pub(crate) struct ActiveWorkflow<'a> {
    pub workflow: &'a WorkflowConfig,
    pub parent_configs: &'a [AgentConfig],
    pub is_agent: bool,
}

pub fn completions_for_source(file: &Path, source: &str, offset: usize) -> Vec<CompletionItem> {
    let root_path = canonicalize_best_effort(file);
    let query = completion_query(source, offset);
    let context_parsed = parse_source(source);
    let completion_source = completion_parse_source(source, &query);
    let parsed = parse_source(&completion_source);
    let mut merged_elements = parsed.model.elements.clone();
    merged_elements.extend(load_import_elements_best_effort(
        &root_path,
        &parsed.model.imports,
    ));
    let merged_model = Model {
        imports: parsed.model.imports.clone(),
        elements: merged_elements,
    };

    let active_workflow = find_active_workflow(&context_parsed.model, offset)
        .or_else(|| find_active_workflow(&parsed.model, offset));

    if let Some(workflow) = active_workflow {
        let scope = build_scope(&merged_model, workflow, offset);
        if let Some(root) = &query.member_root {
            member_completions(&scope, root, &query.member_path, &query.prefix)
        } else {
            scope_completions(&scope, &query.prefix)
        }
    } else {
        top_level_completions(&query.prefix)
    }
}

fn completion_query(source: &str, offset: usize) -> CompletionQuery {
    let bounded = offset.min(source.len());
    let bytes = source.as_bytes();
    let mut start = bounded;

    while start > 0 {
        let ch = bytes[start - 1] as char;
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            start -= 1;
        } else {
            break;
        }
    }

    let fragment = &source[start..bounded];
    let parts = fragment.split('.').collect::<Vec<_>>();
    if parts.len() > 1 && !parts[0].is_empty() {
        let prefix = parts.last().unwrap_or(&"").to_string();
        let member_path = parts[1..parts.len().saturating_sub(1)]
            .iter()
            .filter(|segment| !segment.is_empty())
            .map(|segment| (*segment).to_string())
            .collect();
        CompletionQuery {
            start,
            end: bounded,
            prefix,
            member_root: Some(parts[0].to_string()),
            member_path,
        }
    } else {
        CompletionQuery {
            start,
            end: bounded,
            prefix: fragment.to_string(),
            member_root: None,
            member_path: Vec::new(),
        }
    }
}

fn completion_parse_source(source: &str, query: &CompletionQuery) -> String {
    let replacement = if let Some(root) = &query.member_root {
        let mut path = root.clone();
        for segment in &query.member_path {
            path.push('.');
            path.push_str(segment);
        }
        path.push('.');
        if query.prefix.is_empty() {
            path.push_str("__auwgent_completion");
        } else {
            path.push_str(&format!("{}__auwgent_completion", query.prefix));
        }
        path
    } else {
        let line_start = source[..query.start]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let line_prefix = &source[line_start..query.start];
        if line_prefix.trim().is_empty() {
            "return __auwgent_completion".to_string()
        } else if query.prefix.is_empty() {
            "__auwgent_completion".to_string()
        } else {
            format!("{}__auwgent_completion", query.prefix)
        }
    };

    let mut sanitized = String::with_capacity(source.len() + replacement.len() + 32);
    sanitized.push_str(&source[..query.start]);
    sanitized.push_str(&replacement);
    sanitized.push_str(&source[query.end..]);
    sanitized
}

pub(crate) fn find_active_workflow(model: &Model, offset: usize) -> Option<ActiveWorkflow<'_>> {
    let mut fallback: Option<ActiveWorkflow<'_>> = None;

    for element in &model.elements {
        match element {
            Element::Agent(agent) => {
                for config in &agent.configs {
                    if let AgentConfig::Workflow(workflow) = config {
                        if contains_offset(workflow.span.start, workflow.span.end, offset) {
                            return Some(ActiveWorkflow {
                                workflow,
                                parent_configs: &agent.configs,
                                is_agent: true,
                            });
                        }

                        if workflow.span.start <= offset {
                            fallback = Some(ActiveWorkflow {
                                workflow,
                                parent_configs: &agent.configs,
                                is_agent: true,
                            });
                        }
                    }
                }
            }
            Element::Helper(helper) => {
                for config in &helper.configs {
                    if let AgentConfig::Workflow(workflow) = config {
                        if contains_offset(workflow.span.start, workflow.span.end, offset) {
                            return Some(ActiveWorkflow {
                                workflow,
                                parent_configs: &helper.configs,
                                is_agent: false,
                            });
                        }

                        if workflow.span.start <= offset {
                            fallback = Some(ActiveWorkflow {
                                workflow,
                                parent_configs: &helper.configs,
                                is_agent: false,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fallback
}

pub(crate) fn build_scope(model: &Model, workflow: ActiveWorkflow<'_>, offset: usize) -> Scope {
    let type_map = type_map(model);
    let helper_map = helper_map(model);
    let tool_returns = tool_return_map(
        workflow.parent_configs,
        &workflow.workflow.tool_configs,
        &type_map,
    );
    let mut scope = Scope::default();
    let mut env = BTreeMap::new();

    for config in workflow.parent_configs {
        match config {
            AgentConfig::Input(input) => match &input.shape {
                auwgent_ast::InputShape::Properties(properties) => {
                    for property in properties {
                        env.insert(
                            property.name.value.clone(),
                            value_type_from_type_expr(&property.ty, &type_map),
                        );
                    }
                }
                auwgent_ast::InputShape::Direct(ty) => {
                    env.insert(
                        "input".to_string(),
                        value_type_from_type_expr(ty, &type_map),
                    );
                }
            },
            AgentConfig::Context(context) => {
                for property in &context.properties {
                    let value = value_type_from_type_expr(&property.ty, &type_map);
                    env.insert(property.name.value.clone(), value.clone());
                    scope
                        .context_fields
                        .insert(property.name.value.clone(), value);
                }
            }
            AgentConfig::Tool(tool) => {
                scope
                    .tools
                    .insert(tool.name.value.clone(), tool_signature(tool, &type_map));
            }
            AgentConfig::Tools(tools) => {
                for tool in tools {
                    scope
                        .tools
                        .insert(tool.name.value.clone(), tool_signature(tool, &type_map));
                }
            }
            AgentConfig::Helpers(helpers) if workflow.is_agent => {
                populate_helpers(&mut scope, helpers, &helper_map, &type_map);
            }
            _ => {}
        }
    }

    for param in &workflow.workflow.params {
        env.insert(
            param.name.value.clone(),
            value_type_from_type_expr(&param.ty, &type_map),
        );
    }

    for prompt in model.elements.iter().filter_map(|element| match element {
        Element::NamedPrompt(prompt) => Some(prompt),
        _ => None,
    }) {
        scope.prompts.insert(
            prompt.name.value.clone(),
            prompt_signature(prompt, &type_map),
        );
    }

    for tool in &workflow.workflow.tool_configs {
        scope
            .tools
            .insert(tool.name.value.clone(), tool_signature(tool, &type_map));
    }

    collect_statements_before_offset(
        &workflow.workflow.body,
        offset,
        &mut env,
        &type_map,
        &helper_map,
        &tool_returns,
    );
    scope.variables = env;
    scope
}

fn populate_helpers(
    scope: &mut Scope,
    helpers: &auwgent_ast::HelpersConfig,
    helper_map: &HashMap<String, Helper>,
    type_map: &HashMap<String, TypeDeclaration>,
) {
    for helper_ref in &helpers.helpers {
        if let Some(helper) = helper_map.get(&helper_ref.name.value) {
            let output = helper_output_type(helper, type_map).format();
            scope.helpers.insert(helper_ref.name.value.clone(), output);
        }
    }
}

fn collect_statements_before_offset(
    statements: &[Statement],
    offset: usize,
    env: &mut BTreeMap<String, ValueType>,
    type_map: &HashMap<String, TypeDeclaration>,
    helper_map: &HashMap<String, Helper>,
    tool_returns: &HashMap<String, ValueType>,
) {
    for statement in statements {
        let span = statement_span(statement);
        if span.end <= offset {
            apply_statement(statement, env, type_map, helper_map, tool_returns);
            continue;
        }

        if contains_offset(span.start, span.end, offset) {
            match statement {
                Statement::If(if_statement) => {
                    descend_into_if(
                        if_statement,
                        offset,
                        env,
                        type_map,
                        helper_map,
                        tool_returns,
                    );
                }
                Statement::Parallel(parallel) => {
                    collect_statements_before_offset(
                        &parallel.body,
                        offset,
                        env,
                        type_map,
                        helper_map,
                        tool_returns,
                    );
                }
                _ => {}
            }
        }
        break;
    }
}

fn descend_into_if(
    if_statement: &IfStatement,
    offset: usize,
    env: &mut BTreeMap<String, ValueType>,
    type_map: &HashMap<String, TypeDeclaration>,
    helper_map: &HashMap<String, Helper>,
    tool_returns: &HashMap<String, ValueType>,
) {
    if if_statement.then_block.iter().any(|statement| {
        contains_offset(
            statement_span(statement).start,
            statement_span(statement).end,
            offset,
        )
    }) {
        collect_statements_before_offset(
            &if_statement.then_block,
            offset,
            env,
            type_map,
            helper_map,
            tool_returns,
        );
        return;
    }

    if if_statement.else_block.iter().any(|statement| {
        contains_offset(
            statement_span(statement).start,
            statement_span(statement).end,
            offset,
        )
    }) {
        collect_statements_before_offset(
            &if_statement.else_block,
            offset,
            env,
            type_map,
            helper_map,
            tool_returns,
        );
    }
}

fn apply_statement(
    statement: &Statement,
    env: &mut BTreeMap<String, ValueType>,
    type_map: &HashMap<String, TypeDeclaration>,
    helper_map: &HashMap<String, Helper>,
    tool_returns: &HashMap<String, ValueType>,
) {
    match statement {
        Statement::Let(let_statement) => {
            let value = let_statement
                .ty
                .as_ref()
                .map(|ty| value_type_from_type_expr(ty, type_map))
                .unwrap_or_else(|| {
                    infer_expr_type(
                        &let_statement.value,
                        env,
                        type_map,
                        helper_map,
                        tool_returns,
                    )
                });
            env.insert(let_statement.name.value.clone(), value);
        }
        Statement::Assign(assign) => {
            let value = infer_expr_type(&assign.value, env, type_map, helper_map, tool_returns);
            env.insert(assign.variable.value.clone(), value);
        }
        Statement::Parallel(parallel) => {
            collect_statements_before_offset(
                &parallel.body,
                usize::MAX,
                env,
                type_map,
                helper_map,
                tool_returns,
            );
        }
        Statement::Return(_) | Statement::If(_) | Statement::Transfer(_) => {}
    }
}

pub(crate) fn infer_expr_type(
    expr: &Expr,
    env: &BTreeMap<String, ValueType>,
    type_map: &HashMap<String, TypeDeclaration>,
    helper_map: &HashMap<String, Helper>,
    tool_returns: &HashMap<String, ValueType>,
) -> ValueType {
    match expr {
        Expr::StringLit(_) | Expr::MultilineStringLit(_) | Expr::InlinePrompt(_) => {
            ValueType::Scalar("string".to_string())
        }
        Expr::NumberLit(_) => ValueType::Scalar("number".to_string()),
        Expr::BooleanLit(_) => ValueType::Scalar("boolean".to_string()),
        Expr::Array(array) => array
            .elements
            .first()
            .map(|expr| {
                ValueType::Array(Box::new(infer_expr_type(
                    expr,
                    env,
                    type_map,
                    helper_map,
                    tool_returns,
                )))
            })
            .unwrap_or_else(|| ValueType::Array(Box::new(ValueType::Unknown))),
        Expr::Object(object) => {
            let mut fields = BTreeMap::new();
            for property in &object.properties {
                fields.insert(
                    property.name.value.clone(),
                    property
                        .value
                        .as_ref()
                        .map(|value| {
                            infer_expr_type(value, env, type_map, helper_map, tool_returns)
                        })
                        .unwrap_or(ValueType::Unknown),
                );
            }
            ValueType::Object(fields)
        }
        Expr::VarRef(name) => env.get(&name.value).cloned().unwrap_or(ValueType::Unknown),
        Expr::MemberAccess(access) => resolve_member_access(access, env),
        Expr::IndexAccess(index) => match env.get(&index.object.value) {
            Some(ValueType::Array(inner)) => (*inner.clone()).clone(),
            _ => ValueType::Unknown,
        },
        Expr::BinaryOp(binary) => {
            let left = infer_expr_type(&binary.left, env, type_map, helper_map, tool_returns);
            let right = infer_expr_type(&binary.right, env, type_map, helper_map, tool_returns);
            match binary.op {
                BinOperator::Add if left.format() == "string" || right.format() == "string" => {
                    ValueType::Scalar("string".to_string())
                }
                _ => ValueType::Scalar("number".to_string()),
            }
        }
        Expr::FunctionCall(call) => tool_returns
            .get(&call.name.value)
            .cloned()
            .unwrap_or(ValueType::Unknown),
        Expr::HelperCall(call) => helper_map
            .get(&call.helper.value)
            .map(|helper| helper_output_type(helper, type_map))
            .unwrap_or(ValueType::Unknown),
        Expr::PromptCall(_) => ValueType::Scalar("string".to_string()),
        Expr::ContextRef(context) => env
            .get(&context.property.value)
            .cloned()
            .unwrap_or(ValueType::Unknown),
        Expr::Grouped(inner, _) => infer_expr_type(inner, env, type_map, helper_map, tool_returns),
    }
}

fn tool_return_map(
    parent_configs: &[AgentConfig],
    workflow_tools: &[ToolFunction],
    type_map: &HashMap<String, TypeDeclaration>,
) -> HashMap<String, ValueType> {
    let mut returns = HashMap::new();

    for config in parent_configs {
        match config {
            AgentConfig::Tool(tool) => {
                returns.insert(
                    tool.name.value.clone(),
                    tool.returns
                        .as_ref()
                        .map(|ty| value_type_from_type_expr(ty, type_map))
                        .unwrap_or(ValueType::Unknown),
                );
            }
            AgentConfig::Tools(tools) => {
                for tool in tools {
                    returns.insert(
                        tool.name.value.clone(),
                        tool.returns
                            .as_ref()
                            .map(|ty| value_type_from_type_expr(ty, type_map))
                            .unwrap_or(ValueType::Unknown),
                    );
                }
            }
            _ => {}
        }
    }

    for tool in workflow_tools {
        returns.insert(
            tool.name.value.clone(),
            tool.returns
                .as_ref()
                .map(|ty| value_type_from_type_expr(ty, type_map))
                .unwrap_or(ValueType::Unknown),
        );
    }

    returns
}

fn resolve_member_access(access: &MemberAccess, env: &BTreeMap<String, ValueType>) -> ValueType {
    let mut current = env
        .get(&access.object.value)
        .cloned()
        .unwrap_or(ValueType::Unknown);
    let mut path = vec![access.property.value.as_str()];
    for segment in &access.chain {
        path.push(segment.value.as_str());
    }

    for segment in path {
        current = current
            .member(segment)
            .cloned()
            .unwrap_or(ValueType::Unknown);
    }
    current
}

fn member_completions(
    scope: &Scope,
    root: &str,
    chain: &[String],
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut base = if root == "ctx" {
        ValueType::Object(scope.context_fields.clone())
    } else {
        scope
            .variables
            .get(root)
            .cloned()
            .unwrap_or(ValueType::Unknown)
    };

    for segment in chain {
        base = base.member(segment).cloned().unwrap_or(ValueType::Unknown);
    }

    let mut items = base
        .members()
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, ty)| CompletionItem {
            label: name,
            detail: Some(ty.format()),
            kind: if root == "ctx" {
                CompletionItemKind::Context
            } else {
                CompletionItemKind::Field
            },
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}

fn scope_completions(scope: &Scope, prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    items.extend(
        [
            keyword_completion("let", prefix),
            keyword_completion("return", prefix),
            keyword_completion("if", prefix),
            keyword_completion("transfer", prefix),
            keyword_completion("parallel", prefix),
            keyword_completion("true", prefix),
            keyword_completion("false", prefix),
            keyword_completion("ctx", prefix),
            keyword_completion("string", prefix),
            keyword_completion("number", prefix),
            keyword_completion("boolean", prefix),
            keyword_completion("Text", prefix),
        ]
        .into_iter()
        .flatten(),
    );

    items.extend(
        scope
            .variables
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, ty)| CompletionItem {
                label: name.clone(),
                detail: Some(ty.format()),
                kind: CompletionItemKind::Variable,
            }),
    );
    items.extend(
        scope
            .tools
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, detail)| CompletionItem {
                label: name.clone(),
                detail: Some(detail.clone()),
                kind: CompletionItemKind::Tool,
            }),
    );
    items.extend(
        scope
            .helpers
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, detail)| CompletionItem {
                label: name.clone(),
                detail: Some(format!("helper -> {}", detail)),
                kind: CompletionItemKind::Helper,
            }),
    );
    items.extend(
        scope
            .prompts
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, detail)| CompletionItem {
                label: name.clone(),
                detail: Some(detail.clone()),
                kind: CompletionItemKind::Prompt,
            }),
    );

    dedupe_and_sort(items)
}

fn top_level_completions(prefix: &str) -> Vec<CompletionItem> {
    let items = [
        "agent", "helper", "type", "prompt", "model", "import", "export",
    ]
    .into_iter()
    .filter_map(|keyword| keyword_completion(keyword, prefix))
    .collect::<Vec<_>>();
    dedupe_and_sort(items)
}

fn keyword_completion(keyword: &str, prefix: &str) -> Option<CompletionItem> {
    if keyword.starts_with(prefix) {
        Some(CompletionItem {
            label: keyword.to_string(),
            detail: None,
            kind: CompletionItemKind::Keyword,
        })
    } else {
        None
    }
}

fn dedupe_and_sort(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    let mut seen = HashSet::new();
    let mut unique = items
        .into_iter()
        .filter(|item| seen.insert((item.label.clone(), item.kind.clone())))
        .collect::<Vec<_>>();
    unique.sort_by(|left, right| left.label.cmp(&right.label));
    unique
}

pub(crate) fn type_map(model: &Model) -> HashMap<String, TypeDeclaration> {
    model
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::TypeDecl(ty) => Some((ty.name.value.clone(), ty.clone())),
            _ => None,
        })
        .collect()
}

pub(crate) fn helper_map(model: &Model) -> HashMap<String, Helper> {
    model
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::Helper(helper) => Some((helper.name.value.clone(), helper.clone())),
            _ => None,
        })
        .collect()
}

pub(crate) fn helper_output_type(
    helper: &Helper,
    type_map: &HashMap<String, TypeDeclaration>,
) -> ValueType {
    for config in &helper.configs {
        if let AgentConfig::Output(output) = config {
            return value_type_from_output(output, type_map);
        }
    }
    ValueType::Unknown
}

pub(crate) fn value_type_from_output(
    output: &OutputConfig,
    type_map: &HashMap<String, TypeDeclaration>,
) -> ValueType {
    match &output.shape {
        OutputShape::Properties(properties) => {
            let mut fields = BTreeMap::new();
            for property in properties {
                fields.insert(
                    property.decl.name.value.clone(),
                    value_type_from_type_expr(&property.decl.ty, type_map),
                );
            }
            ValueType::Object(fields)
        }
        OutputShape::Union(options) => ValueType::Union(
            options
                .iter()
                .map(|option| ValueType::Scalar(option.value.clone()))
                .collect(),
        ),
        OutputShape::Direct { ty, .. } => value_type_from_type_expr(ty, type_map),
    }
}

pub(crate) fn value_type_from_type_expr(
    ty: &TypeExpr,
    type_map: &HashMap<String, TypeDeclaration>,
) -> ValueType {
    match ty {
        TypeExpr::String(_) => ValueType::Scalar("string".to_string()),
        TypeExpr::Text(_) => ValueType::Scalar("Text".to_string()),
        TypeExpr::Number(_) => ValueType::Scalar("number".to_string()),
        TypeExpr::Boolean(_) => ValueType::Scalar("boolean".to_string()),
        TypeExpr::Array { element, .. } => {
            ValueType::Array(Box::new(value_type_from_type_expr(element, type_map)))
        }
        TypeExpr::Object { properties, .. } => {
            let mut fields = BTreeMap::new();
            for property in properties {
                fields.insert(
                    property.name.value.clone(),
                    value_type_from_type_expr(&property.ty, type_map),
                );
            }
            ValueType::Object(fields)
        }
        TypeExpr::TypeRef(name) => type_map
            .get(&name.value)
            .map(|declaration| {
                let mut fields = BTreeMap::new();
                for field in &declaration.fields {
                    fields.insert(
                        field.name.value.clone(),
                        value_type_from_type_expr(&field.ty, type_map),
                    );
                }
                ValueType::Object(fields)
            })
            .unwrap_or_else(|| ValueType::Scalar(name.value.clone())),
        TypeExpr::Union { options, .. } => ValueType::Union(
            options
                .iter()
                .map(|option| ValueType::Scalar(option.value.clone()))
                .collect(),
        ),
    }
}

pub(crate) fn tool_signature(
    tool: &ToolFunction,
    type_map: &HashMap<String, TypeDeclaration>,
) -> String {
    let params = tool
        .params
        .iter()
        .map(|param| type_config_signature(param, type_map))
        .collect::<Vec<_>>()
        .join(", ");
    let returns = tool
        .returns
        .as_ref()
        .map(|ty| value_type_from_type_expr(ty, type_map).format())
        .unwrap_or_else(|| "unknown".to_string());
    format!("({params}) -> {returns}")
}

pub(crate) fn prompt_signature(
    prompt: &NamedPrompt,
    type_map: &HashMap<String, TypeDeclaration>,
) -> String {
    let params = prompt
        .params
        .iter()
        .map(|param| type_config_signature(param, type_map))
        .collect::<Vec<_>>()
        .join(", ");
    format!("prompt({params})")
}

fn type_config_signature(
    config: &TypeConfigDecl,
    type_map: &HashMap<String, TypeDeclaration>,
) -> String {
    format!(
        "{}: {}",
        config.name.value,
        value_type_from_type_expr(&config.ty, type_map).format()
    )
}

pub(crate) fn statement_span(statement: &Statement) -> auwgent_errors::Span {
    match statement {
        Statement::Let(statement) => statement.span,
        Statement::Assign(statement) => statement.span,
        Statement::Return(statement) => statement.span,
        Statement::If(statement) => statement.span,
        Statement::Transfer(statement) => statement.span,
        Statement::Parallel(statement) => statement.span,
    }
}

pub(crate) fn contains_offset(start: usize, end: usize, offset: usize) -> bool {
    start <= offset && offset <= end
}

#[cfg(test)]
mod tests {
    use super::completions_for_source;

    #[test]
    fn completes_member_access_from_object_literal() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_completion_member_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file = base.join("main.agent");
        let source = r#"
agent Demo {
    workflow run(name: string): string {
        description: "run"
        let student = { profile: { first: name, age: 20 } }
        return student.profile.
    }
}
"#;
        std::fs::write(&file, source).unwrap();
        let offset = source.find("student.profile.").unwrap() + "student.profile.".len();
        let items = completions_for_source(&file, source, offset);

        assert!(items.iter().any(|item| item.label == "age"));
        assert!(items.iter().any(|item| item.label == "first"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn completes_scope_symbols_in_workflow() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_completion_scope_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file = base.join("main.agent");
        let source = r#"
export prompt SharedPrompt {
    "shared"
}

helper Researcher {
    description: "research helper"
    output {
        summary: string
    }
}

agent Demo {
    context {
        project: string
    }

    helpers {
        Researcher
    }

    tool lookup(id: string): { id: string }

    workflow run(id: string): string {
        description: "run"
        let result = lookup(id)
        ret
    }
}
"#;
        std::fs::write(&file, source).unwrap();
        let offset = source.find("ret").unwrap();
        let items = completions_for_source(&file, source, offset);
        let labels = items
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();

        assert!(
            items.iter().any(|item| item.label == "return"),
            "labels: {:?}",
            labels
        );
        assert!(
            items.iter().any(|item| item.label == "result"),
            "labels: {:?}",
            labels
        );
        assert!(
            items.iter().any(|item| item.label == "lookup"),
            "labels: {:?}",
            labels
        );
        assert!(
            items.iter().any(|item| item.label == "Researcher"),
            "labels: {:?}",
            labels
        );
        assert!(
            items.iter().any(|item| item.label == "SharedPrompt"),
            "labels: {:?}",
            labels
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn filters_scope_completions_by_prefix() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_completion_prefix_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file = base.join("main.agent");
        let source = r#"
agent Demo {
    workflow run(): string {
        description: "run"
        let result = "ok"
        ret
    }
}
"#;
        std::fs::write(&file, source).unwrap();
        let offset = source.find("ret").unwrap() + "ret".len();
        let items = completions_for_source(&file, source, offset);
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["return"]);

        let _ = std::fs::remove_dir_all(&base);
    }
}
