use super::Checker;
use crate::utils::{extract_template_condition_refs, find_closest};
use auwgent_ast::{Expr, NamedPrompt, PromptStatement};
use auwgent_errors::{Diagnostic, Span};
use std::collections::HashMap;

impl Checker {
    pub(crate) fn check_named_prompt(&self, prompt: &NamedPrompt, diags: &mut Vec<Diagnostic>) {
        let mut param_bindings: HashMap<String, (&'static str, Span)> = HashMap::new();
        for param in &prompt.params {
            self.declare_scope_name(
                &mut param_bindings,
                &param.name,
                "prompt parameter",
                diags,
            );
            self.check_type_ref_exists(&param.ty, diags);
        }

        let param_names: Vec<&str> = prompt
            .params
            .iter()
            .map(|p| p.name.value.as_str())
            .collect();

        for stmt in &prompt.body {
            if let PromptStatement::Expr(expr) = stmt {
                self.check_prompt_template_vars(expr, &param_names, diags);
            }
        }
    }

    pub(crate) fn check_prompt_template_vars(
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
            let mut i = 0;
            let bytes = text.as_bytes();
            while i + 3 < bytes.len() {
                if bytes[i] == b'{' && bytes[i + 1] == b'{' {
                    if let Some(end) = text[i + 2..].find("}}") {
                        let var_name = text[i + 2..i + 2 + end].trim();

                        if var_name.starts_with("@schema(") && var_name.ends_with(')') {
                            let schema_args = &var_name[8..var_name.len() - 1];
                            if schema_args != "input" && schema_args != "output" {
                                diags.push(
                                    Diagnostic::error(
                                        format!("Invalid schema argument '{}'", schema_args),
                                        span,
                                    )
                                    .with_help("The @schema directive only supports 'input' or 'output' as arguments."),
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
                                format!("Did you mean '{}' ?", s).replace(" ?", "?")
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

    pub(crate) fn check_template_condition_vars(
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
                    format!("Did you mean '{}' ?", s).replace(" ?", "?")
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
}