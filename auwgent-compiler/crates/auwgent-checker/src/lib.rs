//! # auwgent-checker
//!
//! Type system, type checker, and validation passes for the Auwgent DSL.
//! Ported from `checker.ts` — validates workflows, prompts, model configs,
//! and type consistency across the agent.

use auwgent_ast::*;
use auwgent_errors::{Diagnostic, Span};
use std::collections::HashMap;

// ── Internal Type Algebra ────────────────────────────────────────────────

/// Internal type representation for the checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Const(String), // "string", "number", "boolean"
    Array(Box<Type>),
    Record {
        fields: HashMap<String, Type>,
        optional: HashMap<String, bool>,
    },
    Union(Vec<Type>),
    Error(String),
}

impl Type {
    pub fn string() -> Self {
        Type::Const("string".into())
    }
    pub fn number() -> Self {
        Type::Const("number".into())
    }
    pub fn boolean() -> Self {
        Type::Const("boolean".into())
    }
    pub fn error(msg: &str) -> Self {
        Type::Error(msg.into())
    }

    pub fn format(&self) -> String {
        match self {
            Type::Const(n) => n.clone(),
            Type::Array(el) => format!("{}[]", el.format()),
            Type::Record { fields, optional } => {
                let f: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| {
                        let opt = if *optional.get(k).unwrap_or(&false) {
                            "?"
                        } else {
                            ""
                        };
                        format!("{}{}: {}", k, opt, v.format())
                    })
                    .collect();
                if f.is_empty() {
                    "{}".into()
                } else {
                    format!("{{ {} }}", f.join(", "))
                }
            }
            Type::Union(opts) => opts
                .iter()
                .map(|t| t.format())
                .collect::<Vec<_>>()
                .join(" | "),
            Type::Error(msg) => format!("error({})", msg),
        }
    }
}

// ── Type Environment ─────────────────────────────────────────────────────

struct TypeEnv {
    vars: HashMap<String, Type>,
}

impl TypeEnv {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    fn set(&mut self, name: &str, ty: Type) {
        self.vars.insert(name.to_string(), ty);
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.vars.get(name)
    }

    fn extend(&self) -> TypeEnv {
        TypeEnv {
            vars: self.vars.clone(),
        }
    }
}

// ── Main Check Entry ─────────────────────────────────────────────────────

/// Run all validation and type-checking passes on a parsed model.
/// Returns a list of diagnostics (errors, warnings).
pub fn check(model: &Model) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut checker = Checker::new(model);

    // Collect types and prompts
    checker.collect_declarations(model);

    for element in &model.elements {
        match element {
            Element::Agent(agent) => checker.check_agent(agent, &mut diags),
            Element::Helper(helper) => checker.check_helper(helper, &mut diags),
            Element::TypeDecl(td) => checker.check_type_decl(td, &mut diags),
            Element::NamedPrompt(p) => checker.check_named_prompt(p, &mut diags),
            Element::ModelDef(_) => {}
        }
    }

    diags
}

// ── Checker Struct ───────────────────────────────────────────────────────

struct Checker {
    type_map: HashMap<String, Vec<TypeConfigDecl>>,
    prompt_map: HashMap<String, Vec<TypeConfigDecl>>,
    tool_map: HashMap<String, ToolFunction>,
    /// Context fields for validating ctx.property references
    context_fields: HashMap<String, Span>,
}

impl Checker {
    fn new(_model: &Model) -> Self {
        Self {
            type_map: HashMap::new(),
            prompt_map: HashMap::new(),
            tool_map: HashMap::new(),
            context_fields: HashMap::new(),
        }
    }

    fn collect_declarations(&mut self, model: &Model) {
        for el in &model.elements {
            match el {
                Element::TypeDecl(td) => {
                    self.type_map
                        .insert(td.name.value.clone(), td.fields.clone());
                }
                Element::NamedPrompt(p) => {
                    self.prompt_map
                        .insert(p.name.value.clone(), p.params.clone());
                }
                _ => {}
            }
        }
    }

    // ── Agent / Helper Validation ────────────────────────────────────

