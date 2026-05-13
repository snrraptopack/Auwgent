// Runtime loop and middleware coordination.
// This file owns the agent run loop, stream handling, lifecycle middleware,
// and event-context helpers. Execution of tools/workflows/helpers belongs in
// the execution modules instead of here.
use super::*;
use crate::runtime::session::{display_input_value, input_parts_value};

/// Maximum number of consecutive `forceStart` retries allowed before giving up.
const MAX_FORCE_START_RETRIES: u32 = 5;

/// Deep-merge two JSON objects. `b` wins on conflicts.
/// Non-object values are replaced outright.
pub fn deep_merge_json(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut a_obj), Value::Object(b_obj)) => {
            for (k, v) in b_obj {
                let entry = a_obj.entry(k).or_insert_with(|| Value::Null);
                *entry = deep_merge_json(entry.clone(), v);
            }
            Value::Object(a_obj)
        }
        (_, b) => b,
    }
}

impl AuwgentEngine {
    pub(super) fn build_event_context(
        &self,
        active_agent: &str,
        raw_block: Option<String>,
        system_prompt: Option<String>,
    ) -> EventContext {
        let session = self.session.lock().unwrap();
        EventContext {
            active_agent: active_agent.to_string(),
            raw_block,
            stack: session.stack.clone(),
            root_agent: session
                .stack
                .first()
                .cloned()
                .unwrap_or_else(|| self.ir.name.clone()),
            system_prompt,
            model: None,
            provider: None,
            config: None,
            url: None,
            headers: None,
            api_key: None,
        }
    }

    pub(super) async fn fire_middleware_event(&self, event: Value) -> Option<Value> {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        middleware::fire_middleware_event(handler, event).await
    }

    pub(super) async fn apply_intent_middleware(
        &self,
        name: &str,
        value: &Value,
        active_agent: &str,
    ) -> Option<IntentControl> {
        let raw_block = value
            .get("_raw")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let payload = IntentPayload {
            name: name.to_string(),
            value: value.clone(),
            context: self.build_event_context(active_agent, raw_block, None),
        };
        middleware::apply_intent_middleware(handler, payload).await
    }

    pub(super) fn strip_raw_field(&self, mut value: Value) -> Value {
        if let Value::Object(ref mut map) = value {
            map.remove("_raw");
        }
        value
    }

    async fn process_synthetic_intent(
        &self,
        name: impl Into<String>,
        value: Value,
    ) -> AuwgentResult<(bool, bool, bool)> {
        self.pending_intents
            .lock()
            .unwrap()
            .push((name.into(), value));
        self.process_intents().await
    }

    fn native_response_schema_payload(&self, value: Value) -> Value {
        if value.get("type").and_then(Value::as_str).is_some() && value.get("response").is_some() {
            return value;
        }

        let mut schema_name = "Output".to_string();
        let mut response = value;
        if let Value::Object(ref mut map) = response
            && let Some(Value::String(variant)) = map.remove("__variant")
        {
            schema_name = variant;
        }

        serde_json::json!({
            "type": schema_name,
            "response": response,
        })
    }

