use super::*;

impl AuwgentEngine {
    /// Build a structured payload of tool/workflow results to feed back to
    /// the LLM on the next turn using result blocks.
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

    /// Fire the user's intent callback (if registered).
    /// Returns the control signal, or None if no handler / handler returns None.
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

            // Strip _raw before internal processing (tool execution, etc.)
            if let Value::Object(ref mut map) = value {
                map.remove("_raw");
            }

            // Emit framework-agnostic structured output as JSONL events.
            self.emit_structured_intent(name.clone(), value.clone());

            match name.as_str() {
                "tool_call" => {
                    match control {
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
                    }
                }
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

        self.pending_tool_results
            .lock()
            .unwrap()
            .extend(tool_results);

        Ok((has_terminal, has_actions, hard_stop))
    }

    async fn execute_tool(&self, call: &Value) -> AuwgentResult<(String, Value, Value)> {
        let tool_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let imp = self.tools.lock().unwrap().get(&tool_name).cloned();
        if let Some(imp) = imp {
            match imp(args.clone()).await {
                Ok(val) => Ok((tool_name, args, val)),
                Err(e) => {
                    let error_value = serde_json::json!({
                        "tool": tool_name,
                        "message": e,
                    });
                    self.fire_intent("tool_error".to_string(), error_value.clone())
                        .await;
                    let _ = self
                        .fire_middleware_event(serde_json::json!({
                            "type": "error",
                            "error": {
                                "kind": "tool_error",
                                "tool": tool_name,
                                "message": e,
                            },
                            "session": Value::Null,
                            "context": self.build_event_context(&self.ir.name, None, None),
                        }))
                        .await;
                    Ok((tool_name, args, serde_json::json!({ "error": e })))
                }
            }
        } else {
            let message = format!("Tool not found: {}", tool_name);
            self.fire_intent(
                "tool_error".to_string(),
                serde_json::json!({
                    "tool": tool_name,
                    "message": message,
                }),
            )
            .await;
            let _ = self
                .fire_middleware_event(serde_json::json!({
                    "type": "error",
                    "error": {
                        "kind": "tool_error",
                        "tool": tool_name,
                        "message": message,
                    },
                    "session": Value::Null,
                    "context": self.build_event_context(&self.ir.name, None, None),
                }))
                .await;
            Ok((
                tool_name.clone(),
                args,
                serde_json::json!({ "error": format!("Tool '{}' is not registered", tool_name) }),
            ))
        }
    }

