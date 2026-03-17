use crate::errors::{AuwgentError, AuwgentResult};
use crate::evaluator::Evaluator;
use crate::intent_parser::orchestrator::Orchestrator;
use crate::runtime::drivers::ModelDriver;
use crate::runtime::session::SessionState;
use crate::types::*;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type ToolImplementation = Arc<
    dyn Fn(Value) -> futures_util::future::BoxFuture<'static, Result<Value, String>> + Send + Sync,
>;

// ═══════════════════════════════════════════════════════════════════════════
// INTENT EVENT TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Control returned by an intent handler to override default behavior.
#[derive(Debug, Clone)]
pub enum IntentControl {
    /// Skip this intent — don't execute the tool/workflow
    Skip,
    /// Use this result instead of executing the tool
    Override { result: Value },
}

/// Intent callback signature.
///
/// Receives the intent name and value. Returns:
///   - `None` → engine proceeds normally (auto-execute)
///   - `Some(IntentControl::Skip)` → skip this intent
///   - `Some(IntentControl::Override { result })` → use this result
pub type IntentCallback = Arc<dyn Fn(String, Value) -> Option<IntentControl> + Send + Sync>;

/// Async intent callback for handlers that need to await.
pub type AsyncIntentCallback = Arc<
    dyn Fn(String, Value) -> futures_util::future::BoxFuture<'static, Option<IntentControl>>
        + Send
        + Sync,
>;

/// Async callback for preloading a helper's session history before it runs.
/// Receives `(helper_name, empty_session_json)`. Returns an optional `SessionState` JSON string.
pub type AsyncSessionPreloadCallback = Arc<
    dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, Option<String>>
        + Send
        + Sync,
>;

/// Async callback for saving a helper's session history after it completes.
/// Receives `(helper_name, completed_session_json)`.
pub type SessionSaveCallback =
    Arc<dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync>;

/// Async callback that fires right before the LLM generates a response.
/// Receives the full resolved prompt/messages payload and the system prompt.
/// Returns an optional String to dynamically modify the user's prompt (Interceptor Pattern).
pub type AsyncLlmStartCallback = Arc<
    dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, Option<String>>
        + Send
        + Sync,
>;

/// Async callback that fires right after the LLM completes its generation stream.
/// Receives the full raw LLM response and the system prompt.
pub type AsyncLlmEndCallback =
    Arc<dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync>;

// ═══════════════════════════════════════════════════════════════════════════
// AUWGENT ENGINE
// ═══════════════════════════════════════════════════════════════════════════

pub struct AuwgentEngine {
    ir: AgentIR,
    session: SessionState,
    tools: HashMap<String, ToolImplementation>,
    orchestrator: Orchestrator,
    drivers: HashMap<String, Arc<dyn ModelDriver>>,
    context: Option<Value>,
    /// Pending intents collected by the orchestrator callback
    pending_intents: Arc<Mutex<Vec<(String, Value)>>>,
    /// Tool/workflow results accumulated during the current turn
    /// Used by build_results_payload() to feed results back to the LLM
    pending_tool_results: Vec<(String, Value)>,
    /// Accumulated raw response for the current turn
    current_raw_response: String,
    /// User-facing intent callback (fires on every completed intent)
    intent_handler: Option<AsyncIntentCallback>,
    /// User-facing partial intent callback (fires as data streams in)
    partial_intent_handler: Option<Arc<dyn Fn(String, Value) + Send + Sync>>,
    /// Hook for TypeScript to preload a helper session before sub_engine.run()
    session_preload_handler: Option<AsyncSessionPreloadCallback>,
    /// Hook for TypeScript to save a helper session after sub_engine.run()
    session_save_handler: Option<SessionSaveCallback>,
    /// Hook that fires before LLM generation
    llm_start_handler: Option<AsyncLlmStartCallback>,
    /// Hook that fires after LLM generation
    llm_end_handler: Option<AsyncLlmEndCallback>,
    /// Active fast-forward stack for Stack-Aware Resumption.
    /// When set, the engine bypasses LLM calls and jumps straight to the
    /// deepest agent in the stack, then resumes normal execution from there.
    fast_forward_stack: Option<Vec<String>>,
}

