use super::Checker;
use crate::state::{Type, TypeEnv};
use crate::utils::find_closest;
use auwgent_ast::*;
use auwgent_errors::{Diagnostic, Span};
use std::collections::HashMap;

impl Checker {
    pub(crate) fn check_workflow(
        &self,
        wf: &WorkflowConfig,
        parent_configs: &[AgentConfig],
        diags: &mut Vec<Diagnostic>,
    ) {
        if wf.description.is_none() {
            diags.push(
                Diagnostic::error(
                    format!("Workflow '{}' is missing a description", wf.name.value),
                    wf.name.span,
                )
                .with_help("A workflow must have a description to explain its purpose. Add `description: \"...\"` inside the workflow block."),
            );
        }

        let mut env = TypeEnv::new();
        let mut bindings: HashMap<String, (&'static str, Span)> = HashMap::new();
        for config in parent_configs {
            match config {
                AgentConfig::Input(ic) => {
                    for p in &ic.properties {
                        if self.declare_scope_name(&mut bindings, &p.name, "input field", diags) {
                            env.set(&p.name.value, self.map_type_expr(&p.ty));
                        }
                    }
                }
                AgentConfig::Context(cc) => {
                    for p in &cc.properties {
                        if self.declare_scope_name(&mut bindings, &p.name, "context field", diags) {
                            env.set(&p.name.value, self.map_type_expr(&p.ty));
                        }
                    }
                }
                _ => {}
            }
        }

        for p in &wf.params {
            if self.declare_scope_name(&mut bindings, &p.name, "workflow parameter", diags) {
                env.set(&p.name.value, self.map_type_expr(&p.ty));
            }
        }

        let expected = self.map_type_expr(&wf.return_type);

        for tf in &wf.tool_configs {
            self.check_tool(tf, diags);
        }

        self.check_statements(&wf.body, &mut env, &mut bindings, &expected, diags);
    }

    pub(crate) fn check_statements(
        &self,
        stmts: &[Statement],
        env: &mut TypeEnv,
        bindings: &mut HashMap<String, (&'static str, Span)>,
        expected_return: &Type,
        diags: &mut Vec<Diagnostic>,
    ) {
        for stmt in stmts {
            match stmt {
                Statement::Let(ls) => {
                    let val_ty = self.infer_expression(&ls.value, env, diags);
                    let is_new_binding =
                        self.declare_scope_name(bindings, &ls.name, "variable", diags);
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
                        if is_new_binding {
                            env.set(&ls.name.value, decl_ty);
                        }
                    } else if is_new_binding {
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
                    let mut then_bindings = bindings.clone();
                    self.check_statements(
                        &ifs.then_block,
                        &mut then_env,
                        &mut then_bindings,
                        expected_return,
                        diags,
                    );
                    if !ifs.else_block.is_empty() {
                        let mut else_env = env.extend();
                        let mut else_bindings = bindings.clone();
                        self.check_statements(
                            &ifs.else_block,
                            &mut else_env,
                            &mut else_bindings,
                            expected_return,
                            diags,
                        );
                    }
                }
                Statement::Transfer(_) => {}
                Statement::Parallel(ps) => {
                    self.check_statements(&ps.body, env, bindings, expected_return, diags);
                }
            }
        }
    }

    pub(crate) fn check_condition(
        &self,
        cond: &Condition,
        env: &TypeEnv,
        diags: &mut Vec<Diagnostic>,
    ) {
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

    pub(crate) fn infer_expression(
        &self,
        expr: &Expr,
        env: &TypeEnv,
        diags: &mut Vec<Diagnostic>,
    ) -> Type {
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
                            .map(|param| {
                                format!(
                                    "{}: {}",
                                    param.name.value,
                                    self.map_type_expr(&param.ty).format()
                                )
                            })
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
                if let Some(tool) = self.tool_map.get(&fc.name.value) {
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
                    if let Some(ret_type) = &tool.returns {
                        return self.map_type_expr(ret_type);
                    } else {
                        return Type::error("unknown");
                    }
                }
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
                if !self.context_fields.is_empty()
                    && !self.context_fields.contains_key(&cr.property.value)
                {
                    let ctx_names: Vec<&str> = self.context_fields.keys().map(|s| s.as_str()).collect();
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
                Type::error("unknown")
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
                                diags.push(Diagnostic::error(
                                    format!("Unknown property '{}' on type", segment),
                                    ma.property.span,
                                ));
                                return Type::error("unknown");
                            }
                        }
                        _ => {
                            diags.push(Diagnostic::error(
                                "Cannot access property on this type",
                                ma.property.span,
                            ));
                            return Type::error("unknown");
                        }
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
                        format!("Cannot index into '{}' — it's not an array", ia.object.value),
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

    pub(crate) fn map_type_expr(&self, ty: &TypeExpr) -> Type {
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
                let types = options.iter().map(|o| Type::Const(o.value.clone())).collect();
                Type::Union(types)
            }
        }
    }

    pub(crate) fn types_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if matches!(actual, Type::Error(_)) || matches!(expected, Type::Error(_)) {
            return true;
        }
        match (expected, actual) {
            (Type::Const(a), Type::Const(b)) => a == b,
            (Type::Array(a), Type::Array(b)) => self.types_compatible(a, b),
            (Type::Record { fields: ef, .. }, Type::Record { fields: af, .. }) => {
                for (name, expected_ty) in ef {
                    if let Some(actual_ty) = af.get(name) {
                        if !self.types_compatible(expected_ty, actual_ty) {
                            return false;
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn check_call_args(
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

    pub(crate) fn validate_comparison_types(
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

    pub(crate) fn comparison_kind(&self, ty: &Type) -> String {
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