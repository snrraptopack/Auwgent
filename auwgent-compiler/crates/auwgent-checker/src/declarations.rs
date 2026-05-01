use super::Checker;
use auwgent_ast::{Element, Model, Spanned};
use auwgent_errors::{Diagnostic, Span};
use std::collections::HashMap;

impl Checker {
    pub(crate) fn collect_declarations(&mut self, model: &Model, diags: &mut Vec<Diagnostic>) {
        for el in &model.elements {
            match el {
                Element::Agent(agent) => {
                    self.register_top_level_name(&agent.name, "agent", diags);
                }
                Element::Helper(helper) => {
                    self.register_top_level_name(&helper.name, "helper", diags);
                    self.helper_map
                        .insert(helper.name.value.clone(), helper.clone());
                }
                Element::ComponentDecl(component) => {
                    self.register_top_level_name(&component.name, "component", diags);
                    self.component_map
                        .insert(component.name.value.clone(), component.clone());
                }
                Element::TypeDecl(td) => {
                    self.register_top_level_name(&td.name, "type", diags);
                    self.type_map
                        .entry(td.name.value.clone())
                        .or_insert_with(|| td.fields.clone());
                }
                Element::NamedPrompt(prompt) => {
                    self.register_top_level_name(&prompt.name, "prompt", diags);
                    self.prompt_map
                        .entry(prompt.name.value.clone())
                        .or_insert_with(|| prompt.params.clone());
                }
                Element::ModelDef(model) => {
                    self.register_top_level_name(&model.name, "model", diags);
                }
                Element::IntentDecl(intent) => {
                    self.register_top_level_name(&intent.name, "intent", diags);
                }
            }
        }
    }

    pub(crate) fn register_top_level_name(
        &mut self,
        name: &Spanned<String>,
        kind: &'static str,
        diags: &mut Vec<Diagnostic>,
    ) {
        if let Some((prev_kind, _)) = self.top_level_names.get(&name.value) {
            let message = if *prev_kind == kind {
                format!("Duplicate {} name '{}'", kind, name.value)
            } else {
                format!(
                    "Name collision: {} '{}' conflicts with existing {} '{}'",
                    kind, name.value, prev_kind, name.value
                )
            };
            let help = if *prev_kind == kind {
                format!(
                    "Rename one of the '{}' {} declarations so the top-level name is unique.",
                    name.value, kind
                )
            } else {
                "Agents, helpers, components, prompts, types, intents, and models share one top-level namespace. Rename one of them.".to_string()
            };

            diags.push(Diagnostic::error(message, name.span).with_help(help));
            return;
        }

        self.top_level_names
            .insert(name.value.clone(), (kind, name.span));
    }

    pub(crate) fn declare_scope_name(
        &self,
        bindings: &mut HashMap<String, (&'static str, Span)>,
        name: &Spanned<String>,
        kind: &'static str,
        diags: &mut Vec<Diagnostic>,
    ) -> bool {
        if let Some((prev_kind, _)) = bindings.get(&name.value) {
            let message = if *prev_kind == kind {
                format!("Duplicate {} '{}'", kind, name.value)
            } else {
                format!(
                    "Name collision: {} '{}' conflicts with existing {} '{}'",
                    kind, name.value, prev_kind, name.value
                )
            };
            let help = if *prev_kind == kind {
                format!(
                    "Rename one of the '{}' declarations in this scope.",
                    name.value
                )
            } else {
                format!(
                    "Rename '{}' so it does not shadow the existing {} in this scope.",
                    name.value, prev_kind
                )
            };

            diags.push(Diagnostic::error(message, name.span).with_help(help));
            return false;
        }

        bindings.insert(name.value.clone(), (kind, name.span));
        true
    }
}