    fn check_agent(&mut self, agent: &Agent, diags: &mut Vec<Diagnostic>) {
        let mut has_model_config = false;
        let mut tool_names: Vec<String> = Vec::new();

        // Pass 1: Collect all tools first so helper grants can reference them
        for config in &agent.configs {
            match config {
                AgentConfig::Model(_) => has_model_config = true,
                AgentConfig::Tool(tf) => {
                    if tool_names.contains(&tf.name.value) {
                        diags.push(
                            Diagnostic::error(
                                format!("Duplicate tool name '{}'", tf.name.value),
                                tf.name.span,
                            )
                            .with_help(format!(
                                "Each tool must have a unique name. Rename one of the '{}' tools.",
                                tf.name.value
                            )),
                        );
                    }
                    tool_names.push(tf.name.value.clone());
                    self.tool_map.insert(tf.name.value.clone(), tf.clone());
                }
                AgentConfig::Tools(tfs) => {
                    for tf in tfs {
                        if tool_names.contains(&tf.name.value) {
                            diags.push(Diagnostic::error(
                                format!("Duplicate tool name '{}'", tf.name.value),
                                tf.name.span,
                            ));
                        }
                        tool_names.push(tf.name.value.clone());
                        self.tool_map.insert(tf.name.value.clone(), tf.clone());
                    }
                }
                _ => {}
            }
        }

        // Collect context fields for ctx.property validation *before* other checks
        self.context_fields.clear();
        for config in &agent.configs {
            if let AgentConfig::Context(cc) = config {
                for p in &cc.properties {
                    self.context_fields
                        .insert(p.name.value.clone(), p.name.span);
                }
            }
        }

        // Pass 2: Validate everything (tools are now in tool_map)
        for config in &agent.configs {
            match config {
                AgentConfig::Model(mc) => {
                    // Validate prompt args
                    if let Some(expr) = &mc.default_config.prompt_expr {
                        self.infer_expression(expr, &TypeEnv::new(), diags);
                    }
                    for nc in &mc.named_configs {
                        if let Some(expr) = &nc.config.prompt_expr {
                            self.infer_expression(expr, &TypeEnv::new(), diags);
                        }
                    }
                }
                AgentConfig::Tool(tf) => self.check_tool(tf, diags),
                AgentConfig::Tools(tfs) => {
                    for tf in tfs {
                        self.check_tool(tf, diags);
                    }
                }
                AgentConfig::Workflow(wf) => self.check_workflow(wf, &agent.configs, diags),
                AgentConfig::Input(ic) => self.check_properties(&ic.properties, diags),
                AgentConfig::Output(oc) => self.check_output(oc, diags),
                AgentConfig::Context(cc) => self.check_properties(&cc.properties, diags),
                AgentConfig::Helpers(hc) => self.check_helpers_config(hc, diags),
                _ => {}
            }
        }

        if !has_model_config {
            diags.push(Diagnostic::error(
                format!("Agent '{}' is missing a model configuration", agent.name.value),
                agent.name.span,
            ).with_help(
                "Add a default config block:\n  default config {\n    model: gemini(\"gemini-2.5-flash\")\n    prompt { ... }\n  }"
            ));
        }
    }

    fn check_helper(&mut self, helper: &Helper, diags: &mut Vec<Diagnostic>) {
        let mut has_model_config = false;

        // Collect context fields for ctx.property validation *before* other checks
        self.context_fields.clear();
        for config in &helper.configs {
            if let AgentConfig::Context(cc) = config {
                for p in &cc.properties {
                    self.context_fields
                        .insert(p.name.value.clone(), p.name.span);
                }
            }
        }

        for config in &helper.configs {
            match config {
                AgentConfig::Model(mc) => {
                    has_model_config = true;
                    if let Some(expr) = &mc.default_config.prompt_expr {
                        self.infer_expression(expr, &TypeEnv::new(), diags);
                    }
                    for nc in &mc.named_configs {
                        if let Some(expr) = &nc.config.prompt_expr {
                            self.infer_expression(expr, &TypeEnv::new(), diags);
                        }
                    }
                }
                AgentConfig::Input(ic) => self.check_properties(&ic.properties, diags),
                AgentConfig::Output(oc) => self.check_output(oc, diags),
                AgentConfig::Context(cc) => self.check_properties(&cc.properties, diags),
                AgentConfig::Tool(tf) => self.check_tool(tf, diags),
                AgentConfig::Tools(tfs) => {
                    for tf in tfs {
                        self.check_tool(tf, diags);
                    }
                }
                AgentConfig::Workflow(wf) => self.check_workflow(wf, &helper.configs, diags),
                _ => {}
            }
        }

        if !has_model_config {
            diags.push(Diagnostic::error(
                format!("Helper '{}' is missing a model configuration", helper.name.value),
                helper.name.span,
            ).with_help(
                "Add a default config block:\n  default config {\n    model: gemini(\"gemini-2.5-flash\")\n    prompt: \"Your prompt here\"\n  }"
            ));
        }
    }

