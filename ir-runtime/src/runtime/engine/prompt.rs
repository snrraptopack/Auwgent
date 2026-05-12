// Prompt construction layer.
// This file owns prompt assembly for the main agent and helper agents. Keep
// prompt-shaping logic here so engine.rs stays focused on shared state.
use super::*;
use crate::runtime::session::BindingCursor;

impl AuwgentEngine {
    pub fn generate_prompt(&self, helper_name: Option<String>) -> AuwgentResult<String> {
        if let Some(name) = helper_name {
            let sub_ctx = crate::runtime::helper_runner::build_sub_agent_context(&self.ir, &name)?;
            let sub_engine = AuwgentEngine::new(sub_ctx.ir);

            if let Some(ctx) = self.context.lock().unwrap().as_ref() {
                sub_engine.set_context(ctx.clone());
            }

            sub_engine.generate_prompt(None)
        } else {
            self.generate_main_prompt()
        }
    }

    fn generate_main_prompt(&self) -> AuwgentResult<String> {
        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();

        {
            let ctx_val = self
                .context
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            scope.insert("context".to_string(), ctx_val.clone());
            scope.insert("ctx".to_string(), ctx_val);
        }

        let entry = self
            .ir
            .model_config
            .first()
            .ok_or(AuwgentError::MissingConfig("No model config".into()))?;
        let default = entry
            .default_config
            .as_ref()
            .ok_or(AuwgentError::MissingConfig("No default config".into()))?;

        let parsed_prompt: crate::types::Expression =
            serde_json::from_value(default.prompt.0.clone())
                .map_err(|e| AuwgentError::Evaluation(format!("Prompt parse error: {}", e)))?;
        evaluator.clear_context_usage();
        let prompt_val =
            evaluator.evaluate_prompt_with_context_symbols(&parsed_prompt, &mut scope)?;
        let binding_context_keys = evaluator.symbol_context_usage();
        *self.binding_context_keys.lock().unwrap() = binding_context_keys.clone();
        let mut prompt = prompt_val.as_str().unwrap_or("").to_string();

        let protocol = self.resolve_tool_protocol();
        if protocol == "block" {
            let intents = crate::intents::generate_block_protocol_prompt_with_binding_rules(
                &self.ir,
                !binding_context_keys.is_empty(),
            );
            if !intents.is_empty() {
                prompt.push_str("\n\n");
                prompt.push_str(&intents);
            }
        }
        // native mode: append nothing — tool schemas carry all capability descriptions

        Ok(prompt)
    }

    pub(super) fn render_binding_block(&self) -> Option<String> {
        let context = self.context.lock().unwrap().clone()?;
        let obj = context.as_object()?;
        let mut keys = obj.keys().cloned().collect::<Vec<_>>();
        keys.sort();

        let binding_context_keys = self.binding_context_keys.lock().unwrap().clone();
        let mut binding_lines = Vec::new();
        let mut injected_lines = Vec::new();
        for key in keys {
            let Some(value) = obj.get(&key) else {
                continue;
            };
            if is_empty_binding_value(value) {
                continue;
            }
            if binding_context_keys.contains(&key) {
                binding_lines.push(format!(
                    "@@{} is {}",
                    binding_symbol_name(&key),
                    render_binding_value(value)
                ));
            } else {
                injected_lines.push(format!(
                    "{} = {}",
                    binding_symbol_name(&key),
                    render_binding_value(value)
                ));
            }
        }

        let mut sections = Vec::new();
        if !binding_lines.is_empty() {
            sections.push(format!(
                "[binding]\n{}\n[/binding]",
                binding_lines.join("\n")
            ));
        }
        if !injected_lines.is_empty() {
            sections.push(format!(
                "[injected_context]\n{}\n[/injected_context]",
                injected_lines.join("\n")
            ));
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n"))
        }
    }

    pub(super) fn render_binding_cursor(&self) -> Option<BindingCursor> {
        let input = self.render_binding_block()?;
        let turn_index = self.session.lock().unwrap().binding_cursor_turn_index();
        Some(BindingCursor {
            turn_index,
            role: "user".to_string(),
            input: Some(input),
        })
    }
}

fn is_empty_binding_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(values) => values.is_empty(),
        Value::Object(values) => values.is_empty(),
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

fn binding_symbol_name(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn render_binding_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}
