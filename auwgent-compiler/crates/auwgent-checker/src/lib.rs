//! # auwgent-checker
//!
//! Type system, type checker, and validation passes for the Auwgent DSL.
//! Ported from `checker.ts` — validates workflows, prompts, model configs,
//! and type consistency across the agent.

mod declarations;
mod prompts;
mod state;
mod utils;
mod workflow;

use auwgent_ast::*;
use auwgent_errors::{Diagnostic, Span};
use state::TypeEnv;
use std::collections::HashMap;
use utils::find_closest;

// ── Main Check Entry ─────────────────────────────────────────────────────

/// Run all validation and type-checking passes on a parsed model.
/// Returns a list of diagnostics (errors, warnings).
pub fn check(model: &Model) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut checker = Checker::new(model);

    // Collect types and prompts
    checker.collect_declarations(model, &mut diags);

    for element in &model.elements {
        match element {
            Element::Agent(agent) => checker.check_agent(agent, &mut diags),
            Element::Helper(helper) => checker.check_helper(helper, &mut diags),
            Element::ComponentDecl(component) => {
                checker.check_component_decl(component, &mut diags)
            }
            Element::TypeDecl(td) => checker.check_type_decl(td, &mut diags),
            Element::NamedPrompt(p) => checker.check_named_prompt(p, &mut diags),
            Element::ModelDef(_) => {}
            Element::IntentDecl(_) => {}
        }
    }

    diags
}

// ── Checker Struct ───────────────────────────────────────────────────────