    fn check_helpers_config(&self, hc: &HelpersConfig, diags: &mut Vec<Diagnostic>) {
        for href in &hc.helpers {
            // Check granted tools exist
            for tool_name in &href.granted_tools {
                if !self.tool_map.contains_key(&tool_name.value) {
                    diags.push(
                        Diagnostic::error(
                            format!("Unknown tool '{}' in helper grant", tool_name.value),
                            tool_name.span,
                        )
                        .with_help(format!(
                            "Available tools: {}",
                            self.tool_map.keys().cloned().collect::<Vec<_>>().join(", ")
                        )),
                    );
                }
            }
        }
    }

    // ── Tool Validation ──────────────────────────────────────────────

    fn check_tool(&self, tf: &ToolFunction, diags: &mut Vec<Diagnostic>) {
        if tf.description.is_empty() {
            diags.push(Diagnostic::warning(
                format!("Tool '{}' has no description", tf.name.value),
                tf.name.span,
            ).with_help(
                "Add a description so the AI model knows when to use this tool:\n  getStudentDetails(...): Student @desc \"Gets student info by ID\""
            ));
        }

        self.check_type_ref_exists(&tf.returns, diags);
        for p in &tf.params {
            self.check_type_ref_exists(&p.ty, diags);
        }
    }

    // ── Type Validation ──────────────────────────────────────────────

    fn check_type_decl(&self, td: &TypeDeclaration, diags: &mut Vec<Diagnostic>) {
        if td.fields.is_empty() {
            diags.push(
                Diagnostic::warning(
                    format!("Type '{}' has no fields", td.name.value),
                    td.name.span,
                )
                .with_help("Add at least one field to make this type useful."),
            );
        }

        // Check for duplicate field names
        let mut seen: HashMap<String, Span> = HashMap::new();
        for field in &td.fields {
            if let Some(prev_span) = seen.get(&field.name.value) {
                diags.push(
                    Diagnostic::error(
                        format!(
                            "Duplicate field '{}' in type '{}'",
                            field.name.value, td.name.value
                        ),
                        field.name.span,
                    )
                    .with_label(*prev_span, "first defined here"),
                );
            } else {
                seen.insert(field.name.value.clone(), field.name.span);
            }
            self.check_type_ref_exists(&field.ty, diags);
        }
    }

    fn check_type_ref_exists(&self, ty: &TypeExpr, diags: &mut Vec<Diagnostic>) {
        match ty {
            TypeExpr::TypeRef(name) => {
                if !self.type_map.contains_key(&name.value) {
                    // Check built-in types for "did you mean?" suggestions
                    let builtins = ["string", "number", "boolean"];
                    let all_types: Vec<&str> = builtins
                        .iter()
                        .copied()
                        .chain(self.type_map.keys().map(|s| s.as_str()))
                        .collect();

                    let suggestion = find_closest(&name.value, &all_types);
                    let help = if let Some(suggest) = suggestion {
                        format!("Did you mean '{}'?", suggest)
                    } else {
                        let available = if self.type_map.is_empty() {
                            "string, number, boolean".to_string()
                        } else {
                            let mut types: Vec<String> =
                                builtins.iter().map(|s| s.to_string()).collect();
                            types.extend(self.type_map.keys().cloned());
                            types.join(", ")
                        };
                        format!("Available types: {}. Define a custom type with:\n  type {} {{\n    field: string\n  }}", available, name.value)
                    };

                    diags.push(
                        Diagnostic::error(format!("Unknown type '{}'", name.value), name.span)
                            .with_help(help),
                    );
                }
            }
            TypeExpr::Array { element, .. } => self.check_type_ref_exists(element, diags),
            TypeExpr::Object { properties, .. } => {
                for p in properties {
                    self.check_type_ref_exists(&p.ty, diags);
                }
            }
            _ => {}
        }
    }

    fn check_properties(&self, props: &[TypeConfigDecl], diags: &mut Vec<Diagnostic>) {
        let mut seen: HashMap<String, Span> = HashMap::new();
        for p in props {
            if let Some(prev_span) = seen.get(&p.name.value) {
                diags.push(
                    Diagnostic::error(
                        format!("Duplicate property '{}'", p.name.value),
                        p.name.span,
                    )
                    .with_label(*prev_span, "first defined here"),
                );
            } else {
                seen.insert(p.name.value.clone(), p.name.span);
            }
            self.check_type_ref_exists(&p.ty, diags);
        }
    }

