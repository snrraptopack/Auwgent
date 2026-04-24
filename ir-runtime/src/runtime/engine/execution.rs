// Intent dispatch layer.
// This file decides how parsed intents are routed and recorded. Concrete tool,
// workflow, and helper execution lives in the execution submodules.
use super::*;

mod helpers;
mod tools;
mod workflows;

impl AuwgentEngine {
    pub(super) fn build_results_payload(&self) -> String {
        let results = self.pending_tool_results.lock().unwrap();
        if results.is_empty() {
            return String::new();
        }

        let mut blocks = Vec::new();
        for (name, args, result) in &*results {
            let payload = serde_json::json!({
                "name": name,
                "args": args,
                "result": result
            });

            let body = serde_yaml::to_string(&payload)
                .unwrap_or_default()
                .trim()
                .trim_start_matches("---")
                .trim()
                .to_string();

            blocks.push(format!("[result]\n{}\n[/result]", body));
        }

        blocks.join("\n\n")
    }

    pub(super) async fn fire_intent(&self, name: String, value: Value) -> Option<IntentControl> {
        let handler = self.intent_handler.lock().unwrap().clone();
        if let Some(h) = handler {
            h(name, value, self.ir.name.clone()).await
        } else {
            None
        }
    }

    pub async fn process_intents(&self) -> AuwgentResult<(bool, bool, bool)> {
        let intents = {
            let mut pending = self
                .pending_intents
                .lock()
                .expect("pending_intents mutex poisoned");
            std::mem::take(&mut *pending)
        };

        let contains_actions = intents.iter().any(|(name, _)| {
            matches!(name.as_str(), "tool_call" | "workflow_call" | "helper_call")
        });

        let mut has_terminal = false;
        let mut has_actions = false;
        let mut hard_stop = false;
        let mut tool_results: Vec<(String, Value, Value)> = Vec::new();

        for (name, mut value) in intents {
            if contains_actions && matches!(name.as_str(), "response_text" | "response_schema") {
                continue;
            }

            let control = if let Some(control) = self
                .apply_intent_middleware(&name, &value, &self.ir.name)
                .await
            {
                Some(control)
            } else {
                self.fire_intent(name.clone(), self.strip_raw_field(value.clone()))
                    .await
            };

            if let Value::Object(ref mut map) = value {
                map.remove("_raw");
            }

            self.emit_structured_intent(name.clone(), value.clone());

            match name.as_str() {
                "tool_call" => match control {
                    Some(IntentControl::Skip) => {
                        self.fire_intent("tool_skipped".to_string(), value.clone())
                            .await;
                        continue;
                    }
                    Some(IntentControl::Override { result }) => {
                        let tool_name = value["type"].as_str().unwrap_or("").to_string();
                        let args = value["args"].clone();
                        self.fire_intent(
                            "tool_result".to_string(),
                            serde_json::json!({
                                "name": tool_name,
                                "args": args,
                                "result": result,
                                "overridden": true,
                            }),
                        )
                        .await;
                        tool_results.push((tool_name, args, result));
                        has_actions = true;
                    }
                    None => {
                        let (tool_name, args, result) = self.execute_tool(&value).await?;
                        self.fire_intent(
                            "tool_result".to_string(),
                            serde_json::json!({
                                "name": tool_name,
                                "args": args,
                                "result": result,
                            }),
                        )
                        .await;
                        tool_results.push((tool_name, args, result));
                        has_actions = true;
                    }
                },
                "workflow_call" => match control {
                    Some(IntentControl::Skip) => continue,
                    Some(IntentControl::Override { result }) => {
                        let wf_name = value["type"].as_str().unwrap_or("").to_string();
                        let args = value["args"].clone();
                        tool_results.push((format!("workflow:{}", wf_name), args, result));
                        has_actions = true;
                    }
                    None => {
                        let (wf_name, args, result) = self.execute_workflow(&value).await?;
                        self.fire_intent(
                            "workflow_result".to_string(),
                            serde_json::json!({
                                "name": wf_name,
                                "args": args,
                                "result": result,
                            }),
                        )
                        .await;
                        tool_results.push((format!("workflow:{}", wf_name), args, result));
                        has_actions = true;
                    }
                },
                "helper_call" => match control {
                    Some(IntentControl::Skip) => continue,
                    Some(IntentControl::Override { result }) => {
                        let helper_name = value["type"].as_str().unwrap_or("").to_string();
                        let args = value["args"].clone();
                        tool_results.push((format!("helper:{}", helper_name), args, result));
                        has_actions = true;
                    }
                    None => {
                        let (helper_name, args, result) = self.execute_helper(&value).await?;

                        if let Some(obj) = result.as_object()
                            && obj
                                .get("__handoff_stop")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                        {
                            has_terminal = true;
                            hard_stop = true;
                        }

                        self.fire_intent(
                            "helper_result".to_string(),
                            serde_json::json!({
                                "name": helper_name,
                                "args": args,
                                "result": result,
                            }),
                        )
                        .await;
                        tool_results.push((format!("helper:{}", helper_name), args, result));
                        has_actions = true;
                    }
                },
                "response_schema" | "response_text" => {
                    has_terminal = true;
                    *self.last_turn_response_value.lock().unwrap() = value.clone();
                    *self.terminal_response_emitted.lock().unwrap() = true;
                    *self.final_response_emitted.lock().unwrap() = true;
                }
                _ => {
                    has_terminal = true;
                    *self.last_turn_response_value.lock().unwrap() = value.clone();
                    *self.terminal_response_emitted.lock().unwrap() = true;
                }
            }
        }

        self.pending_tool_results.lock().unwrap().extend(tool_results);
        Ok((has_terminal, has_actions, hard_stop))
    }
}

