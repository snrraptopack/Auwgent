// Helper execution.
// This file owns sub-engine setup, helper handoff semantics, helper session
// preload/save hooks, and stack-aware helper resumption behavior.
use super::*;

impl AuwgentEngine {
    pub(super) fn execute_helper<'a>(
        &'a self,
        call: &'a Value,
    ) -> futures_util::future::BoxFuture<'a, AuwgentResult<(String, Value, Value)>> {
        let session_preload_handler = self.session_preload_handler.clone();
        let session_save_handler = self.session_save_handler.clone();

        Box::pin(async move {
            use crate::runtime::helper_runner::{build_sub_agent_context, HandoffMode};

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
                            unless you have distinct task that is differ from what we have completed entirely.
                            you can end by providing a useful comment to the user
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

