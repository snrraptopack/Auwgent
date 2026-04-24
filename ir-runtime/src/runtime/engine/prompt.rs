// Prompt construction layer.
// This file owns prompt assembly for the main agent and helper agents. Keep
// prompt-shaping logic here so engine.rs stays focused on shared state.
use super::*;

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
        let referenced_context_keys = evaluator.collect_context_references(&parsed_prompt, &scope);
        let prompt_val = evaluator.evaluate(&parsed_prompt, &mut scope)?;
        let mut prompt = prompt_val.as_str().unwrap_or("").to_string();

        if let Some(ctx) = self.context.lock().unwrap().as_ref()
            && let Some(obj) = ctx.as_object()
        {
            let mut filtered_ctx = serde_json::Map::new();
            for (key, value) in obj {
                if referenced_context_keys.contains(key) {
                    continue;
                }

                let is_empty = match value {
                    Value::Null => true,
                    Value::Array(values) => values.is_empty(),
                    Value::Object(values) => values.is_empty(),
                    Value::String(text) => text.is_empty(),
                    _ => false,
                };

                if !is_empty {
                    filtered_ctx.insert(key.clone(), value.clone());
                }
            }

            if !filtered_ctx.is_empty()
                && let Ok(yaml) = serde_yaml::to_string(&Value::Object(filtered_ctx))
            {
                prompt.push_str("\n\n# ADDITIONAL CONTEXT\n");
                prompt.push_str(yaml.trim());
            }
        }

        let intents = crate::intents::generate_block_protocol_prompt(&self.ir);
        if !intents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&intents);
        }

        Ok(prompt)
    }
}