    async fn execute_workflow(&self, call: &Value) -> AuwgentResult<(String, Value, Value)> {
        let wf_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let body_clone = {
            let wf = match self.ir.workflows.iter().find(|w| w.name == wf_name) {
                Some(w) => w,
                None => {
                    return Ok((
                        wf_name.clone(),
                        args.clone(),
                        serde_json::json!({ "error": format!("Workflow not found: {}", wf_name) }),
                    ));
                }
            };
            wf.body.clone()
        };

        let mut tool_fns: HashMap<String, crate::evaluator::SyncToolFn> = HashMap::new();
        {
            let tools = self.tools.lock().unwrap();
            let ir_tools = &self.ir.tools;

            for (name, imp) in &*tools {
                let imp = imp.clone();
                let name_clone = name.clone();

                let param_names: Vec<String> = ir_tools
                    .iter()
                    .find(|t| t.name == *name)
                    .and_then(|t| t.params.0.as_object())
                    .map(|params: &serde_json::Map<String, Value>| {
                        let mut names: Vec<_> = params.keys().cloned().collect();
                        names.sort();
                        names
                    })
                    .unwrap_or_default();

                tool_fns.insert(
                    name.clone(),
                    std::sync::Arc::new(move |fn_args: Vec<Value>| {
                        let arg_val = if param_names.is_empty() {
                            if fn_args.len() == 1 {
                                fn_args.into_iter().next().unwrap_or(Value::Null)
                            } else {
                                Value::Array(fn_args)
                            }
                        } else {
                            let mut args_obj: serde_json::Map<String, Value> =
                                serde_json::Map::new();
                            for (i, param_name) in param_names.iter().enumerate() {
                                if let Some(arg_value) = fn_args.get(i) {
                                    args_obj.insert(param_name.clone(), arg_value.clone());
                                }
                            }
                            Value::Object(args_obj)
                        };

                        let rt = tokio::runtime::Handle::current();
                        let imp = imp.clone();
                        std::thread::spawn(move || rt.block_on(imp(arg_val)))
                            .join()
                            .map_err(|_| format!("Tool '{}' panicked", name_clone))?
                    }),
                );
            }
        }

        let ir_clone = self.ir.clone();

        let mut scope = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                scope.insert(k.clone(), v.clone());
            }
        }
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let mut last_result = Value::Null;
        for stmt in &body_clone {
            let eval_result = {
                let evaluator = Evaluator::with_tools(&ir_clone, tool_fns.clone());
                evaluator.evaluate(stmt, &mut scope)?
            };

            if let Some(obj) = eval_result.as_object() {
                if obj
                    .get("__requires_async_helper_call")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj
                        .get("helper_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let helper_args = obj.get("args").unwrap_or(&Value::Null).clone();

                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": helper_args
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;
                    last_result = sub_result;
                    continue;
                }

                if obj
                    .get("__requires_async_transfer")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": {}
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;

                    if let Some(res_obj) = sub_result.as_object()
                        && res_obj
                            .get("__handoff_stop")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        return Ok((wf_name, args, sub_result));
                    }

                    last_result = sub_result;
                    continue;
                }
            }

            last_result = eval_result;
        }

        Ok((wf_name, args, last_result))
    }

    fn execute_helper<'a>(
        &'a self,
        call: &'a Value,
    ) -> futures_util::future::BoxFuture<'a, AuwgentResult<(String, Value, Value)>> {
        let session_preload_handler = self.session_preload_handler.clone();
        let session_save_handler = self.session_save_handler.clone();

        Box::pin(async move {
            use crate::runtime::helper_runner::{HandoffMode, build_sub_agent_context};

            let helper_name = call["type"].as_str().unwrap_or("").to_string();
            let args = call["args"].clone();

            {
                let mut session = self.session.lock().unwrap();
                let is_teleporting = {
                    let ffs_lock = self.fast_forward_stack.lock().unwrap();
                    ffs_lock.is_some()
                };

                if !is_teleporting {
                    session.stack.push(helper_name.clone());
                }
            }

            let sub_ctx = build_sub_agent_context(&self.ir, &helper_name)?;

            let helper_tool_names: Vec<String> =
                sub_ctx.ir.tools.iter().map(|t| t.name.clone()).collect();

            let mut helper_workflow_tool_names: Vec<String> = Vec::new();
            for workflow in &sub_ctx.ir.workflows {
                for tool in &workflow.tools {
                    helper_workflow_tool_names.push(tool.name.clone());
                }
            }

            let sub_engine = AuwgentEngine::new(sub_ctx.ir);

            {
                let drivers = self.drivers.lock().unwrap();
                let mut sub_drivers = sub_engine.drivers.lock().unwrap();
                for (provider_type, driver) in &*drivers {
                    sub_drivers.insert(provider_type.clone(), Arc::clone(driver));
                }
            }

            {
                let tools = self.tools.lock().unwrap();
                let mut sub_tools = sub_engine.tools.lock().unwrap();

                for tool_name in &sub_ctx.authorized_parent_tool_names {
                    if let Some(imp) = tools.get(tool_name) {
                        sub_tools.insert(tool_name.clone(), Arc::clone(imp));
                    }
                }

                for tool_name in &helper_tool_names {
                    if let Some(imp) = tools.get(tool_name) {
                        sub_tools.insert(tool_name.clone(), Arc::clone(imp));
                    }
                }

                for tool_name in &helper_workflow_tool_names {
                    if let Some(imp) = tools.get(tool_name) {
                        sub_tools.insert(tool_name.clone(), Arc::clone(imp));
                    }
                }
            }

            if let Some(ctx) = self.context.lock().unwrap().as_ref() {
                sub_engine.set_context(ctx.clone());
            }

            match sub_ctx.handoff_mode {
                HandoffMode::User | HandoffMode::ThenContinue => {
                    if let Some(handler) = self.intent_handler.lock().unwrap().as_ref() {
                        sub_engine.on_intent(Arc::clone(handler));
                    }
                    if let Some(handler) = self.partial_intent_handler.lock().unwrap().as_ref() {
                        sub_engine.on_intent_partial(Arc::clone(handler));
                    }
                }
                HandoffMode::Return => {}
            }

            {
                let h = self.llm_start_handler.lock().unwrap().clone();
                if let Some(handler) = h {
                    sub_engine.on_llm_start(handler);
                }
            }
            {
                let h = self.llm_end_handler.lock().unwrap().clone();
                if let Some(handler) = h {
                    sub_engine.on_llm_end(handler);
                }
            }
            {
                let h = self.run_start_handler.lock().unwrap().clone();
                if let Some(handler) = h {
                    sub_engine.on_run_start(handler);
                }
            }
            {
                let h = self.run_complete_handler.lock().unwrap().clone();
                if let Some(handler) = h {
                    sub_engine.on_run_complete(handler);
                }
            }
            {
                let h = self.error_handler.lock().unwrap().clone();
                if let Some(handler) = h {
                    sub_engine.on_error(handler);
                }
            }

            if let Ok(system_prompt) = sub_engine.generate_prompt(None) {
                sub_engine
                    .session
                    .lock()
                    .unwrap()
                    .set_system_prompt(&system_prompt);
            }

            let preload_fn = session_preload_handler.lock().unwrap().clone();
            if let Some(f) = preload_fn {
                let empty_session = sub_engine
                    .export_session()
                    .unwrap_or_else(|_| "{}".to_string());
                let _ = self
                    .fire_middleware_event(serde_json::json!({
                        "type": "sub_engine_start",
                        "helper": helper_name.clone(),
                        "session": serde_json::from_str::<Value>(&empty_session).unwrap_or(Value::Null),
                        "context": self.build_event_context(&helper_name, None, None),
                    }))
                    .await;
                if let Some(loaded_json) = f(helper_name.clone(), empty_session).await {
                    let _ = sub_engine.import_session(&loaded_json);
                }
            }

            let sub_initial_stack = {
                let mut ffs_lock = self.fast_forward_stack.lock().unwrap();
                let stack = ffs_lock.as_ref().and_then(|ffs| {
                    if ffs.first().map(|s| s.as_str()) == Some(helper_name.as_str()) {
                        let mut sub_stack = vec![helper_name.clone()];
                        sub_stack.extend_from_slice(&ffs[1..]);
                        Some(sub_stack)
                    } else {
                        None
                    }
                });
                if stack.is_some() {
                    *ffs_lock = None;
                }
                stack
            };

            let sub_input = {
                let mut user_input_lock = self.user_input.lock().unwrap();
                let is_resuming_at_target =
                    sub_initial_stack.as_ref().is_some_and(|s| s.len() == 1);

                if is_resuming_at_target {
                    user_input_lock.take()
                } else if sub_initial_stack.is_some() {
                    None
                } else {
                    Some(args.clone())
                }
            };

            sub_engine.run(sub_input, sub_initial_stack).await?;

            let save_fn = session_save_handler.lock().unwrap().clone();
            if let Some(f) = save_fn
                && let Ok(completed_json) = sub_engine.export_session()
            {
                f(helper_name.clone(), completed_json).await;
            }
            if let Ok(completed_json) = sub_engine.export_session() {
                let _ = self
                    .fire_middleware_event(serde_json::json!({
                        "type": "sub_engine_complete",
                        "helper": helper_name.clone(),
                        "session": serde_json::from_str::<Value>(&completed_json).unwrap_or(Value::Null),
                        "context": self.build_event_context(&helper_name, None, None),
                    }))
                    .await;
            }

            let final_resp = sub_engine
                .session
                .lock()
                .unwrap()
                .turns
                .last()
                .map(|t| t.model_response.clone())
                .unwrap_or_default();

            let emitted_terminal = *sub_engine.terminal_response_emitted.lock().unwrap();
            let emitted_final = *sub_engine.final_response_emitted.lock().unwrap();

            match sub_ctx.handoff_mode {
                HandoffMode::User => {
                    if emitted_final {
                        self.session.lock().unwrap().stack.pop();
                        Ok((helper_name, args, serde_json::json!({ "__handoff_stop": true })))
                    } else {
                        let _ = emitted_terminal;
                        Ok((helper_name, args, serde_json::json!({ "__handoff_stop": true })))
                    }
                }
                HandoffMode::ThenContinue => {
                    if emitted_final {
                        self.session.lock().unwrap().stack.pop();
                        let msg = serde_json::json!({
                            "status": "complete",
                            "note": format!("{} has responded to the user directly. No further action needed.
                            unless you have distinct that is differ from what we have completed entirely.
                            you can end by providing a useful commet to the user
                            ", &helper_name)
                        });
                        Ok((helper_name, args, msg))
                    } else {
                        let _ = emitted_terminal;
                        Ok((helper_name, args, serde_json::json!({ "__handoff_stop": true })))
                    }
                }
                HandoffMode::Return => {
                    if emitted_final {
                        self.session.lock().unwrap().stack.pop();
                        Ok((
                            helper_name,
                            args,
                            serde_json::json!({ "result": final_resp }),
                        ))
                    } else if emitted_terminal {
                        Ok((
                            helper_name,
                            args,
                            serde_json::json!({ "__handoff_stop": true }),
                        ))
                    } else {
                        self.session.lock().unwrap().stack.pop();
                        Ok((
                            helper_name,
                            args,
                            serde_json::json!({ "result": final_resp }),
                        ))
                    }
                }
            }
        })
    }
}