    fn check_output(&self, oc: &OutputConfig, diags: &mut Vec<Diagnostic>) {
        match &oc.shape {
            OutputShape::Properties(props) => {
                for p in props {
                    self.check_type_ref_exists(&p.decl.ty, diags);
                }
            }
            OutputShape::Direct { ty, .. } => self.check_type_ref_exists(ty, diags),
            OutputShape::Union(types) => {
                for t in types {
                    if !self.type_map.contains_key(&t.value) {
                        diags.push(Diagnostic::error(
                            format!("Unknown type '{}' in output union", t.value),
                            t.span,
                        ));
                    }
                }
            }
        }
    }

    fn check_named_prompt(&self, prompt: &NamedPrompt, diags: &mut Vec<Diagnostic>) {
        // Collect declared param names
        let param_names: Vec<&str> = prompt
            .params
            .iter()
            .map(|p| p.name.value.as_str())
            .collect();

        // Scan prompt body for {{var}} template references and validate them
        for stmt in &prompt.body {
            if let PromptStatement::Expr(expr) = stmt {
                self.check_prompt_template_vars(expr, &param_names, diags);
            }
        }
    }

    /// Check that {{var}} references inside prompt strings match declared params.
    fn check_prompt_template_vars(
        &self,
        expr: &Expr,
        params: &[&str],
        diags: &mut Vec<Diagnostic>,
    ) {
        let text = match expr {
            Expr::StringLit(s) => Some((&s.value, s.span)),
            Expr::MultilineStringLit(s) => Some((&s.value, s.span)),
            _ => None,
        };
        if let Some((text, span)) = text {
            // Find all {{var_name}} references
            let mut i = 0;
            let bytes = text.as_bytes();
            while i + 3 < bytes.len() {
                if bytes[i] == b'{' && bytes[i + 1] == b'{' {
                    if let Some(end) = text[i + 2..].find("}}") {
                        let var_name = text[i + 2..i + 2 + end].trim();

                        // Ignore built-in @schema directive but validate arguments
                        if var_name.starts_with("@schema(") && var_name.ends_with(')') {
                            let schema_args = &var_name[8..var_name.len() - 1];
                            if schema_args != "input" && schema_args != "output" {
                                diags.push(
                                    Diagnostic::error(
                                        format!("Invalid schema argument '{}'", schema_args),
                                        span,
                                    )
                                    .with_help("The @schema directive only supports 'input' or 'output' as arguments.")
                                );
                            }
                            i = i + 2 + end + 2;
                            continue;
                        }

                        if let Some(condition) = var_name.strip_prefix("#if") {
                            self.check_template_condition_vars(condition.trim(), params, span, diags);
                            i = i + 2 + end + 2;
                            continue;
                        }

                        if var_name == "else" || var_name == "/if" {
                            i = i + 2 + end + 2;
                            continue;
                        }

                        if !params.contains(&var_name) {
                            let suggestion = find_closest(var_name, params);
                            let help = if let Some(s) = suggestion {
                                format!("Did you mean '{}'?", s)
                            } else {
                                format!("Available params: {}", params.join(", "))
                            };
                            diags.push(
                                Diagnostic::error(
                                    format!(
                                        "Unknown template variable '{{{{{}}}}}' in prompt",
                                        var_name
                                    ),
                                    span,
                                )
                                .with_help(help),
                            );
                        }
                        i = i + 2 + end + 2;
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }

    fn check_template_condition_vars(
        &self,
        condition: &str,
        params: &[&str],
        span: Span,
        diags: &mut Vec<Diagnostic>,
    ) {
        let mut seen: Vec<String> = Vec::new();
        for reference in extract_template_condition_refs(condition) {
            if seen.iter().any(|existing| existing == &reference) {
                continue;
            }
            seen.push(reference.clone());

            if !params.contains(&reference.as_str()) {
                let suggestion = find_closest(&reference, params);
                let help = if let Some(s) = suggestion {
                    format!("Did you mean '{}' ?", s)
                } else {
                    format!("Available params: {}", params.join(", "))
                };
                diags.push(
                    Diagnostic::error(
                        format!(
                            "Unknown template variable '{{{{#if {}}}}}' in prompt",
                            condition
                        ),
                        span,
                    )
                    .with_help(help),
                );
            }
        }
    }

    // ── Workflow Validation ──────────────────────────────────────────

    fn check_workflow(
        &self,
        wf: &WorkflowConfig,
        parent_configs: &[AgentConfig],
        diags: &mut Vec<Diagnostic>,
    ) {
        // Build env from agent/helper input+context
        let mut env = TypeEnv::new();
        for config in parent_configs {
            match config {
                AgentConfig::Input(ic) => {
                    for p in &ic.properties {
                        env.set(&p.name.value, self.map_type_expr(&p.ty));
                    }
                }
                AgentConfig::Context(cc) => {
                    for p in &cc.properties {
                        env.set(&p.name.value, self.map_type_expr(&p.ty));
                    }
                }
                _ => {}
            }
        }

        // Add workflow params to env
        for p in &wf.params {
            env.set(&p.name.value, self.map_type_expr(&p.ty));
        }

        let expected = self.map_type_expr(&wf.return_type);

        // Check workflow tools exist
        for tf in &wf.tool_configs {
            self.check_tool(tf, diags);
        }

        // Check statements
        self.check_statements(&wf.body, &mut env, &expected, diags);
    }

    fn check_statements(
        &self,
        stmts: &[Statement],
        env: &mut TypeEnv,
        expected_return: &Type,
        diags: &mut Vec<Diagnostic>,
    ) {
        for stmt in stmts {
            match stmt {
                Statement::Let(ls) => {
                    let val_ty = self.infer_expression(&ls.value, env, diags);
                    if let Some(declared) = &ls.ty {
                        let decl_ty = self.map_type_expr(declared);
                        if !self.types_compatible(&decl_ty, &val_ty) {
                            diags.push(Diagnostic::error(
                                format!(
                                    "Variable '{}' declared as {} but assigned {}",
                                    ls.name.value,
                                    decl_ty.format(),
                                    val_ty.format()
                                ),
                                ls.name.span,
                            ));
                        }
                        env.set(&ls.name.value, decl_ty);
                    } else {
                        env.set(&ls.name.value, val_ty);
                    }
                }
                Statement::Assign(as_) => {
                    let val_ty = self.infer_expression(&as_.value, env, diags);
                    if env.get(&as_.variable.value).is_none() {
                        diags.push(
                            Diagnostic::error(
                                format!("Unknown variable '{}'", as_.variable.value),
                                as_.variable.span,
                            )
                            .with_help("Use 'let' to declare variables first."),
                        );
                    }
                    env.set(&as_.variable.value, val_ty);
                }
                Statement::Return(rs) => {
                    let actual = self.infer_expression(&rs.value, env, diags);
                    if !self.types_compatible(expected_return, &actual) {
                        diags.push(Diagnostic::error(
                            format!(
                                "Return type mismatch: expected {} but got {}",
                                expected_return.format(),
                                actual.format()
                            ),
                            rs.span,
                        ));
                    }
                }
                Statement::If(ifs) => {
                    self.check_condition(&ifs.condition, env, diags);
                    let mut then_env = env.extend();
                    self.check_statements(&ifs.then_block, &mut then_env, expected_return, diags);
                    if !ifs.else_block.is_empty() {
                        let mut else_env = env.extend();
                        self.check_statements(
                            &ifs.else_block,
                            &mut else_env,
                            expected_return,
                            diags,
                        );
                    }
                }
                Statement::Transfer(_) => {}
                Statement::Parallel(ps) => {
                    self.check_statements(&ps.body, env, expected_return, diags);
                }
            }
        }
    }

    fn check_condition(&self, cond: &Condition, env: &TypeEnv, diags: &mut Vec<Diagnostic>) {
        match cond {
            Condition::Comparison {
                left,
                op,
                right,
                span,
            } => {
                let left_ty = self.infer_expression(left, env, diags);
                let right_ty = self.infer_expression(right, env, diags);

                if let Some(message) = self.validate_comparison_types(op, &left_ty, &right_ty) {
                    diags.push(Diagnostic::error(message, *span));
                }
            }
            Condition::Logical { left, right, .. } => {
                self.check_condition(left, env, diags);
                self.check_condition(right, env, diags);
            }
            Condition::Boolean { value, .. } => {
                self.infer_expression(value, env, diags);
            }
        }
    }

    // ── Expression Type Inference ────────────────────────────────────

    fn infer_expression(&self, expr: &Expr, env: &TypeEnv, diags: &mut Vec<Diagnostic>) -> Type {
        match expr {
            Expr::StringLit(_) | Expr::MultilineStringLit(_) => Type::string(),
            Expr::NumberLit(_) => Type::number(),
            Expr::BooleanLit(_) => Type::boolean(),
            Expr::Array(a) => {
                if a.elements.is_empty() {
                    return Type::Array(Box::new(Type::error("unknown")));
                }
                let first = self.infer_expression(&a.elements[0], env, diags);
                Type::Array(Box::new(first))
            }
            Expr::Object(o) => {
                let mut fields = HashMap::new();
                let optional = HashMap::new();
                for p in &o.properties {
                    let ty = p
                        .value
                        .as_ref()
                        .map(|v| self.infer_expression(v, env, diags))
                        .unwrap_or_else(|| Type::error("unknown"));
                    fields.insert(p.name.value.clone(), ty);
                }
                Type::Record { fields, optional }
            }
            Expr::VarRef(v) => {
                if let Some(ty) = env.get(&v.value) {
                    ty.clone()
                } else if let Some(params) = self.prompt_map.get(&v.value) {
                    if params.is_empty() {
                        Type::string()
                    } else {
                        let signature = params
                            .iter()
                            .map(|param| format!("{}: {}", param.name.value, self.map_type_expr(&param.ty).format()))
                            .collect::<Vec<_>>()
                            .join(", ");

                        diags.push(
                            Diagnostic::error(
                                format!("Prompt '{}' requires arguments", v.value),
                                v.span,
                            )
                            .with_help(format!(
                                "Use prompt {}({}) or {}(...).",
                                v.value, signature, v.value
                            )),
                        );
                        Type::error("missing prompt arguments")
                    }
                } else {
                    diags.push(
                        Diagnostic::error(format!("Unknown variable '{}'", v.value), v.span)
                            .with_help("Check the spelling, or declare it with 'let'."),
                    );
                    Type::error("unknown")
                }
            }
            Expr::FunctionCall(fc) => {
                // Check if it's a tool call
                if let Some(tool) = self.tool_map.get(&fc.name.value) {
                    // Check argument count
                    if fc.args.len() != tool.params.len() {
                        diags.push(Diagnostic::error(
                            format!(
                                "Tool '{}' expects {} argument(s) but got {}",
                                fc.name.value,
                                tool.params.len(),
                                fc.args.len()
                            ),
                            fc.span,
                        ));
                    }
                    self.check_call_args(&fc.args, &tool.params, &fc.name.value, fc.span, env, diags);
                    return self.map_type_expr(&tool.returns);
                }
                // Could be a prompt call
                if let Some(params) = self.prompt_map.get(&fc.name.value) {
                    if fc.args.len() != params.len() {
                        diags.push(Diagnostic::error(
                            format!(
                                "Prompt '{}' expects {} argument(s) but got {}",
                                fc.name.value,
                                params.len(),
                                fc.args.len()
                            ),
                            fc.span,
                        ));
                    }
                    self.check_call_args(&fc.args, params, &fc.name.value, fc.span, env, diags);
                    return Type::string();
                }

                for arg in &fc.args {
                    self.infer_expression(arg, env, diags);
                }
                Type::error("unknown")
            }
            Expr::ContextRef(cr) => {
                // Validate ctx.property exists in declared context
                if !self.context_fields.is_empty()
                    && !self.context_fields.contains_key(&cr.property.value)
                {
                    let ctx_names: Vec<&str> =
                        self.context_fields.keys().map(|s| s.as_str()).collect();
                    let suggestion = find_closest(&cr.property.value, &ctx_names);
                    let help = if let Some(s) = suggestion {
                        format!("Did you mean 'ctx.{}'?", s)
                    } else {
                        format!("Available context properties: {}", ctx_names.join(", "))
                    };
                    diags.push(
                        Diagnostic::error(
                            format!("Unknown context property 'ctx.{}'", cr.property.value),
                            cr.property.span,
                        )
                        .with_help(help),
                    );
                }
                Type::string()
            }
            Expr::HelperCall(hc) => {
                for arg in &hc.args {
                    self.infer_expression(arg, env, diags);
                }
                Type::error("unknown") // would need helper output type
            }
            Expr::PromptCall(pc) => {
                if let Some(params) = self.prompt_map.get(&pc.prompt.value) {
                    if pc.args.len() != params.len() {
                        diags.push(Diagnostic::error(
                            format!(
                                "Prompt '{}' expects {} argument(s) but got {}",
                                pc.prompt.value,
                                params.len(),
                                pc.args.len()
                            ),
                            pc.span,
                        ));
                    }
                    self.check_call_args(&pc.args, params, &pc.prompt.value, pc.span, env, diags);
                } else {
                    diags.push(Diagnostic::error(
                        format!("Unknown prompt '{}'", pc.prompt.value),
                        pc.span,
                    ));
                }
                Type::string()
            }
            Expr::MemberAccess(ma) => {
                let mut current_ty = if let Some(ty) = env.get(&ma.object.value) {
                    ty.clone()
                } else {
                    return Type::error("unknown");
                };

                let mut path = vec![ma.property.value.as_str()];
                for segment in &ma.chain {
                    path.push(segment.value.as_str());
                }

                for segment in path {
                    match current_ty {
                        Type::Record { ref fields, .. } => {
                            if let Some(field_ty) = fields.get(segment) {
                                current_ty = field_ty.clone();
                            } else {
                                return Type::error("unknown");
                            }
                        }
                        _ => return Type::error("unknown"),
                    }
                }

                current_ty
            }
            Expr::IndexAccess(ia) => {
                let arr_ty = if let Some(ty) = env.get(&ia.object.value) {
                    ty.clone()
                } else {
                    Type::error("unknown")
                };
                if let Type::Array(el) = arr_ty {
                    *el
                } else {
                    diags.push(Diagnostic::error(
                        format!(
                            "Cannot index into '{}' — it's not an array",
                            ia.object.value
                        ),
                        ia.span,
                    ));
                    Type::error("not array")
                }
            }
            Expr::BinaryOp(bo) => {
                let left = self.infer_expression(&bo.left, env, diags);
                let right = self.infer_expression(&bo.right, env, diags);
                match bo.op {
                    BinOperator::Add => {
                        if left == Type::string() || right == Type::string() {
                            Type::string()
                        } else {
                            Type::number()
                        }
                    }
                    _ => Type::number(),
                }
            }
            Expr::Grouped(inner, _) => self.infer_expression(inner, env, diags),
            Expr::InlinePrompt(_) => Type::string(),
        }
    }

    // ── Type Mapping ─────────────────────────────────────────────────

    fn map_type_expr(&self, ty: &TypeExpr) -> Type {
        match ty {
            TypeExpr::String(_) => Type::string(),
            TypeExpr::Number(_) => Type::number(),
            TypeExpr::Boolean(_) => Type::boolean(),
            TypeExpr::Array { element, .. } => Type::Array(Box::new(self.map_type_expr(element))),
            TypeExpr::TypeRef(name) => {
                if let Some(fields_decl) = self.type_map.get(&name.value) {
                    let mut fields = HashMap::new();
                    let mut optional = HashMap::new();
                    for f in fields_decl {
                        fields.insert(f.name.value.clone(), self.map_type_expr(&f.ty));
                        optional.insert(f.name.value.clone(), f.optional);
                    }
                    Type::Record { fields, optional }
                } else {
                    Type::error(&name.value)
                }
            }
            TypeExpr::Object { properties, .. } => {
                let mut fields = HashMap::new();
                let mut opt = HashMap::new();
                for p in properties {
                    fields.insert(p.name.value.clone(), self.map_type_expr(&p.ty));
                    opt.insert(p.name.value.clone(), p.optional);
                }
                Type::Record {
                    fields,
                    optional: opt,
                }
            }
            TypeExpr::Union { options, .. } => {
                let types = options
                    .iter()
                    .map(|o| Type::Const(o.value.clone()))
                    .collect();
                Type::Union(types)
            }
        }
    }

    fn types_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if matches!(actual, Type::Error(_)) || matches!(expected, Type::Error(_)) {
            return true; // Don't cascade errors
        }
        match (expected, actual) {
            (Type::Const(a), Type::Const(b)) => a == b,
            (Type::Array(a), Type::Array(b)) => self.types_compatible(a, b),
            (Type::Record { fields: ef, .. }, Type::Record { fields: af, .. }) => {
                // Check all expected non-optional fields exist in actual
                for (name, expected_ty) in ef {
                    if let Some(actual_ty) = af.get(name) {
                        if !self.types_compatible(expected_ty, actual_ty) {
                            return false;
                        }
                    }
                    // Missing field is a soft mismatch — don't fail on it for now
                }
                true
            }
            _ => false,
        }
    }

    fn check_call_args(
        &self,
        args: &[Expr],
        params: &[TypeConfigDecl],
        callee_name: &str,
        span: Span,
        env: &TypeEnv,
        diags: &mut Vec<Diagnostic>,
    ) {
        for (index, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
            let arg_ty = self.infer_expression(arg, env, diags);
            let param_ty = self.map_type_expr(&param.ty);

            if !self.types_compatible(&param_ty, &arg_ty) {
                diags.push(
                    Diagnostic::error(
                        format!(
                            "Argument {} type mismatch for '{}': expected {} but got {} (Type mismatch: {} vs {})",
                            index + 1,
                            callee_name,
                            param_ty.format(),
                            arg_ty.format(),
                            param_ty.format(),
                            arg_ty.format()
                        ),
                        span,
                    )
                    .with_help(format!(
                        "Parameter '{}' expects {}.",
                        param.name.value,
                        param_ty.format()
                    )),
                );
            }
        }

        for arg in args.iter().skip(params.len()) {
            self.infer_expression(arg, env, diags);
        }
    }

    fn validate_comparison_types(
        &self,
        op: &ComparisonOp,
        left: &Type,
        right: &Type,
    ) -> Option<String> {
        if matches!(left, Type::Error(_)) || matches!(right, Type::Error(_)) {
            return None;
        }

        let left_kind = self.comparison_kind(left);
        let right_kind = self.comparison_kind(right);

        match op {
            ComparisonOp::Eq | ComparisonOp::Neq => {
                if left_kind == right_kind {
                    None
                } else {
                    Some(format!(
                        "Condition type mismatch: {} vs {} (Type mismatch: {} vs {})",
                        left_kind, right_kind, left_kind, right_kind
                    ))
                }
            }
            ComparisonOp::Gt | ComparisonOp::Lt | ComparisonOp::Gte | ComparisonOp::Lte => {
                if left_kind == "number" && right_kind == "number" {
                    None
                } else {
                    Some(format!(
                        "Condition type mismatch: {} vs {} (Type mismatch: {} vs {})",
                        left_kind, right_kind, left_kind, right_kind
                    ))
                }
            }
        }
    }

    fn comparison_kind(&self, ty: &Type) -> String {
        match ty {
            Type::Const(name) if name == "string" || name == "number" || name == "boolean" => {
                name.clone()
            }
            Type::Const(_) => "string".into(),
            Type::Union(options) => {
                if options.iter().all(|opt| matches!(opt, Type::Const(_))) {
                    "string".into()
                } else {
                    ty.format()
                }
            }
            _ => ty.format(),
        }
    }
}

// ── Utilities ────────────────────────────────────────────────────────────

/// Find the closest match from a list of candidates using Levenshtein distance.
fn find_closest<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    if candidates.is_empty() {
        return None;
    }

    let mut best_match = None;
    let mut min_dist = usize::MAX;

    for &candidate in candidates {
        let dist = levenshtein(target, candidate);
        // Only suggest if it's reasonably close (e.g. within 3 edits)
        // and proportional to word length.
        let threshold = (target.len() / 3).max(1) + 1;
        if dist < min_dist && dist <= threshold {
            min_dist = dist;
            best_match = Some(candidate);
        }
    }

    best_match
}

/// Calculate Levenshtein edit distance between two strings using dynamic programming.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let len_a = a.len();
    let len_b = b.len();

    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }

    let mut row: Vec<usize> = (0..=len_b).collect();

    for (i, &char_a) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;

        for (j, &char_b) in b.iter().enumerate() {
            let old_val = row[j + 1];
            let cost = if char_a == char_b { 0 } else { 1 };

            row[j + 1] = std::cmp::min(
                std::cmp::min(
                    row[j] + 1,     // Insertion
                    row[j + 1] + 1, // Deletion
                ),
                prev + cost, // Substitution
            );

            prev = old_val;
        }
    }

    row[len_b]
}

fn extract_template_condition_refs(condition: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let operators = ["==", "!=", ">=", "<=", ">", "<"];

    let mut matched = false;
    for operator in operators {
        if let Some((left, right)) = condition.split_once(operator) {
            matched = true;
            collect_template_ref(left, &mut refs);
            collect_template_ref(right, &mut refs);
            break;
        }
    }

    if !matched {
        collect_template_ref(condition, &mut refs);
    }

    refs
}

fn collect_template_ref(token: &str, refs: &mut Vec<String>) {
    let trimmed = token
        .trim()
        .trim_matches(|c: char| matches!(c, '(' | ')' | '{' | '}' | '[' | ']'));

    if trimmed.is_empty()
        || trimmed == "true"
        || trimmed == "false"
        || trimmed.parse::<f64>().is_ok()
        || ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return;
    }

    let root = trimmed.split('.').next().unwrap_or(trimmed).trim();
    if !root.is_empty() {
        refs.push(root.to_string());
    }
}