impl AuwgentEngine {
    pub fn new(ir: AgentIR) -> Self {
        let mut orchestrator = Orchestrator::new(None);

        // Register standard Auwgent intents
        orchestrator.register_intent("tool_call");
        orchestrator.register_intent("workflow_call");
        orchestrator.register_intent("response_schema");
        orchestrator.register_intent("response_text");
        orchestrator.register_intent("helper_call");

        // Register custom intents from IR
        if let Some(custom) = &ir.custom_intents {
            for ci in custom {
                orchestrator.register_intent(&ci.name);
            }
        }

        let pending_intents = Arc::new(Mutex::new(Vec::new()));
        let intents_for_handler = Arc::clone(&pending_intents);

        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            if let Ok(mut pending) = intents_for_handler.lock() {
                pending.push((name, value));
            }
        }));

        Self {
            ir,
            session: SessionState::new(),
            tools: HashMap::new(),
            orchestrator,
            drivers: HashMap::new(),
            context: None,
            pending_intents,
            pending_tool_results: Vec::new(),
            current_raw_response: String::new(),
            intent_handler: None,
            partial_intent_handler: None,
            session_preload_handler: None,
            session_save_handler: None,
            llm_start_handler: None,
            llm_end_handler: None,
            fast_forward_stack: None,
        }
    }

    pub fn on_sub_engine_start(&mut self, handler: AsyncSessionPreloadCallback) {
        self.session_preload_handler = Some(handler);
    }

    pub fn on_sub_engine_complete(&mut self, handler: SessionSaveCallback) {
        self.session_save_handler = Some(handler);
    }

    pub fn on_llm_start(&mut self, handler: AsyncLlmStartCallback) {
        self.llm_start_handler = Some(handler);
    }

    pub fn on_llm_end(&mut self, handler: AsyncLlmEndCallback) {
        self.llm_end_handler = Some(handler);
    }

    pub fn register_driver(&mut self, provider_type: &str, driver: Arc<dyn ModelDriver>) {
        self.drivers.insert(provider_type.to_string(), driver);
    }

    pub fn set_context(&mut self, context: Value) {
        self.context = Some(context);
    }

    pub fn register_tool(&mut self, name: &str, implementation: ToolImplementation) {
        self.tools.insert(name.to_string(), implementation);
    }

    /// Register an async intent callback.
    ///
    /// This fires for every detected intent (tool_call, response_text, etc.)
    /// during the agentic loop. The engine auto-executes by default.
    ///
    /// To control behavior, return `Some(IntentControl::Skip)` to skip
    /// a tool call, or `Some(IntentControl::Override { result })` to
    /// provide a custom result without executing the tool.
    pub fn on_intent(&mut self, handler: AsyncIntentCallback) {
        self.intent_handler = Some(handler);
    }

    /// Register a sync intent callback (convenience wrapper).
    pub fn on_intent_sync(&mut self, handler: IntentCallback) {
        let handler = handler.clone();
        self.intent_handler = Some(Arc::new(move |name, value| {
            let result = handler(name, value);
            Box::pin(async move { result })
        }));
    }

    /// Register a partial intent callback.
    ///
    /// This fires as YAML data streams in, BEFORE the intent block is
    /// complete. Useful for:
    /// - Streaming partial `response_text` to the UI as tokens arrive
    /// - Showing tool call args as they're being typed by the LLM
    /// - Progress indicators for long structured outputs
    ///
    /// Partial intents are observational only (no control/skip/override).
    pub fn on_intent_partial(&mut self, handler: Arc<dyn Fn(String, Value) + Send + Sync>) {
        // Wire into the orchestrator's partial handler
        let user_handler = handler.clone();
        self.orchestrator
            .on_intent_partial(Arc::new(move |name, value| {
                user_handler(name, value);
            }));
        self.partial_intent_handler = Some(handler);
    }

    pub fn clear_intent_handlers(&mut self) {
        self.intent_handler = None;
        self.partial_intent_handler = None;
        self.orchestrator.on_intent_partial(Arc::new(|_, _| {})); // Clear orchestrator partials
    }

    pub fn clear_sub_engine_handlers(&mut self) {
        self.session_preload_handler = None;
        self.session_save_handler = None;
    }

    pub fn clear_llm_handlers(&mut self) {
        self.llm_start_handler = None;
        self.llm_end_handler = None;
    }

    // ── Session export/import for host runtime hooks ──────────────────────

    /// Export the session state as JSON for the host to persist.
    pub fn export_session(&self) -> AuwgentResult<String> {
        self.session.export().map_err(AuwgentError::Serialization)
    }

    /// Import a session state from JSON, restoring conversation history.
    pub fn import_session(&mut self, json: &str) -> AuwgentResult<()> {
        self.session = SessionState::import(json).map_err(AuwgentError::Serialization)?;
        Ok(())
    }

    /// Clear the session (start a fresh conversation).
    pub fn clear_session(&mut self) {
        self.session.clear();
    }

    /// Get a reference to the session state.
    pub fn session(&self) -> &SessionState {
        &self.session
    }

    // ── Agentic Loop ──────────────────────────────────────────────────────

    /// Execute the agentic loop.
    ///
    /// `initial_stack`: Optional stack for Stack-Aware Resumption. When provided,
    /// the engine fast-forwards through intermediate agents without calling the LLM,
    /// resuming directly at the deepest (last) agent in the stack.
    pub async fn run(
        &mut self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<()> {
        // ── Stack-Aware Resumption ─────────────────────────────────────────
        // Store the initial stack. The first agent in the stack is always the
        // root agent (self), so we skip it and keep the rest for child agents.
        if let Some(stack) = initial_stack {
            // Skip the root agent (index 0) — it IS us, so we just keep the tail
            if stack.len() > 1 {
                self.fast_forward_stack = Some(stack[1..].to_vec());
            } else {
                // Stack only had the root — no fast-forward needed, we ARE the focus
                self.fast_forward_stack = None;
            }
        }

        let evaluator = Evaluator::new(&self.ir);

        // 1. Evaluate Model Info
        let model_entry = self
            .ir
            .model_config
            .first()
            .ok_or(AuwgentError::MissingConfig("No model config".into()))?;
        let default_config = model_entry
            .default_config
            .as_ref()
            .ok_or(AuwgentError::MissingConfig("No default config".into()))?;

        let mut scope = HashMap::new();
        if let Some(val) = input.as_ref() {
            scope.insert("input".to_string(), val.clone());
        }
        if let Some(ctx) = self.context.as_ref() {
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

        // 2. Generate system prompt and set it on the session
        let system_prompt = self.generate_prompt()?;
        self.session.set_system_prompt(&system_prompt);

        // 3. Build the initial user input
        let initial_user_input = match input {
            Some(Value::String(s)) => Some(s),
            Some(v) => Some(serde_json::to_string(&v).map_err(AuwgentError::Serialization)?),
            None => None,
        };

        // Start the first turn if an explicit input is provided.
        // If not (e.g. None), we assume the host wrapper (like TS) already pushed the turn
        // onto the session history, or we are continuing from where we left off.
        if let Some(user_input) = initial_user_input {
            self.session.start_turn(&user_input);
        } else if self.session.turns.is_empty() {
            // Safety fallback: if no input and session is empty, start one
            self.session.start_turn("");
        }

        // Read max loops from lifecycle config, fallback to 12
        let max_loops: usize = self
            .ir
            .lifecycle
            .as_ref()
            .and_then(|lc| lc.get("maxMessages").and_then(|v| v.as_u64()))
            .map(|v| v as usize)
            .unwrap_or(12);

        let mut loop_count = 0;

        loop {
            loop_count += 1;
            if loop_count > max_loops {
                return Err(AuwgentError::MaxLoopsExceeded(max_loops));
            }

            self.current_raw_response.clear();
            self.pending_tool_results.clear();
            self.orchestrator.reset();

            // ── Stack-Aware Resumption: TELEPORTATION ─────────────────────
            // If we have a fast-forward stack, skip the LLM entirely and
            // inject a synthetic helper_call intent to jump straight to
            // the target agent. This is the core of Execution-Tunneling.
            if let Some(ref ffs) = self.fast_forward_stack {
                if let Some(next_helper) = ffs.first() {
                    // Inject synthetic helper_call — no LLM needed
                    let synthetic_intent = serde_json::json!({
                        "type": next_helper,
                        "args": {}
                    });

                    if let Ok(mut pending) = self.pending_intents.lock() {
                        pending.push(("helper_call".to_string(), synthetic_intent));
                    }

                    // Process the synthetic intent (this will call execute_helper,
                    // which will propagate the remaining stack slice)
                    let (_terminal, actions, hard_stop) = self.process_intents().await?;

                    // After teleportation, the sub-engine has completed.
                    // Store results and decide whether to loop or stop.
                    if hard_stop {
                        break;
                    }
                    if actions {
                        let results_payload = self.build_results_payload();
                        self.session.start_turn(&results_payload);
                    }
                    // Continue the loop — the next iteration will run
                    // normally since fast_forward_stack was consumed
                    continue;
                }
            }

            // If this is the first turn (the actual user prompt, not a tool feedback cycle),
            // fire the onLlmStart interceptor. It can return a modified string.
            if loop_count == 1 {
                if let Some(handler) = &self.llm_start_handler {
                    let sys_prompt = self.session.system_prompt.clone().unwrap_or_default();
                    if let Some(last_turn) = self.session.current_turn_mut() {
                        if let Some(modified_prompt) =
                            handler(last_turn.input.clone(), sys_prompt).await
                        {
                            last_turn.input = modified_prompt;
                        }
                    }
                }
            }

            // Build message history from session state (AFTER potential interception)
            let messages = self.session.to_messages();

            // Stream from the driver using full message history
            let mut stream = {
                let driver = self
                    .drivers
                    .get(provider_id)
                    .ok_or_else(|| AuwgentError::NoDriver)?;
                driver
                    .stream_generate(model_name, &messages, config_params.clone())
                    .await
                    .map_err(AuwgentError::Driver)?
            };

            let mut actions_performed = false;

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(text) => {
                        if !text.is_empty() {
                            self.current_raw_response.push_str(&text);
                        }
                        self.orchestrator.write(&text);
                        let (_terminal, actions, hard_stop) = self.process_intents().await?;
                        if actions {
                            actions_performed = true;
                        }
                        if hard_stop {
                            break;
                        }
                    }
                    Err(e) => {
                        // Fire error as intent if handler exists
                        self.fire_intent("error".to_string(), serde_json::json!({ "message": e }))
                            .await;
                        return Err(AuwgentError::StreamError(e));
                    }
                }
            }

            // Log the raw model output for debugging
            // eprintln!(
            //     "[RAW MODEL OUTPUT turn {}]\n{}\n[/RAW]",
            //     loop_count, self.current_raw_response
            // );

            // Finalize parsing and get the full parsed JSON from the intent parser
            // This forces the orchestrator to flush the final pending YAML object
            // (e.g. the final `response_text`) into the engine's pending_intents list.
            let _final_val = self.orchestrator.end();

            let (_terminal, actions, mut hard_stop) = self.process_intents().await?;
            if actions {
                actions_performed = true;
            }

            // Fallback: If no intents were detected in the whole turn, try a deep extraction.
            // This handles cases where streaming parsing got confused by noise but
            // extract_yaml can find a clean signal at the end.
            if !actions_performed {
                let cleaned =
                    crate::intent_parser::orchestrator::extract_yaml(&self.current_raw_response);
                if !cleaned.is_empty() && cleaned != self.current_raw_response {
                    self.orchestrator.reset();
                    self.orchestrator.write(&cleaned);
                    self.orchestrator.end();
                    let (_t2, a2, h2) = self.process_intents().await?;
                    if a2 {
                        actions_performed = true;
                        hard_stop = h2;
                    }
                }
            }

            if let Some(handler) = &self.llm_end_handler {
                let sys_prompt = self.session.system_prompt.clone().unwrap_or_default();
                handler(self.current_raw_response.clone(), sys_prompt).await;
            }

            // If the model output was wrapped in fences, or contained noise,
            // the orchestrator might have parsed it, but 'current_raw_response'
            // still contains the noisy version.
            // We should ideally store the CLEANED version in the session
            // if we want to avoid showing fences in the final stored state.
            let cleaned_response =
                crate::intent_parser::orchestrator::extract_yaml(&self.current_raw_response);

            // Save the raw LLM output in the session history so the exact
            // YAML text is visible in logs and follow-up turns.
            self.session.set_model_response(&cleaned_response);

            // Decide if we loop or stop
            if hard_stop {
                break;
            }

            // If the model performed actions, we MUST loop to feed the results back.
            // We only stop if there are no pending tool/helper results to provide.
            if !actions_performed {
                break;
            }

            // Feed tool/workflow results back to the LLM as the next turn's input
            let results_payload = self.build_results_payload();
            self.session.start_turn(&results_payload);
        }

        Ok(())
    }

    /// Build a structured payload of tool/workflow results to feed back to
    /// the LLM on the next turn. This is critical — without it, the LLM
    /// has no idea what the tools returned.
    fn build_results_payload(&self) -> String {
        if self.pending_tool_results.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        for (name, result) in &self.pending_tool_results {
            let indented_result = match serde_yaml::to_string(result) {
                Ok(yaml_str) => {
                    let trimmed = yaml_str.trim();
                    let content = trimmed.strip_prefix("---\n").unwrap_or(trimmed);
                    let indented: String = content
                        .lines()
                        .map(|line| format!("    {}", line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("  result:\n{}", indented)
                }
                Err(_) => format!("  result: {:?}", result),
            };

            parts.push(format!(
                "tool_result:\n  name: {}\n{}",
                name, indented_result
            ));
        }
        parts.join("\n\n")
    }

    /// Fire the user's intent callback (if registered).
    /// Returns the control signal, or None if no handler / handler returns None.
    async fn fire_intent(&self, name: String, value: Value) -> Option<IntentControl> {
        if let Some(ref handler) = self.intent_handler {
            handler(name, value).await
        } else {
            None
        }
    }

    pub async fn process_intents(&mut self) -> AuwgentResult<(bool, bool, bool)> {
        let intents = {
            let mut pending = self
                .pending_intents
                .lock()
                .expect("pending_intents mutex poisoned");
            std::mem::take(&mut *pending)
        };

        let mut has_terminal = false;
        let mut has_actions = false;
        let mut hard_stop = false;

        let mut tool_results: Vec<(String, Value)> = Vec::new();

        for (name, mut value) in intents {
            // Fire the user callback BEFORE execution
            // Note: _raw is intentionally kept in `value` here so that the host
            // (TypeScript wrapper) can extract it for middleware logging/audit.
            // The TS wrapper removes _raw from the value after extracting it.
            let control = self.fire_intent(name.clone(), value.clone()).await;

            // Strip _raw before internal processing (tool execution, etc.)
            if let Value::Object(ref mut map) = value {
                map.remove("_raw");
            }

            match name.as_str() {
                "tool_call" => {
                    match control {
                        Some(IntentControl::Skip) => {
                            // User chose to skip — fire a skip notification
                            self.fire_intent("tool_skipped".to_string(), value.clone())
                                .await;
                            continue;
                        }
                        Some(IntentControl::Override { result }) => {
                            // User provided a custom result
                            let tool_name = value["type"].as_str().unwrap_or("").to_string();
                            self.fire_intent(
                                "tool_result".to_string(),
                                serde_json::json!({
                                    "name": tool_name,
                                    "result": result,
                                    "overridden": true,
                                }),
                            )
                            .await;
                            tool_results.push((tool_name, result));
                            has_actions = true;
                        }
                        None => {
                            // Default: auto-execute the tool
                            let (tool_name, result) = self.execute_tool(&value).await?;
                            // Fire tool_result intent
                            self.fire_intent(
                                "tool_result".to_string(),
                                serde_json::json!({
                                    "name": tool_name,
                                    "result": result,
                                }),
                            )
                            .await;
                            tool_results.push((tool_name, result));
                            has_actions = true;
                        }
                    }
                }
                "workflow_call" => match control {
                    Some(IntentControl::Skip) => continue,
                    Some(IntentControl::Override { result }) => {
                        let wf_name = value["type"].as_str().unwrap_or("").to_string();
                        tool_results.push((format!("workflow:{}", wf_name), result));
                        has_actions = true;
                    }
                    None => {
                        let (wf_name, result) = self.execute_workflow(&value).await?;
                        self.fire_intent(
                            "workflow_result".to_string(),
                            serde_json::json!({
                                "name": wf_name,
                                "result": result,
                            }),
                        )
                        .await;
                        tool_results.push((format!("workflow:{}", wf_name), result));
                        has_actions = true;
                    }
                },
                "helper_call" => match control {
                    Some(IntentControl::Skip) => continue,
                    Some(IntentControl::Override { result }) => {
                        let helper_name = value["type"].as_str().unwrap_or("").to_string();
                        tool_results.push((format!("helper:{}", helper_name), result));
                        has_actions = true;
                    }
                    None => {
                        let (helper_name, result) = self.execute_helper(&value).await?;

                        // Check if the helper signaled a hard stop (handoff mod="user")
                        if let Some(obj) = result.as_object() {
                            if obj
                                .get("__handoff_stop")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                // The helper already streamed its terminal intent directly to the user.
                                // We tell the parent engine to stop looping immediately.
                                has_terminal = true;
                                hard_stop = true;
                                break;
                            }
                        }

                        self.fire_intent(
                            "helper_result".to_string(),
                            serde_json::json!({
                                "name": helper_name,
                                "result": result,
                            }),
                        )
                        .await;
                        tool_results.push((format!("helper:{}", helper_name), result));
                        has_actions = true;
                    }
                },
                "response_schema" | "response_text" => {
                    // Terminal intents — already fired to user above
                    has_terminal = true;
                }
                _ => {
                    // Custom / unknown intents — already fired to user
                }
            }
        }

        // Store tool results so build_results_payload() can feed them back
        self.pending_tool_results.extend(tool_results);

        Ok((has_terminal, has_actions, hard_stop))
    }

    async fn execute_tool(&self, call: &Value) -> AuwgentResult<(String, Value)> {
        let tool_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        if let Some(imp) = self.tools.get(&tool_name) {
            match imp(args).await {
                Ok(val) => Ok((tool_name, val)),
                Err(e) => {
                    // Fire a specific tool_error intent so the host can react
                    self.fire_intent(
                        "tool_error".to_string(),
                        serde_json::json!({
                            "tool": tool_name,
                            "message": e,
                        }),
                    )
                    .await;
                    // Return the error as the result — the LLM will see it
                    // and can retry or adjust
                    Ok((tool_name, serde_json::json!({ "error": e })))
                }
            }
        } else {
            self.fire_intent(
                "tool_error".to_string(),
                serde_json::json!({
                    "tool": tool_name,
                    "message": format!("Tool not found: {}", tool_name),
                }),
            )
            .await;
            Ok((
                tool_name.clone(),
                serde_json::json!({ "error": format!("Tool '{}' is not registered", tool_name) }),
            ))
        }
    }

    async fn execute_workflow(&mut self, call: &Value) -> AuwgentResult<(String, Value)> {
        let wf_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let body_clone = {
            let wf = match self.ir.workflows.iter().find(|w| w.name == wf_name) {
                Some(w) => w,
                None => {
                    return Ok((
                        wf_name.clone(),
                        serde_json::json!({ "error": format!("Workflow not found: {}", wf_name) }),
                    ));
                }
            };
            wf.body.clone()
        };

        // Create evaluator WITH tools so workflow body can call functions (#4)
        let mut tool_fns: HashMap<String, crate::evaluator::SyncToolFn> = HashMap::new();
        for (name, imp) in &self.tools {
            let imp = imp.clone();
            let name_clone = name.clone();
            tool_fns.insert(
                name.clone(),
                std::sync::Arc::new(move |fn_args: Vec<Value>| {
                    // Convert Vec<Value> to a single JSON object for the tool
                    let arg_val = if fn_args.len() == 1 {
                        fn_args.into_iter().next().unwrap_or(Value::Null)
                    } else {
                        Value::Array(fn_args)
                    };
                    // Block on the async tool — workflows are evaluated synchronously
                    // This is a known limitation; async workflows would need a redesign
                    let rt = tokio::runtime::Handle::current();
                    let imp = imp.clone();
                    std::thread::spawn(move || rt.block_on(imp(arg_val)))
                        .join()
                        .map_err(|_| format!("Tool '{}' panicked", name_clone))?
                }),
            );
        }

        // Clone IR so Evaluator doesn't hold an immutable borrow on self,
        // which allows us to mutably borrow self inside the loop.
        let ir_clone = self.ir.clone();

        let mut scope = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                scope.insert(k.clone(), v.clone());
            }
        }
        // Inject context into workflow scope
        if let Some(ctx) = self.context.as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let mut last_result = Value::Null;
        for stmt in &body_clone {
            let eval_result = {
                // Reconstruct evaluator for each statement using the cloned IR
                // to completely avoid lifetime/borrow issues across await point
                let evaluator = Evaluator::with_tools(&ir_clone, tool_fns.clone());
                evaluator.evaluate(stmt, &mut scope)?
            };

            // Since workflows evaluate synchronously, but helper transfers require spinning up
            // an async Engine, the evaluator returns a special control payload. We intercept it
            // here and perform the async execution.
            if let Some(obj) = eval_result.as_object() {
                // Check for programmatic helper call
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

                    // Build a helper_call value and delegate to execute_helper
                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": helper_args
                    });
                    let (_, sub_result) = self.execute_helper(&helper_call).await?;
                    last_result = sub_result;
                    continue;
                }

                // Check for programmatic transfer
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
                    let (_, sub_result) = self.execute_helper(&helper_call).await?;

                    // If the transfer returned a hard stop signal (handoff user), bubble up
                    if let Some(res_obj) = sub_result.as_object() {
                        if res_obj
                            .get("__handoff_stop")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            return Ok((wf_name, sub_result));
                        }
                    }

                    last_result = sub_result;
                    continue;
                }
            }

            last_result = eval_result;
        }

        Ok((wf_name, last_result))
    }

    /// Execute a helper as a fully-featured nested engine.
    ///
    /// The helper gets:
    ///  - Its own AgentIR (correct prompt, tools, workflows, output schema)
    ///  - All authorized parent tools (filtered via `helperToolGrants`)
    ///  - All parent drivers (shared via Arc — no recreation)
    ///  - Full agentic loop via `sub_engine.run()` — multi-turn, tool calls,
    ///    skip/override controls, workflows — everything the main agent can do.
    ///
    /// The only thing helpers cannot do is invoke other helpers.
    ///
    /// Returns a `BoxFuture` (heap-allocated future) to break the async recursion
    /// cycle: `run → process_intents → execute_workflow → execute_helper → sub_engine.run()`
    fn execute_helper<'a>(
        &'a mut self,
        call: &'a Value,
    ) -> futures_util::future::BoxFuture<'a, AuwgentResult<(String, Value)>> {
        let session_preload_handler = self.session_preload_handler.clone();
        let session_save_handler = self.session_save_handler.clone();

        Box::pin(async move {
            use crate::runtime::helper_runner::{HandoffMode, build_sub_agent_context};

            let helper_name = call["type"].as_str().unwrap_or("").to_string();
            let args = call["args"].clone();

            // 1. Build the sub-agent IR + determine which parent tools are authorized
            let sub_ctx = build_sub_agent_context(&self.ir, &helper_name)?;

            // 2. Construct a fresh sub-engine with the helper's IR
            let mut sub_engine = AuwgentEngine::new(sub_ctx.ir);

            // 3. Share all parent drivers (Arc clone, zero-cost)
            for (provider_type, driver) in &self.drivers {
                sub_engine
                    .drivers
                    .insert(provider_type.clone(), Arc::clone(driver));
            }

            // 4. Inject authorized parent tools into the sub-engine
            for tool_name in &sub_ctx.authorized_parent_tool_names {
                if let Some(imp) = self.tools.get(tool_name) {
                    sub_engine.tools.insert(tool_name.clone(), Arc::clone(imp));
                }
            }

            // 5. Propagate context
            if let Some(ctx) = &self.context {
                sub_engine.set_context(ctx.clone());
            }

            // 6. Wire intent handlers based on handoff mode
            match sub_ctx.handoff_mode {
                HandoffMode::User | HandoffMode::ThenContinue => {
                    // The helper speaks directly to the user —
                    // give it the parent's exact intent + partial handlers
                    if let Some(handler) = &self.intent_handler {
                        sub_engine.on_intent(Arc::clone(handler));
                    }
                    if let Some(handler) = &self.partial_intent_handler {
                        let h = Arc::clone(handler);
                        sub_engine.on_intent_partial(Arc::new(move |name, value| {
                            h(name, value);
                        }));
                    }
                }
                HandoffMode::Return => {
                    // The helper is an internal reasoning step — silent to the user.
                    // We attach no handlers; sub_engine runs silently.
                }
            }

            // 6.1 ALWAYS inherit LLM execution hooks so middleware can trace helper generations
            if let Some(handler) = &self.llm_start_handler {
                sub_engine.on_llm_start(Arc::clone(handler));
            }
            if let Some(handler) = &self.llm_end_handler {
                sub_engine.on_llm_end(Arc::clone(handler));
            }

            // 6.2 Pre-generate the helper's system prompt so it is available
            // to the TypeScript middleware during the "onSubEngineStart" hook.
            if let Ok(system_prompt) = sub_engine.generate_prompt() {
                sub_engine.session.set_system_prompt(&system_prompt);
            }

            // 6.5 Preload session from host if handler exists
            if let Some(preload_fn) = &session_preload_handler {
                let empty_session = sub_engine
                    .export_session()
                    .unwrap_or_else(|_| "{}".to_string());
                if let Some(loaded_json) = preload_fn(helper_name.clone(), empty_session).await {
                    if let Err(e) = sub_engine.import_session(&loaded_json) {
                        eprintln!("Failed to import loaded helper session: {}", e);
                    }
                }
            }

            // 7. Run the full nested agentic loop
            // ── Stack-Aware Resumption: propagate the fast-forward path ─────
            // If the parent engine has a fast-forward stack and the next target
            // is this helper, give the sub-engine the remaining slice.
            let sub_initial_stack = self.fast_forward_stack.as_ref().and_then(|ffs| {
                if ffs.first().map(|s| s.as_str()) == Some(helper_name.as_str()) {
                    // This helper IS next in the stack — give it the remaining slice
                    // (which it will use to continue fast-forwarding into its children).
                    Some({
                        // Include the helper's own name as the "root" of its sub-stack
                        let mut sub_stack = vec![helper_name.clone()];
                        sub_stack.extend_from_slice(&ffs[1..]);
                        sub_stack
                    })
                } else {
                    None
                }
            });

            // Consume the fast-forward stack entry now that we've dispatched it
            if sub_initial_stack.is_some() {
                self.fast_forward_stack = None;
            }

            sub_engine.run(Some(args), sub_initial_stack).await?;

            // 7.5 Save session to host if handler exists
            if let Some(save_fn) = &session_save_handler {
                if let Ok(completed_json) = sub_engine.export_session() {
                    save_fn(helper_name.clone(), completed_json).await;
                }
            }

            // 8. Extract the result & route based on handoff mode
            let final_resp = sub_engine
                .session
                .turns
                .last()
                .map(|t| t.model_response.clone())
                .unwrap_or_default();

            match sub_ctx.handoff_mode {
                HandoffMode::User => {
                    // Helper spoke to user, parent engine stops this turn
                    Ok((helper_name, serde_json::json!({ "__handoff_stop": true })))
                }
                HandoffMode::ThenContinue => {
                    // Helper spoke to user, parent engine continues
                    let msg = format!(
                        "Helper {} delivered a response to the user. Continue your task.",
                        &helper_name
                    );
                    Ok((helper_name, serde_json::json!({ "status": msg })))
                }
                HandoffMode::Return => {
                    // Return the helper's final output to the parent as a tool result
                    Ok((helper_name, serde_json::json!({ "result": final_resp })))
                }
            }
        }) // end Box::pin
    }

    pub fn write_llm_chunk(&mut self, chunk: &str) {
        self.orchestrator.write(chunk);
    }

    pub fn end_llm_stream(&mut self) -> Value {
        self.orchestrator.end()
    }

    pub fn generate_prompt(&self) -> AuwgentResult<String> {
        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();

        // Inject context into scope so prompt templates can use {{context.field}} (#7)
        if let Some(ctx) = self.context.as_ref() {
            scope.insert("context".to_string(), ctx.clone());
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

        let prompt_val = evaluator.evaluate(&default.prompt, &mut scope)?;
        let mut prompt = prompt_val.as_str().unwrap_or("").to_string();

        let intents = crate::intents::generate_intents(&self.ir);
        if !intents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&intents);
        }

        Ok(prompt)
    }
}
