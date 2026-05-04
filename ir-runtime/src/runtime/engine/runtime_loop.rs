// Runtime loop and middleware coordination.
// This file owns the agent run loop, stream handling, lifecycle middleware,
// and event-context helpers. Execution of tools/workflows/helpers belongs in
// the execution modules instead of here.
use super::*;

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
            let user_text = match &user_input {
                Value::String(text) => text.clone(),
                value => serde_json::to_string(value).map_err(AuwgentError::Serialization)?,
            };
            *self.user_input.lock().unwrap() = Some(user_input);
            self.session.lock().unwrap().start_turn(user_text);
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
            if let Some(modified) = middleware_result.get("prompt").and_then(Value::as_str) {
                next_prompt = modified.to_string();
                self.session.lock().unwrap().set_input(next_prompt.clone());
            }

            if let Some(new_stack) = middleware_result.get("stack").and_then(Value::as_array) {
                let stack_vec: Vec<String> = new_stack
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
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
        let provider_id = if provider_type == "custom" {
            model_info["id"].as_str().unwrap_or("custom")
        } else {
            provider_type
        };
        let model_name = model_info["modelName"]
            .as_str()
            .unwrap_or("gemini-2.0-flash");
        let config_params = model_info.get("config").cloned();

        let mut system_prompt = self.generate_prompt(None)?;
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
            system_prompt = self.generate_prompt(None)?;
            self.session
                .lock()
                .unwrap()
                .set_system_prompt(&system_prompt);
        }

        let initial_user_input = match input.as_ref() {
            Some(Value::String(text)) => Some(text.clone()),
            Some(value) => Some(serde_json::to_string(value).map_err(AuwgentError::Serialization)?),
            None => None,
        };

        *self.terminal_response_emitted.lock().unwrap() = false;
        *self.final_response_emitted.lock().unwrap() = false;
        self.emit_structured_stream_start();

        let is_teleporting = self.fast_forward_stack.lock().unwrap().is_some();
        if let Some(user_text) = initial_user_input.clone() {
            *self.user_input.lock().unwrap() = Some(Value::String(user_text.clone()));
            if !is_teleporting {
                self.session.lock().unwrap().start_turn(&user_text);
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
                            self.session.lock().unwrap().set_model_response(&raw_resp);
                        }
                        break;
                    }
                    if actions {
                        let results_payload = self.build_results_payload();
                        self.session.lock().unwrap().start_turn(&results_payload);
                    }
                    continue;
                }

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
                    let payload = LlmStartPayload {
                        prompt: input_text.clone(),
                        context: self.build_event_context(
                            &self.ir.name,
                            None,
                            Some(sys_prompt.clone()),
                        ),
                    };

                    if let Some(middleware_result) =
                        middleware::apply_llm_start_middleware(handler, payload).await
                    {
                        if let Some(modified) =
                            middleware_result.get("prompt").and_then(Value::as_str)
                        {
                            self.session.lock().unwrap().set_input(modified.to_string());
                        }

                        if let Some(new_stack) =
                            middleware_result.get("stack").and_then(Value::as_array)
                        {
                            let stack_vec: Vec<String> = new_stack
                                .iter()
                                .filter_map(|value| value.as_str().map(ToString::to_string))
                                .collect();
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
                    }

                    if self.middleware_event_handler.lock().unwrap().is_some() {
                        let system_prompt = self.generate_prompt(None)?;
                        self.session
                            .lock()
                            .unwrap()
                            .set_system_prompt(&system_prompt);
                    }
                }

                let messages = self.session.lock().unwrap().to_messages();
                let stream_res = {
                    let driver = self
                        .drivers
                        .lock()
                        .unwrap()
                        .get(provider_id)
                        .ok_or(AuwgentError::NoDriver)?
                        .clone();
                    driver
                        .stream_generate(model_name, &messages, config_params.clone())
                        .await
                };

                let mut stream = match stream_res {
                    Ok(stream) => stream,
                    Err(error) => {
                        self.fire_intent(
                            "error".to_string(),
                            serde_json::json!({ "message": error }),
                        )
                        .await;
                        self.session
                            .lock()
                            .unwrap()
                            .set_model_response(format!("(request error: {})", error));
                        return Err(AuwgentError::Driver(error));
                    }
                };

                let mut actions_performed = false;
                let mut turn_usage = TokenUsage::default();
                let mut turn_finish_reason = None;

                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(ModelEvent::ContentChunk(text)) => {
                            if !text.is_empty() {
                                self.current_raw_response.lock().unwrap().push_str(&text);
                            }
                            self.orchestrator.lock().unwrap().write(&text);

                            let (_terminal, actions, hard_stop) = match self.process_intents().await
                            {
                                Ok(result) => result,
                                Err(error) => {
                                    let raw_resp =
                                        self.current_raw_response.lock().unwrap().clone();
                                    if !raw_resp.is_empty() {
                                        self.session.lock().unwrap().set_model_response(&raw_resp);
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
                                    self.session.lock().unwrap().set_model_response(&raw_resp);
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
                            self.session.lock().unwrap().set_model_response(&raw_resp);
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

                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                if !raw_resp.is_empty() {
                    self.session.lock().unwrap().set_model_response(&raw_resp);
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
                    sleep(Duration::from_millis(EMPTY_COMPLETION_RETRY_DELAY_MS)).await;
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

                let results_payload = self.build_results_payload();
                self.session.lock().unwrap().start_turn(&results_payload);
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