    pub async fn begin_manual_run(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<Value> {
        {
            let mut session = self.session.lock().unwrap();
            if session.stack.is_empty() {
                session.stack = vec![self.ir.name.clone()];
            }
            if let Some(stack) = initial_stack {
                session.stack = stack;
            }
        }
        self.sync_fast_forward_from_session();

        let system_prompt = self.generate_prompt(None)?;
        self.session
            .lock()
            .unwrap()
            .set_system_prompt(&system_prompt);

        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let payload = RunStartPayload {
            session: serde_json::from_str::<Value>(&self.export_session()?)
                .map_err(AuwgentError::Serialization)?,
            context: self.build_event_context(&self.ir.name, None, Some(system_prompt.clone())),
        };

        if let Some(response) = middleware::apply_run_start_middleware(handler, payload).await
            && let Some(updated_session) = response.get("session")
        {
            self.import_session(
                &serde_json::to_string(updated_session).map_err(AuwgentError::Serialization)?,
            )?;
            self.sync_fast_forward_from_session();
        }

        if self.middleware_event_handler.lock().unwrap().is_some() {
            let system_prompt = self.generate_prompt(None)?;
            self.session
                .lock()
                .unwrap()
                .set_system_prompt(&system_prompt);
        }

        if let Some(user_input) = input {
            let user_text = display_input_value(&user_input);
            let input_parts = input_parts_value(&user_input);
            *self.user_input.lock().unwrap() = Some(user_input);
            if let Some(parts) = input_parts {
                self.session
                    .lock()
                    .unwrap()
                    .start_turn_parts(user_text, parts);
            } else {
                self.session.lock().unwrap().start_turn(user_text);
            }
        }

        self.pending_tool_results.lock().unwrap().clear();
        *self.current_raw_response.lock().unwrap() = String::new();
        *self.last_turn_response_value.lock().unwrap() = Value::Null;
        *self.terminal_response_emitted.lock().unwrap() = false;
        *self.final_response_emitted.lock().unwrap() = false;
        *self.last_run_metadata.lock().unwrap() = RunMetadata::default();
        self.emit_structured_stream_start();

        serde_json::from_str::<Value>(&self.export_session()?).map_err(AuwgentError::Serialization)
    }

    pub async fn apply_manual_llm_start(&self, prompt: String) -> AuwgentResult<String> {
        let system_prompt = self.session.lock().unwrap().system_prompt.clone();
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let payload = LlmStartPayload {
            prompt: prompt.clone(),
            context: self.build_event_context(&self.ir.name, None, system_prompt),
        };

        let mut next_prompt = prompt;
        if let Some(middleware_result) =
            middleware::apply_llm_start_middleware(handler, payload).await
        {
            let parsed = middleware::parse_llm_start_response(&middleware_result);

            if let Some(modified) = parsed.prompt {
                next_prompt = modified;
                self.session
                    .lock()
                    .unwrap()
                    .set_display_input(next_prompt.clone());
            }

            if let Some(stack_vec) = parsed.stack {
                if !stack_vec.is_empty() {
                    self.session.lock().unwrap().stack = stack_vec;
                    self.sync_fast_forward_from_session();
                }
            }
        }

        if self.middleware_event_handler.lock().unwrap().is_some() {
            let system_prompt = self.generate_prompt(None)?;
            self.session
                .lock()
                .unwrap()
                .set_system_prompt(&system_prompt);
        }

        Ok(next_prompt)
    }

    pub async fn apply_manual_llm_end(&self, response: Value) -> AuwgentResult<()> {
        if let Some(text) = response.as_str() {
            self.session.lock().unwrap().set_model_response(text);
        } else {
            self.session.lock().unwrap().set_model_response(
                serde_json::to_string(&response).map_err(AuwgentError::Serialization)?,
            );
        }

        let turn_metadata = TurnMetadata {
            turn_index: self.session.lock().unwrap().turns.len().saturating_sub(1),
            usage: TokenUsage::default(),
            finish_reason: None,
            model: "manual".to_string(),
        };
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        middleware::notify_llm_end_middleware(
            handler,
            &response,
            self.build_event_context(&self.ir.name, None, None),
            &turn_metadata,
        )
        .await;
        Ok(())
    }

    pub async fn complete_manual_run(&self) -> AuwgentResult<Value> {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let payload = RunCompletePayload {
            session: serde_json::from_str::<Value>(&self.export_session()?)
                .map_err(AuwgentError::Serialization)?,
            context: self.build_event_context(&self.ir.name, None, None),
        };
        let _ = middleware::apply_run_complete_middleware(handler, payload).await;
        self.emit_structured_stream_finish();
        serde_json::from_str::<Value>(&self.export_session()?).map_err(AuwgentError::Serialization)
    }

    pub async fn apply_manual_error(
        &self,
        error: Value,
        include_session: bool,
    ) -> AuwgentResult<bool> {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let session = if include_session {
            Some(
                serde_json::from_str::<Value>(&self.export_session()?)
                    .map_err(AuwgentError::Serialization)?,
            )
        } else {
            None
        };
        let payload = ErrorPayload {
            session,
            context: self.build_event_context(&self.ir.name, None, None),
            error,
        };
        let response = middleware::apply_error_middleware(handler, payload).await;
        Ok(response
            .as_ref()
            .and_then(|value| value.get("swallow"))
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn run(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<()> {
        let timing = RuntimeTimingProbe::new("rust.engine.run");
        {
            let mut session = self.session.lock().unwrap();
            if session.stack.is_empty() {
                session.stack = vec![self.ir.name.clone()];
            }
            if let Some(stack) = initial_stack {
                session.stack = stack;
            }

            if session.stack.len() > 1 {
                *self.fast_forward_stack.lock().unwrap() = Some(session.stack[1..].to_vec());
            } else {
                *self.fast_forward_stack.lock().unwrap() = None;
            }
        }
        timing.mark("initialized session stack");

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
        timing.mark("built initial scope");

        let evaluator = Evaluator::new(&self.ir);
        let model_entry = self
            .ir
            .model_config
            .first()
            .ok_or(AuwgentError::MissingConfig("No model config".into()))?;
        let default_config = model_entry
            .default_config
            .as_ref()
            .ok_or(AuwgentError::MissingConfig("No default config".into()))?;

        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let model_info = evaluator.evaluate_model(default_config, &mut scope)?;
        let provider_type = model_info["type"]
            .as_str()
            .or_else(|| model_info["provider"].as_str())
            .unwrap_or("gemini");
        let mut provider_id = if provider_type == "custom" {
            model_info["id"].as_str().unwrap_or("custom").to_string()
        } else {
            provider_type.to_string()
        };
        let mut model_name = model_info["modelName"]
            .as_str()
            .unwrap_or("gemini-2.0-flash")
            .to_string();
        let mut config_params = model_info.get("config").cloned();
        timing.mark("evaluated model config");

        let mut system_prompt = self.generate_prompt(None)?;
        self.session
            .lock()
            .unwrap()
            .set_system_prompt(&system_prompt);
        timing.mark("generated system prompt");

        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let payload = RunStartPayload {
            session: serde_json::from_str::<Value>(&self.export_session()?)
                .map_err(AuwgentError::Serialization)?,
            context: self.build_event_context(&self.ir.name, None, Some(system_prompt.clone())),
        };

        if let Some(response) = middleware::apply_run_start_middleware(handler, payload).await
            && let Some(updated_session) = response.get("session")
        {
            self.import_session(
                &serde_json::to_string(updated_session).map_err(AuwgentError::Serialization)?,
            )?;
            self.sync_fast_forward_from_session();
        }
        timing.mark("run_start middleware complete");

        if self.middleware_event_handler.lock().unwrap().is_some() {
            system_prompt = self.generate_prompt(None)?;
            self.session
                .lock()
                .unwrap()
                .set_system_prompt(&system_prompt);
            timing.mark("regenerated prompt after run_start middleware");
        }

        let initial_user_input = input.as_ref().map(display_input_value);
        let initial_input_parts = input.as_ref().and_then(input_parts_value);

        *self.terminal_response_emitted.lock().unwrap() = false;
        *self.final_response_emitted.lock().unwrap() = false;
        self.emit_structured_stream_start();
        timing.mark("started structured stream");

        let is_teleporting = self.fast_forward_stack.lock().unwrap().is_some();
        if let Some(user_text) = initial_user_input.clone() {
            *self.user_input.lock().unwrap() = input.clone();
            if !is_teleporting {
                if let Some(parts) = initial_input_parts.clone() {
                    self.session
                        .lock()
                        .unwrap()
                        .start_turn_parts(&user_text, parts);
                } else {
                    self.session.lock().unwrap().start_turn(&user_text);
                }
            }
        }

        self.pending_tool_results.lock().unwrap().clear();
        *self.current_raw_response.lock().unwrap() = String::new();
        *self.last_turn_response_value.lock().unwrap() = Value::Null;
        *self.last_run_metadata.lock().unwrap() = RunMetadata::default();

        let run_result: AuwgentResult<()> = async {
            let mut loop_count = 0usize;
            let mut empty_completion_retries = 0usize;

            loop {
                loop_count += 1;
                self.pending_tool_results.lock().unwrap().clear();
                *self.current_raw_response.lock().unwrap() = String::new();
                *self.last_turn_response_value.lock().unwrap() = Value::Null;
                *self.terminal_response_emitted.lock().unwrap() = false;
                *self.final_response_emitted.lock().unwrap() = false;
                self.orchestrator.lock().unwrap().reset();

                let teleport_target = {
                    let ffs_lock = self.fast_forward_stack.lock().unwrap();
                    ffs_lock.as_ref().and_then(|ffs| ffs.first().cloned())
                };

                if let Some(target_helper_name) = teleport_target {
                    let synthetic_intent = serde_json::json!({
                        "type": target_helper_name,
                        "args": {
                            "user_text": input.as_ref().and_then(|v| v.as_str()).unwrap_or("")
                        }
                    });

                    self.pending_intents
                        .lock()
                        .unwrap()
                        .push(("helper_call".to_string(), synthetic_intent));

                    let (_terminal, actions, hard_stop) = self.process_intents().await?;

                    if hard_stop {
                        let raw_resp = self.current_raw_response.lock().unwrap().clone();
                        if !raw_resp.is_empty() {
                            self.session
                                .lock()
                                .unwrap()
                                .set_model_response(self.sanitize_model_response_if_block(&raw_resp));
                        }
                        break;
                    }
                    if actions {
                        let results_payload = self.build_results_payload();
                        self.session.lock().unwrap().start_turn(&results_payload);
                    }
                    continue;
                }

                let mut provider_headers: Option<Value> = None;
                let mut provider_api_key: Option<String> = None;

                if loop_count == 1 {
                    let sys_prompt = self
                        .session
                        .lock()
                        .unwrap()
                        .system_prompt
                        .clone()
                        .unwrap_or_default();
                    let input_text = self
                        .session
                        .lock()
                        .unwrap()
                        .turns
                        .last()
                        .map(|turn| turn.input.clone())
                        .unwrap_or_default();

                    let handler = self.middleware_event_handler.lock().unwrap().clone();
                    let mut context = self.build_event_context(
                        &self.ir.name,
                        None,
                        Some(sys_prompt.clone()),
                    );
                    // Populate request metadata for middleware inspection
                    context.model = Some(model_name.clone());
                    context.provider = Some(provider_id.clone());
                    context.config = config_params.clone();
                    context.headers = provider_headers.clone();
                    context.api_key = provider_api_key.clone();
                    if provider_type == "custom" {
                        context.url = model_info.get("url").and_then(Value::as_str).map(ToString::to_string);
                    }

                    let payload = LlmStartPayload {
                        prompt: input_text.clone(),
                        context,
                    };

                    if let Some(middleware_result) =
                        middleware::apply_llm_start_middleware(handler, payload).await
                    {
                        let parsed = middleware::parse_llm_start_response(&middleware_result);

                        if let Some(modified) = parsed.prompt {
                            self.session
                                .lock()
                                .unwrap()
                                .set_display_input(modified);
                        }

                        if let Some(stack_vec) = parsed.stack {
                            if !stack_vec.is_empty() {
                                let mut session = self.session.lock().unwrap();
                                session.stack = stack_vec;

                                if session.stack.len() > 1 {
                                    *self.fast_forward_stack.lock().unwrap() =
                                        Some(session.stack[1..].to_vec());
                                    drop(session);
                                    continue;
                                }
                            }
                        }

                        // Apply config override (deep merge)
                        if let Some(ref override_config) = parsed.config {
                            config_params = Some(deep_merge_json(
                                config_params.unwrap_or_else(|| serde_json::json!({})),
                                override_config.clone(),
                            ));
                        }

                        // Apply provider override
                        if let Some(new_provider) = parsed.provider {
                            provider_id = new_provider;
                        }

                        // Apply model override
                        if let Some(new_model) = parsed.model {
                            model_name = new_model;
                        }

                        // Apply URL override into config so drivers can read it
                        if let Some(url_override) = parsed.url {
                            config_params = Some(deep_merge_json(
                                config_params.unwrap_or_else(|| serde_json::json!({})),
                                serde_json::json!({ "url": url_override }),
                            ));
                        }

                        // Apply headers
                        if parsed.headers.is_some() {
                            provider_headers = parsed.headers;
                        }

                        // Apply api_key
                        if parsed.api_key.is_some() {
                            provider_api_key = parsed.api_key;
                        }
                    }
                    timing.mark("llm_start middleware complete");

                    if self.middleware_event_handler.lock().unwrap().is_some() {
                        let system_prompt = self.generate_prompt(None)?;
                        self.session
                            .lock()
                            .unwrap()
                            .set_system_prompt(&system_prompt);
                        timing.mark("regenerated prompt after llm_start middleware");
                    }
                }

                let is_native = self.resolve_tool_protocol() == "native";

                // Build provider messages based on protocol mode
                let messages = if is_native {
                    self.session.lock().unwrap().to_messages_native_openai()
                } else {
                    let binding_block = self.render_binding_block();
                    self.session
                        .lock()
                        .unwrap()
                        .to_messages_with_bindings(binding_block.clone())
                };
                timing.mark("built provider messages");

                // Inject native tools and output schema into config when in native mode.
                // OpenAI rejects requests that combine tools with response_format,
                // so we skip the output schema when tools are present.
                let config_params = if is_native {
                    let mut config = config_params.clone().unwrap_or_else(|| serde_json::json!({}));
                    let registry = self.native_registry();
                    match provider_id.as_str() {
                        "openai" | "groq" | "custom" => {
                            let tools = registry.openai_tools();
                            if !tools.is_empty() {
                                config["auwgent_native_tools"] = serde_json::json!(tools);
                            }
                            // Only inject output schema when no tools are present
                            if tools.is_empty() {
                                if let Some(fmt) = registry.openai_output_format() {
                                    config["auwgent_native_output_schema"] = fmt;
                                }
                            }
                        }
                        "gemini" => {
                            let tools = registry.gemini_tools();
                            if !tools.is_empty() {
                                config["auwgent_native_tools"] = serde_json::json!(tools);
                            }
                            if let Some(fmt) = registry.gemini_output_format() {
                                config["auwgent_native_output_schema"] = fmt;
                            }
                        }
                        _ => {}
                    }
                    Some(config)
                } else {
                    config_params.clone()
                };

                let stream_res = {
                    let driver = self
                        .drivers
                        .lock()
                        .unwrap()
                        .get(&provider_id)
                        .ok_or(AuwgentError::NoDriver)?
                        .clone();
                    driver
                        .stream_generate(&model_name, &messages, config_params, provider_headers.clone(), provider_api_key.clone())
                        .await
                };
                timing.mark("provider stream_generate returned stream");

                let mut stream = match stream_res {
                    Ok(stream) => {
                        self.reset_force_start_retry_count();
                        stream
                    }
                    Err(error) => {
                        self.fire_intent(
                            "error".to_string(),
                            serde_json::json!({ "message": &error }),
                        )
                        .await;

                        // Fire error middleware with forceStart support
                        let handler = self.middleware_event_handler.lock().unwrap().clone();
                        let payload = ErrorPayload {
                            context: self.build_event_context(&self.ir.name, None, None),
                            session: self
                                .export_session()
                                .ok()
                                .and_then(|session| serde_json::from_str::<Value>(&session).ok()),
                            error: serde_json::json!({ "message": &error }),
                        };
                        let middleware_response =
                            middleware::apply_error_middleware(handler, payload).await;
                        let decision = middleware_response
                            .as_ref()
                            .map(|r| middleware::parse_error_response(r))
                            .unwrap_or_default();

                        if decision.swallow {
                            return Ok(());
                        }

                        match decision.force_start.as_deref() {
                            Some("llm_start") => {
                                let retry_count = self.increment_force_start_retry();
                                if retry_count > MAX_FORCE_START_RETRIES {
                                    return Err(AuwgentError::Driver(format!(
                                        "forceStart 'llm_start' exceeded max retries ({})",
                                        MAX_FORCE_START_RETRIES
                                    )));
                                }
                                // Reset turn artifacts and retry
                                self.reset_turn_state();
                                self.session.lock().unwrap().pop_last_turn_if_empty();
                                continue;
                            }
                            Some("run_start") => {
                                let retry_count = self.increment_force_start_retry();
                                if retry_count > MAX_FORCE_START_RETRIES {
                                    return Err(AuwgentError::Driver(format!(
                                        "forceStart 'run_start' exceeded max retries ({})",
                                        MAX_FORCE_START_RETRIES
                                    )));
                                }
                                // Reset run state and retry from beginning
                                self.reset_run_state();
                                continue;
                            }
                            _ => {
                                self.session
                                    .lock()
                                    .unwrap()
                                    .set_model_response(format!("(request error: {})", error));
                                return Err(AuwgentError::Driver(error));
                            }
                        }
                    }
                };

                let mut actions_performed = false;
                let mut turn_usage = TokenUsage::default();
                let mut turn_finish_reason = None;
                let mut native_tool_calls: Vec<crate::runtime::session::NativeToolCallRecord> =
                    Vec::new();
                let mut native_structured_output: Option<Value> = None;

                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(ModelEvent::ContentChunk(text)) => {
                            if !text.is_empty() {
                                self.current_raw_response.lock().unwrap().push_str(&text);
                            }
                            if !is_native {
                                self.orchestrator.lock().unwrap().write(&text);
                            }

                            let (_terminal, actions, hard_stop) = match self.process_intents().await
                            {
                                Ok(result) => result,
                                Err(error) => {
                                    let raw_resp =
                                        self.current_raw_response.lock().unwrap().clone();
                                    if !raw_resp.is_empty() {
                                        self.session
                                            .lock()
                                            .unwrap()
                                            .set_model_response(self.sanitize_model_response_if_block(&raw_resp));
                                    }
                                    return Err(error);
                                }
                            };
                            if actions {
                                actions_performed = true;
                            }
                            if hard_stop {
                                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                                if !raw_resp.is_empty() {
                                    self.session
                                        .lock()
                                        .unwrap()
                                        .set_model_response(self.sanitize_model_response_if_block(&raw_resp));
                                }
                                break;
                            }
                        }
                        Ok(ModelEvent::Usage(usage)) => turn_usage = usage,
                        Ok(ModelEvent::FinishReason(reason)) => turn_finish_reason = Some(reason),
                        Ok(ModelEvent::Metadata(meta)) => {
                            turn_usage = meta.usage;
                            turn_finish_reason = meta.finish_reason;
                        }
                        Ok(ModelEvent::NativeToolCall {
                            id,
                            provider_name,
                            arguments,
                        }) => {
                            if let Some((action_kind, canonical_name)) =
                                crate::runtime::engine::native_registry::NativeCallableRegistry::route(
                                    &provider_name,
                                )
                            {
                                let canonical_name = canonical_name.to_string();
                                let intent_name = match action_kind {
                                    crate::runtime::engine::native_registry::ActionKind::Tool => {
                                        "tool_call"
                                    }
                                    crate::runtime::engine::native_registry::ActionKind::Workflow => {
                                        "workflow_call"
                                    }
                                    crate::runtime::engine::native_registry::ActionKind::Helper => {
                                        "helper_call"
                                    }
                                };
                                let intent_value = serde_json::json!({
                                    "type": &canonical_name,
                                    "args": arguments
                                });
                                self.pending_intents
                                    .lock()
                                    .unwrap()
                                    .push((intent_name.to_string(), intent_value));
                                let (_terminal, actions, hard_stop) =
                                    match self.process_intents().await {
                                        Ok(result) => result,
                                        Err(error) => {
                                            let raw_resp = self
                                                .current_raw_response
                                                .lock()
                                                .unwrap()
                                                .clone();
                                            if !raw_resp.is_empty() {
                                                self.session.lock().unwrap().set_model_response(
                                                    self.sanitize_model_response_if_block(&raw_resp),
                                                );
                                            }
                                            return Err(error);
                                        }
                                    };
                                if actions {
                                    actions_performed = true;
                                }
                                if hard_stop {
                                    let raw_resp =
                                        self.current_raw_response.lock().unwrap().clone();
                                    if !raw_resp.is_empty() {
                                        self.session.lock().unwrap().set_model_response(
                                            self.sanitize_model_response_if_block(&raw_resp),
                                        );
                                    }
                                    break;
                                }
                                native_tool_calls.push(
                                    crate::runtime::session::NativeToolCallRecord {
                                        id,
                                        provider_name,
                                        canonical_name,
                                        action_kind: intent_name.to_string(),
                                        arguments,
                                    },
                                );
                            }
                        }
                        Ok(ModelEvent::NativeStructuredOutput(value)) => {
                            let payload = self.native_response_schema_payload(value.clone());
                            let (_terminal, _actions, hard_stop) = match self
                                .process_synthetic_intent("response_schema", payload)
                                .await
                            {
                                Ok(result) => result,
                                Err(error) => {
                                    let raw_resp =
                                        self.current_raw_response.lock().unwrap().clone();
                                    if !raw_resp.is_empty() {
                                        self.session.lock().unwrap().set_model_response(
                                            self.sanitize_model_response_if_block(&raw_resp),
                                        );
                                    }
                                    return Err(error);
                                }
                            };
                            native_structured_output = Some(value);
                            if hard_stop {
                                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                                if !raw_resp.is_empty() {
                                    self.session.lock().unwrap().set_model_response(
                                        self.sanitize_model_response_if_block(&raw_resp),
                                    );
                                }
                                break;
                            }
                        }
                        Err(error) => {
                            self.fire_intent(
                                "error".to_string(),
                                serde_json::json!({ "message": error.clone() }),
                            )
                            .await;
                            self.session
                                .lock()
                                .unwrap()
                                .set_model_response(format!("(error: {})", error));
                            return Err(AuwgentError::StreamError(error));
                        }
                    }
                }
                timing.mark("provider stream drained");

                let turn_metadata = TurnMetadata {
                    turn_index: loop_count - 1,
                    usage: turn_usage.clone(),
                    finish_reason: turn_finish_reason.clone(),
                    model: model_name.to_string(),
                };

                let _final_val = self.orchestrator.lock().unwrap().end();

                let (_terminal, actions, hard_stop) = match self.process_intents().await {
                    Ok(result) => result,
                    Err(error) => {
                        let raw_resp = self.current_raw_response.lock().unwrap().clone();
                        if !raw_resp.is_empty() {
                            self.session
                                .lock()
                                .unwrap()
                                .set_model_response(self.sanitize_model_response_if_block(&raw_resp));
                        }
                        return Err(error);
                    }
                };
                if actions {
                    actions_performed = true;
                }

                let sys_prompt = self
                    .session
                    .lock()
                    .unwrap()
                    .system_prompt
                    .clone()
                    .unwrap_or_default();
                let context = self.build_event_context(&self.ir.name, None, Some(sys_prompt));
                let handler = self.middleware_event_handler.lock().unwrap().clone();
                middleware::notify_llm_end_middleware(
                    handler,
                    &self.last_turn_response_value(),
                    context,
                    &turn_metadata,
                )
                .await;
                timing.mark("llm_end middleware complete");

                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                if !raw_resp.is_empty() {
                    self.session
                        .lock()
                        .unwrap()
                        .set_model_response(self.sanitize_model_response_if_block(&raw_resp));
                }

                // Native mode: if no terminal intent was emitted and we have raw text,
                // treat it as response_text (or response_schema if JSON and output schema configured)
                if is_native && !raw_resp.is_empty() && !*self.terminal_response_emitted.lock().unwrap() {
                    let has_output_schema = self.ir.output.is_some();
                    let synthetic_terminal = if has_output_schema {
                        // Try to parse as JSON for structured output
                        if let Ok(value) = serde_json::from_str::<Value>(&raw_resp) {
                            (
                                "response_schema".to_string(),
                                self.native_response_schema_payload(value),
                            )
                        } else {
                            let payload = serde_json::json!({ "text": raw_resp.clone() });
                            ("response_text".to_string(), payload)
                        }
                    } else {
                        let payload = serde_json::json!({ "text": raw_resp.clone() });
                        ("response_text".to_string(), payload)
                    };

                    let (_terminal, actions, hard_stop) = match self
                        .process_synthetic_intent(synthetic_terminal.0, synthetic_terminal.1)
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => {
                            if !raw_resp.is_empty() {
                                self.session.lock().unwrap().set_model_response(
                                    self.sanitize_model_response_if_block(&raw_resp),
                                );
                            }
                            return Err(error);
                        }
                    };
                    if actions {
                        actions_performed = true;
                    }
                    if hard_stop {
                        break;
                    }
                }

                // Store native turn data for session reconstruction.
                // Only populate nativeAssistantTurn when there are actual native-specific
                // artifacts (tool calls or structured output). Plain text lives in model_response.
                if is_native {
                    let mut session = self.session.lock().unwrap();
                    session.set_turn_protocol("native");
                    if !native_tool_calls.is_empty() || native_structured_output.is_some() {
                        session.set_native_assistant_turn(
                            crate::runtime::session::NativeAssistantTurn {
                                text_content: if raw_resp.is_empty() {
                                    None
                                } else {
                                    Some(raw_resp.clone())
                                },
                                tool_calls: native_tool_calls.clone(),
                                structured_output: native_structured_output.clone(),
                            },
                        );
                    }
                }

                let emitted_terminal = *self.terminal_response_emitted.lock().unwrap();
                let emitted_final = *self.final_response_emitted.lock().unwrap();
                let should_retry_empty = should_retry_empty_completion(
                    &raw_resp,
                    actions_performed,
                    emitted_terminal,
                    emitted_final,
                    turn_finish_reason.as_ref(),
                );
                if should_retry_empty && empty_completion_retries < MAX_EMPTY_COMPLETION_RETRIES {
                    empty_completion_retries += 1;
                    runtime_sleep(Duration::from_millis(EMPTY_COMPLETION_RETRY_DELAY_MS)).await;
                    continue;
                }
                if !should_retry_empty {
                    empty_completion_retries = 0;
                }

                {
                    let mut meta_lock = self.last_run_metadata.lock().unwrap();
                    meta_lock.aggregate.prompt_tokens += turn_usage.prompt_tokens;
                    meta_lock.aggregate.completion_tokens += turn_usage.completion_tokens;
                    meta_lock.aggregate.total_tokens += turn_usage.total_tokens;
                    meta_lock.aggregate.reasoning_tokens += turn_usage.reasoning_tokens;
                    meta_lock.aggregate.cached_tokens += turn_usage.cached_tokens;
                    meta_lock.turns.push(turn_metadata.clone());
                }

                if hard_stop {
                    break;
                }

                if !actions_performed {
                    let mut session = self.session.lock().unwrap();
                    if let Some(turn) = session.current_turn_mut()
                        && turn.model_response.is_empty()
                    {
                        turn.model_response = empty_response_marker(turn_finish_reason.as_ref());
                    }
                    break;
                }

                if is_native {
                    // Store native tool results in the current turn for message reconstruction
                    let results = self.pending_tool_results.lock().unwrap().clone();
                    let mut session = self.session.lock().unwrap();
                    for (result_key, args, result) in results {
                        // Match result to a native tool call by canonical name
                        let canonical_name = result_key
                            .strip_prefix("workflow:")
                            .or_else(|| result_key.strip_prefix("helper:"))
                            .unwrap_or(&result_key)
                            .to_string();
                        let action_kind = if result_key.starts_with("workflow:") {
                            "workflow_call"
                        } else if result_key.starts_with("helper:") {
                            "helper_call"
                        } else {
                            "tool_call"
                        };
                        let call_id = native_tool_calls
                            .iter()
                            .find(|tc| {
                                tc.canonical_name == canonical_name && tc.action_kind == action_kind
                            })
                            .and_then(|tc| tc.id.clone());
                        let provider_name = native_tool_calls
                            .iter()
                            .find(|tc| {
                                tc.canonical_name == canonical_name && tc.action_kind == action_kind
                            })
                            .map(|tc| tc.provider_name.clone())
                            .unwrap_or_else(|| format!("{}_{}", action_kind.replace("_call", ""), canonical_name));
                        session.append_native_tool_result(
                            crate::runtime::session::NativeToolResult {
                                call_id,
                                provider_name,
                                canonical_name,
                                action_kind: action_kind.to_string(),
                                arguments: args,
                                result,
                            },
                        );
                    }
                    drop(session);
                    // Start an empty turn to trigger the next model call
                    // The tool results will be included in message history via to_messages_native_openai
                    self.session.lock().unwrap().start_turn("");
                } else {
                    let results_payload = self.build_results_payload();
                    self.session.lock().unwrap().start_turn(&results_payload);
                }
            }

            Ok(())
        }
        .await;

        match run_result {
            Ok(()) => {
                self.emit_structured_stream_finish();
                let handler = self.middleware_event_handler.lock().unwrap().clone();
                let payload = RunCompletePayload {
                    session: serde_json::from_str::<Value>(&self.export_session()?)
                        .map_err(AuwgentError::Serialization)?,
                    context: self.build_event_context(&self.ir.name, None, None),
                };
                let _ = middleware::apply_run_complete_middleware(handler, payload).await;
                timing.mark("run_complete middleware complete");
                Ok(())
            }
            Err(err) => {
                self.emit_structured_stream_error(err.to_string());
                {
                    let mut session = self.session.lock().unwrap();
                    if let Some(turn) = session.current_turn_mut()
                        && turn.model_response.is_empty()
                    {
                        turn.model_response = format!("(error: {})", err);
                    }
                }

                let handler = self.middleware_event_handler.lock().unwrap().clone();
                let payload = ErrorPayload {
                    context: self.build_event_context(&self.ir.name, None, None),
                    session: self
                        .export_session()
                        .ok()
                        .and_then(|session| serde_json::from_str::<Value>(&session).ok()),
                    error: serde_json::json!({ "message": err.to_string() }),
                };

                let middleware_response =
                    middleware::apply_error_middleware(handler, payload).await;
                if middleware_response
                    .as_ref()
                    .and_then(|response| response.get("swallow"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                Err(err)
            }
        }
    }

    pub fn write_llm_chunk(&self, chunk: &str) {
        self.orchestrator.lock().unwrap().write(chunk);
    }

    pub fn end_llm_stream(&self) -> Value {
        self.orchestrator.lock().unwrap().end()
    }

    fn sync_fast_forward_from_session(&self) {
        let session = self.session.lock().unwrap();
        if session.stack.len() > 1 {
            *self.fast_forward_stack.lock().unwrap() = Some(session.stack[1..].to_vec());
        } else {
            *self.fast_forward_stack.lock().unwrap() = None;
        }
    }

    fn last_turn_response_value(&self) -> Value {
        self.last_turn_response_value.lock().unwrap().clone()
    }
}

pub(super) fn sanitize_model_response(raw: &str) -> String {
    let mut scanner = function_parser::BlockScanner::new(raw);
    let blocks = scanner.scan();

    if blocks.iter().any(|block| block.raw.starts_with('[')) {
        blocks
            .into_iter()
            .filter(|block| block.raw.starts_with('['))
            .map(|block| block.raw)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        raw.trim().to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> u128 {
    js_sys::Date::now() as u128
}

struct RuntimeTimingProbe {
    label: &'static str,
    start_ms: u128,
    enabled: bool,
}

impl RuntimeTimingProbe {
    fn new(label: &'static str) -> Self {
        let enabled = runtime_timing_enabled();
        let probe = Self {
            label,
            start_ms: current_time_ms(),
            enabled,
        };
        if enabled {
            eprintln!("[auwgent][timing][rust] {} +0ms start", label);
        }
        probe
    }

    fn mark(&self, message: &str) {
        if self.enabled {
            let elapsed = current_time_ms().saturating_sub(self.start_ms);
            eprintln!(
                "[auwgent][timing][rust] {} +{}ms {}",
                self.label, elapsed, message
            );
        }
    }
}

fn runtime_timing_enabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("AUWGENT_DEBUG_TIMING")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_model_response;

    #[test]
    fn sanitized_model_response_drops_orphan_text_before_protocol_block() {
        assert_eq!(
            sanitize_model_response(
                "  I will rather use the [response_text]It says your balance is 40.00 GHS[/response_text]"
            ),
            "[response_text]It says your balance is 40.00 GHS[/response_text]"
        );
    }
}