struct Checker {
    type_map: HashMap<String, Vec<TypeConfigDecl>>,
    prompt_map: HashMap<String, Vec<TypeConfigDecl>>,
    tool_map: HashMap<String, ToolFunction>,
    helper_map: HashMap<String, Helper>,
    component_map: HashMap<String, ComponentDeclaration>,
    top_level_names: HashMap<String, (&'static str, Span)>,
    /// Context fields for validating ctx.property references
    context_fields: HashMap<String, Span>,
    context_types: HashMap<String, state::Type>,
}

impl Checker {
    fn new(_model: &Model) -> Self {
        Self {
            type_map: HashMap::new(),
            prompt_map: HashMap::new(),
            tool_map: HashMap::new(),
            helper_map: HashMap::new(),
            component_map: HashMap::new(),
            top_level_names: HashMap::new(),
            context_fields: HashMap::new(),
            context_types: HashMap::new(),
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
        self.context_types.clear();
        for config in &agent.configs {
            if let AgentConfig::Context(cc) = config {
                for p in &cc.properties {
                    self.context_fields
                        .insert(p.name.value.clone(), p.name.span);
                    self.context_types
                        .insert(p.name.value.clone(), self.map_type_expr(&p.ty));
                }
            }
        }

        // Pass 2: Validate everything (tools are now in tool_map)
        for config in &agent.configs {
            match config {
                AgentConfig::Model(mc) => {
                    let mut scope_params = Vec::new();
                    for config in &agent.configs {
                        if let AgentConfig::Input(ic) = config {
                            match &ic.shape {
                                InputShape::Properties(props) => {
                                    scope_params.extend(props.iter().map(|p| p.name.value.clone()));
                                }
                                InputShape::Direct(_) => {
                                    scope_params.push("input".to_string());
                                }
                            }
                        }
                        if let AgentConfig::Context(cc) = config {
                            scope_params.extend(cc.properties.iter().map(|p| p.name.value.clone()));
                        }
                    }

                    // Validate default config
                    if let Some(expr) = &mc.default_config.prompt_expr {
                        self.infer_expression(expr, &TypeEnv::new(), diags);
                    }
                    if !mc.default_config.prompt_block.is_empty() {
                        self.check_prompt_statements(
                            &mc.default_config.prompt_block,
                            &scope_params,
                            diags,
                        );
                    }

                    // Validate named configs
                    for nc in &mc.named_configs {
                        if let Some(expr) = &nc.config.prompt_expr {
                            self.infer_expression(expr, &TypeEnv::new(), diags);
                        }
                        if !nc.config.prompt_block.is_empty() {
                            self.check_prompt_statements(
                                &nc.config.prompt_block,
                                &scope_params,
                                diags,
                            );
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
                AgentConfig::Input(ic) => self.check_input(ic, true, diags),
                AgentConfig::Output(oc) => self.check_output(oc, diags),
                AgentConfig::Context(cc) => self.check_properties(&cc.properties, diags),
                AgentConfig::Helpers(hc) => self.check_helpers_config(hc, diags),
                AgentConfig::Intent(_) => {}
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
        self.context_types.clear();
        for config in &helper.configs {
            if let AgentConfig::Context(cc) = config {
                for p in &cc.properties {
                    self.context_fields
                        .insert(p.name.value.clone(), p.name.span);
                    self.context_types
                        .insert(p.name.value.clone(), self.map_type_expr(&p.ty));
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
                AgentConfig::Input(ic) => self.check_input(ic, false, diags),
                AgentConfig::Output(oc) => self.check_output(oc, diags),
                AgentConfig::Context(cc) => self.check_properties(&cc.properties, diags),
                AgentConfig::Tool(tf) => self.check_tool(tf, diags),
                AgentConfig::Tools(tfs) => {
                    for tf in tfs {
                        self.check_tool(tf, diags);
                    }
                }
                AgentConfig::Workflow(wf) => self.check_workflow(wf, &helper.configs, diags),
                AgentConfig::Intent(_) => {}
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

        if let Some(returns) = &tf.returns {
            self.check_type_ref_exists(returns, diags);
        } else {
            diags.push(
                Diagnostic::error(
                    format!("Tool '{}' does not specify a return type", tf.name.value),
                    tf.name.span,
                )
                .with_help(
                    "A tool must specify a return type, e.g. `tool one(id: string): string`",
                ),
            );
        }
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

    fn check_component_decl(&self, component: &ComponentDeclaration, diags: &mut Vec<Diagnostic>) {
        if component.fields.is_empty() {
            diags.push(
                Diagnostic::warning(
                    format!("Component '{}' has no fields", component.name.value),
                    component.name.span,
                )
                .with_help("Add props, action bindings, or a children constraint to make this component useful."),
            );
        }

        let mut seen: HashMap<String, Span> = HashMap::new();
        let mut action_seen: HashMap<String, Span> = HashMap::new();
        let mut has_children = false;

        for field in &component.fields {
            match field {
                ComponentField::Prop(prop) => {
                    if let Some(prev_span) = seen.get(&prop.name.value) {
                        diags.push(
                            Diagnostic::error(
                                format!(
                                    "Duplicate field '{}' in component '{}'",
                                    prop.name.value, component.name.value
                                ),
                                prop.name.span,
                            )
                            .with_label(*prev_span, "first defined here"),
                        );
                    } else {
                        seen.insert(prop.name.value.clone(), prop.name.span);
                    }
                    self.check_type_ref_exists(&prop.ty, diags);
                }
                ComponentField::Action(action) => {
                    if let Some(prev_span) = seen.get("action") {
                        diags.push(
                            Diagnostic::error(
                                format!(
                                    "Duplicate field 'action' in component '{}'",
                                    component.name.value
                                ),
                                action.span,
                            )
                            .with_label(*prev_span, "first defined here"),
                        );
                    } else {
                        seen.insert("action".to_string(), action.span);
                    }

                    for binding in &action.bindings {
                        if let Some(prev_span) = action_seen.get(&binding.name.value) {
                            diags.push(
                                Diagnostic::error(
                                    format!(
                                        "Duplicate action binding '{}' in component '{}'",
                                        binding.name.value, component.name.value
                                    ),
                                    binding.name.span,
                                )
                                .with_label(*prev_span, "first defined here"),
                            );
                        } else {
                            action_seen.insert(binding.name.value.clone(), binding.name.span);
                        }

                        let mut target_seen: HashMap<String, Span> = HashMap::new();
                        for target in &binding.targets {
                            if let Some(prev_span) = target_seen.get(&target.name.value) {
                                diags.push(
                                    Diagnostic::error(
                                        format!(
                                            "Duplicate action target '{}' in binding '{}' for component '{}'",
                                            target.name.value, binding.name.value, component.name.value
                                        ),
                                        target.name.span,
                                    )
                                    .with_label(*prev_span, "first defined here"),
                                );
                            } else {
                                target_seen.insert(target.name.value.clone(), target.name.span);
                            }

                            for param in &target.params {
                                self.check_type_ref_exists(&param.ty, diags);
                            }
                        }
                    }
                }
                ComponentField::Children(children) => {
                    if has_children {
                        diags.push(
                            Diagnostic::error(
                                format!(
                                    "Duplicate field 'children' in component '{}'",
                                    component.name.value
                                ),
                                children.span,
                            )
                            .with_help("A component can declare at most one children constraint."),
                        );
                    }
                    has_children = true;

                    if let ComponentChildrenConstraint::Only(allowed) = &children.allowed {
                        for child in allowed {
                            if !self.component_map.contains_key(&child.value) {
                                diags.push(
                                    Diagnostic::error(
                                        format!(
                                            "Unknown component '{}' in children constraint for '{}'",
                                            child.value, component.name.value
                                        ),
                                        child.span,
                                    )
                                    .with_help(format!(
                                        "Declare component '{}' or use one of: {}",
                                        child.value,
                                        self.component_map.keys().cloned().collect::<Vec<_>>().join(", ")
                                    )),
                                );
                            }
                        }
                    }
                }
            }
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

    fn check_input(&self, ic: &InputConfig, is_root: bool, diags: &mut Vec<Diagnostic>) {
        match &ic.shape {
            InputShape::Direct(ty) => {
                if is_root {
                    // Enforce that main agent input is Text (String)
                    // We allow 'Text' as a TypeRef or explicit 'string' keyword
                    let is_valid = match ty {
                        TypeExpr::String(_) => true,
                        TypeExpr::Text(_) => true,
                        TypeExpr::TypeRef(name) if name.value == "Text" => true,
                        _ => false,
                    };

                    if !is_valid {
                        diags.push(
                            Diagnostic::error("Main agent input must be 'Text' for now", ic.span)
                                .with_help("Change to: input: Text"),
                        );
                    }
                }
                self.check_type_ref_exists(ty, diags);
            }
            InputShape::Properties(props) => {
                if is_root {
                    diags.push(
                        Diagnostic::error(
                            "Main agent does not support structured input blocks. Use a helper instead.",
                            ic.span,
                        )
                        .with_help("Main agent input must be 'Text'."),
                    );
                }
                self.check_properties(props, diags);
            }
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
}

#[cfg(test)]
mod tests {
    use super::check;
    use auwgent_lexer::tokenize;
    use auwgent_parser::parse;

    fn check_source(source: &str) -> Vec<String> {
        let (tokens, lex_errors) = tokenize(source);
        assert!(lex_errors.is_empty(), "lexer errors: {lex_errors:?}");

        let (model, parse_errors) = parse(&tokens);
        assert!(parse_errors.is_empty(), "parse errors: {parse_errors:?}");

        check(&model).into_iter().map(|diag| diag.message).collect()
    }

    #[test]
    fn rejects_string_boolean_comparisons_in_workflows() {
        let diagnostics = check_source(
            r#"
            agent Manager {
                default config {
                    model: gemini("gemini-2.5-flash")
                    prompt: "hello"
                }

                context {
                    user_type: string
                }

                workflow deleteAccount(id: string): { delete: boolean, reason?: string } {
                    description: "delete with authorization checks"

                    if (ctx.user_type == true) {
                        return { delete: true }
                    }

                    return { delete: false, reason: "not allowed" }
                }
            }
            "#,
        );

        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("Cannot compare string with boolean")),
            "expected string/boolean comparison error, got {diagnostics:?}"
        );
    }

    #[test]
    fn rejects_non_boolean_if_conditions() {
        let diagnostics = check_source(
            r#"
            agent Manager {
                default config {
                    model: gemini("gemini-2.5-flash")
                    prompt: "hello"
                }

                workflow deleteAccount(id: string): { delete: boolean } {
                    description: "delete with authorization checks"

                    if ("admin") {
                        return { delete: true }
                    }

                    return { delete: false }
                }
            }
            "#,
        );

        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("If-condition must be boolean")),
            "expected boolean condition error, got {diagnostics:?}"
        );
    }

    #[test]
    fn rejects_unknown_component_in_children_constraint() {
        let diagnostics = check_source(
            r#"
            component Button {
                label: string
            }

            component Screen {
                children: Button | MissingCard
            }

            agent Main {
                default config {
                    model: gemini("gemini-2.5-flash")
                    prompt: "hello"
                }

                input: Text
            }
            "#,
        );

        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("Unknown component 'MissingCard'")),
            "expected unknown component error, got {diagnostics:?}"
        );
    }
}
