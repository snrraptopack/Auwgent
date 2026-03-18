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
    session: Arc<Mutex<SessionState>>,
    tools: Arc<Mutex<HashMap<String, ToolImplementation>>>,
    orchestrator: Arc<Mutex<Orchestrator>>,
    drivers: Arc<Mutex<HashMap<String, Arc<dyn ModelDriver>>>>,
    context: Arc<Mutex<Option<Value>>>,
    /// Pending intents collected by the orchestrator callback
    pending_intents: Arc<Mutex<Vec<(String, Value)>>>,
    /// Tool/workflow results accumulated during the current turn
    pending_tool_results: Arc<Mutex<Vec<(String, Value)>>>,
    /// Accumulated raw response for the current turn
    current_raw_response: Arc<Mutex<String>>,
    /// User-facing intent callback
    intent_handler: Arc<Mutex<Option<AsyncIntentCallback>>>,
    partial_intent_handler: Arc<Mutex<Option<Arc<dyn Fn(String, Value) + Send + Sync>>>>,
    session_preload_handler: Arc<Mutex<Option<AsyncSessionPreloadCallback>>>,
    session_save_handler: Arc<Mutex<Option<SessionSaveCallback>>>,
    llm_start_handler: Arc<Mutex<Option<AsyncLlmStartCallback>>>,
    llm_end_handler: Arc<Mutex<Option<AsyncLlmEndCallback>>>,
    fast_forward_stack: Arc<Mutex<Option<Vec<String>>>>,
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
            session: Arc::new(Mutex::new(SessionState::new())),
            tools: Arc::new(Mutex::new(HashMap::new())),
            orchestrator: Arc::new(Mutex::new(orchestrator)),
            drivers: Arc::new(Mutex::new(HashMap::new())),
            context: Arc::new(Mutex::new(None)),
            pending_intents,
            pending_tool_results: Arc::new(Mutex::new(Vec::new())),
            current_raw_response: Arc::new(Mutex::new(String::new())),
            intent_handler: Arc::new(Mutex::new(None)),
            partial_intent_handler: Arc::new(Mutex::new(None)),
            session_preload_handler: Arc::new(Mutex::new(None)),
            session_save_handler: Arc::new(Mutex::new(None)),
            llm_start_handler: Arc::new(Mutex::new(None)),
            llm_end_handler: Arc::new(Mutex::new(None)),
            fast_forward_stack: Arc::new(Mutex::new(None)),
        }
    }

    pub fn on_sub_engine_start(&self, handler: AsyncSessionPreloadCallback) {
        *self.session_preload_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_sub_engine_complete(&self, handler: SessionSaveCallback) {
        *self.session_save_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_llm_start(&self, handler: AsyncLlmStartCallback) {
        *self.llm_start_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_llm_end(&self, handler: AsyncLlmEndCallback) {
        *self.llm_end_handler.lock().unwrap() = Some(handler);
    }

    pub fn register_driver(&self, provider_type: &str, driver: Arc<dyn ModelDriver>) {
        self.drivers.lock().unwrap().insert(provider_type.to_string(), driver);
    }

    pub fn set_context(&self, context: Value) {
        *self.context.lock().unwrap() = Some(context);
    }

    pub fn register_tool(&self, name: &str, implementation: ToolImplementation) {
        self.tools.lock().unwrap().insert(name.to_string(), implementation);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        *self.intent_handler.lock().unwrap() = Some(handler);
    }

    /// Register a sync intent callback (convenience wrapper).
    pub fn on_intent_sync(&self, handler: IntentCallback) {
        let handler = handler.clone();
        *self.intent_handler.lock().unwrap() = Some(Arc::new(move |name, value| {
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
    pub fn on_intent_partial(&self, handler: Arc<dyn Fn(String, Value) + Send + Sync>) {
        // Wire into the orchestrator's partial handler
        let user_handler = handler.clone();
        self.orchestrator
            .lock()
            .unwrap()
            .on_intent_partial(Arc::new(move |name, value| {
                user_handler(name, value);
            }));
        *self.partial_intent_handler.lock().unwrap() = Some(handler);
    }

    pub fn clear_intent_handlers(&self) {
        *self.intent_handler.lock().unwrap() = None;
        *self.partial_intent_handler.lock().unwrap() = None;
    }

    pub fn clear_sub_engine_handlers(&self) {
        *self.session_preload_handler.lock().unwrap() = None;
        *self.session_save_handler.lock().unwrap() = None;
    }

    pub fn clear_llm_handlers(&self) {
        *self.llm_start_handler.lock().unwrap() = None;
        *self.llm_end_handler.lock().unwrap() = None;
    }

    // ── Session export/import for host runtime hooks ──────────────────────

    /// Export the session state as JSON for the host to persist.
    pub fn export_session(&self) -> AuwgentResult<String> {
        self.session.lock().unwrap().export().map_err(AuwgentError::Serialization)
    }

    /// Import a session state from JSON, restoring conversation history.
    pub fn import_session(&self, json: &str) -> AuwgentResult<()> {
        *self.session.lock().unwrap() = SessionState::import(json).map_err(AuwgentError::Serialization)?;
        Ok(())
    }

    /// Access the session state directly (for testing/CLI).
    pub fn session(&self) -> std::sync::MutexGuard<'_, SessionState> {
        self.session.lock().unwrap()
    }

    /// Clear the session (start a fresh conversation).
    pub fn clear_session(&self) {
        self.session.lock().unwrap().clear();
    }

    /// Get a reference to the session state. (Removed: Cannot return ref to Mutex guard easily)
    // pub fn session(&self) -> &SessionState { ... }

    // ── Embedding ─────────────────────────────────────────────────────────
    pub async fn embed(&self, text: &str) -> AuwgentResult<Vec<f32>> {
        let (driver, model_name, config) = self.get_embedding_config()?;
        driver
            .embed(&model_name, text, config)
            .await
            .map_err(AuwgentError::Driver)
    }

    pub async fn embed_batch(&self, texts: &[String]) -> AuwgentResult<Vec<Vec<f32>>> {
        let (driver, model_name, config) = self.get_embedding_config()?;
        driver
            .embed_batch(&model_name, texts, config)
            .await
            .map_err(AuwgentError::Driver)
    }

    fn get_embedding_config(&self) -> AuwgentResult<(Arc<dyn ModelDriver>, String, Option<Value>)> {
        let model_entry = self
            .ir
            .model_config
            .first()
            .ok_or(AuwgentError::MissingConfig("No model config".into()))?;
        let default_config = model_entry
            .default_config
            .as_ref()
            .ok_or(AuwgentError::MissingConfig("No default config".into()))?;

        let embedding_provider = default_config
            .embedding
            .as_ref()
            .ok_or(AuwgentError::MissingConfig(
                "No embedding model configured".into(),
            ))?;

        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();
        // Use a block to minimize lock duration
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }
        
        let provider_info = evaluator.evaluate_provider(embedding_provider, &mut scope)?;

        let provider_type = provider_info["provider"].as_str().unwrap_or("gemini");
        let provider_id = if provider_type == "custom" {
            provider_info["id"].as_str().unwrap_or("custom")
        } else {
            provider_type
        };

        let model_name = provider_info["modelName"]
            .as_str()
            .ok_or_else(|| AuwgentError::MissingConfig("modelName is required for embedding".into()))?;

        let config_params = provider_info.get("config").cloned();

        let driver = self
            .drivers
            .lock()
            .unwrap()
            .get(provider_id)
            .ok_or_else(|| AuwgentError::NoDriver)?
            .clone();

        Ok((driver, model_name.to_string(), config_params))
    }

    // ── Agentic Loop ──────────────────────────────────────────────────────

    /// Execute the agentic loop.
    ///
    /// `initial_stack`: Optional stack for Stack-Aware Resumption. When provided,
    /// the engine fast-forwards through intermediate agents without calling the LLM,
    /// resuming directly at the deepest (last) agent in the stack.
    pub async fn run(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<()> {
        // ── Stack-Aware Resumption ─────────────────────────────────────────
        // Store the initial stack. The first agent in the stack is always the
        // root agent (self), so we skip it and keep the rest for child agents.
        if let Some(stack) = initial_stack {
            // Skip the root agent (index 0) — it IS us, so we just keep the tail
            if stack.len() > 1 {
                *self.fast_forward_stack.lock().unwrap() = Some(stack[1..].to_vec());
            } else {
                // Stack only had the root — no fast-forward needed, we ARE the focus
                *self.fast_forward_stack.lock().unwrap() = None;
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

        // 2. Generate system prompt and set it on the session
        let system_prompt = self.generate_prompt()?;
        self.session.lock().unwrap().set_system_prompt(&system_prompt);

        // 3. Build the initial user input
        let initial_user_input = match input {
            Some(Value::String(s)) => Some(s),
            Some(v) => Some(serde_json::to_string(&v).map_err(AuwgentError::Serialization)?),
            None => None,
        };

        // Start the first turn if an explicit input is provided.
        if let Some(user_input) = initial_user_input {
            self.session.lock().unwrap().start_turn(&user_input);
        } else if self.session.lock().unwrap().turns.is_empty() {
            // Safety fallback: if no input and session is empty, start one
            self.session.lock().unwrap().start_turn("");
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

            self.current_raw_response.lock().unwrap().clear();
            self.pending_tool_results.lock().unwrap().clear();
            self.orchestrator.lock().unwrap().reset();

            // ── Stack-Aware Resumption: TELEPORTATION ─────────────────────
            // If we have a fast-forward stack, skip the LLM entirely and
            // inject a synthetic helper_call intent to jump straight to
            // the target agent. This is the core of Execution-Tunneling.
            let next_helper = {
                let ffs_lock = self.fast_forward_stack.lock().unwrap();
                ffs_lock.as_ref().and_then(|ffs| ffs.first().cloned())
            };

            if let Some(next_helper) = next_helper {
                // Inject synthetic helper_call — no LLM needed
                let synthetic_intent = serde_json::json!({
                    "type": next_helper,
                    "args": {}
                });

                self.pending_intents.lock().unwrap().push(("helper_call".to_string(), synthetic_intent));

                // Process the synthetic intent
                let (_terminal, actions, hard_stop) = self.process_intents().await?;

                if hard_stop {
                    break;
                }
                if actions {
                    let results_payload = self.build_results_payload();
                    self.session.lock().unwrap().start_turn(&results_payload);
                }
                continue;
            }

            // If this is the first turn (the actual user prompt, not a tool feedback cycle),
            // fire the onLlmStart interceptor. It can return a modified string.
            if loop_count == 1 {
                let start_handler = self.llm_start_handler.lock().unwrap().clone();
                if let Some(h) = start_handler {
                    let sys_prompt = self.session.lock().unwrap().system_prompt.clone().unwrap_or_default();
                    let input_text = self.session.lock().unwrap().turns.last().map(|t| t.input.clone());
                    if let Some(text) = input_text {
                        if let Some(modified) = h(text, sys_prompt).await {
                            self.session.lock().unwrap().set_input(modified);
                        }
                    }
                }
            }

            // Build message history from session state (AFTER potential interception)
            let messages = self.session.lock().unwrap().to_messages();

            // Stream from the driver using full message history
            let mut stream = {
                let driver = self
                    .drivers
                    .lock()
                    .unwrap()
                    .get(provider_id)
                    .ok_or_else(|| AuwgentError::NoDriver)?
                    .clone();
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
                            self.current_raw_response.lock().unwrap().push_str(&text);
                        }
                        self.orchestrator.lock().unwrap().write(&text);
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

            // Finalize parsing
            let _final_val = self.orchestrator.lock().unwrap().end();

            let (_terminal, actions, mut hard_stop) = self.process_intents().await?;
            if actions {
                actions_performed = true;
            }

            // Fallback: If no intents were detected in the whole turn, try a deep extraction.
            if !actions_performed {
                let (needs_fallback, cleaned) = {
                    let raw = self.current_raw_response.lock().unwrap();
                    let cleaned = crate::intent_parser::orchestrator::extract_yaml(&raw);
                    (!cleaned.is_empty() && cleaned != *raw, cleaned)
                };

                if needs_fallback {
                    {
                        let mut orch = self.orchestrator.lock().unwrap();
                        orch.reset();
                        orch.write(&cleaned);
                        orch.end();
                    }
                    let (_t2, a2, h2) = self.process_intents().await?;
                    if a2 {
                        actions_performed = true;
                        hard_stop = h2;
                    }
                }
            }

            // Fire LLM end hook
            let end_handler = self.llm_end_handler.lock().unwrap().clone();
            if let Some(h) = end_handler {
                let sys_prompt = self.session.lock().unwrap().system_prompt.clone().unwrap_or_default();
                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                h(raw_resp, sys_prompt).await;
            }

            // If the model output was wrapped in fences, or contained noise,
            // the orchestrator might have parsed it, but 'current_raw_response'
            // still contains the noisy version.
            // We should ideally store the CLEANED version in the session
            // if we want to avoid showing fences in the final stored state.
            let cleaned_response = {
                let raw = self.current_raw_response.lock().unwrap();
                crate::intent_parser::orchestrator::extract_yaml(&raw)
            };

            // Save the raw LLM output in the session history so the exact
            // YAML text is visible in logs and follow-up turns.
            self.session.lock().unwrap().set_model_response(&cleaned_response);

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
            self.session.lock().unwrap().start_turn(&results_payload);
        }

        Ok(())
    }

    /// Build a structured payload of tool/workflow results to feed back to
    /// the LLM on the next turn. This is critical — without it, the LLM
    /// has no idea what the tools returned.
    fn build_results_payload(&self) -> String {
        let results = self.pending_tool_results.lock().unwrap();
        if results.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        for (name, result) in &*results {
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
        let handler = self.intent_handler.lock().unwrap().clone();
        if let Some(h) = handler {
            h(name, value).await
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

        // Store tool results
        self.pending_tool_results.lock().unwrap().extend(tool_results);

        Ok((has_terminal, has_actions, hard_stop))
    }

    async fn execute_tool(&self, call: &Value) -> AuwgentResult<(String, Value)> {
        let tool_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let imp = self.tools.lock().unwrap().get(&tool_name).cloned();
        if let Some(imp) = imp {
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

    async fn execute_workflow(&self, call: &Value) -> AuwgentResult<(String, Value)> {
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

        // Create evaluator WITH tools
        let mut tool_fns: HashMap<String, crate::evaluator::SyncToolFn> = HashMap::new();
        {
            let tools = self.tools.lock().unwrap();
            for (name, imp) in &*tools {
                let imp = imp.clone();
                let name_clone = name.clone();
                tool_fns.insert(
                    name.clone(),
                    std::sync::Arc::new(move |fn_args: Vec<Value>| {
                        let arg_val = if fn_args.len() == 1 {
                            fn_args.into_iter().next().unwrap_or(Value::Null)
                        } else {
                            Value::Array(fn_args)
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
        // Inject context into workflow scope
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
                    let (_, sub_result) = self.execute_helper(&helper_call).await?;
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
                    let (_, sub_result) = self.execute_helper(&helper_call).await?;

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
        &'a self,
        call: &'a Value,
    ) -> futures_util::future::BoxFuture<'a, AuwgentResult<(String, Value)>> {
        let session_preload_handler = self.session_preload_handler.clone();
        let session_save_handler = self.session_save_handler.clone();

        Box::pin(async move {
            use crate::runtime::helper_runner::{HandoffMode, build_sub_agent_context};

            let helper_name = call["type"].as_str().unwrap_or("").to_string();
            let args = call["args"].clone();

            // 1. Build the sub-agent IR
            let sub_ctx = build_sub_agent_context(&self.ir, &helper_name)?;

            // 2. Construct a fresh sub-engine
            let sub_engine = AuwgentEngine::new(sub_ctx.ir);

            // 3. Share all parent drivers
            {
                let drivers = self.drivers.lock().unwrap();
                let mut sub_drivers = sub_engine.drivers.lock().unwrap();
                for (provider_type, driver) in &*drivers {
                    sub_drivers.insert(provider_type.clone(), Arc::clone(driver));
                }
            }

            // 4. Inject authorized parent tools
            {
                let tools = self.tools.lock().unwrap();
                let mut sub_tools = sub_engine.tools.lock().unwrap();
                for tool_name in &sub_ctx.authorized_parent_tool_names {
                    if let Some(imp) = tools.get(tool_name) {
                        sub_tools.insert(tool_name.clone(), Arc::clone(imp));
                    }
                }
            }

            // 5. Propagate context
            if let Some(ctx) = self.context.lock().unwrap().as_ref() {
                sub_engine.set_context(ctx.clone());
            }

            // 6. Wire intent handlers
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

            // Inherit hooks (middleware listeners)
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

            // Pre-generate system prompt
            if let Ok(system_prompt) = sub_engine.generate_prompt() {
                sub_engine.session.lock().unwrap().set_system_prompt(&system_prompt);
            }

            // Preload session
            let preload_fn = session_preload_handler.lock().unwrap().clone();
            if let Some(f) = preload_fn {
                let empty_session = sub_engine
                    .export_session()
                    .unwrap_or_else(|_| "{}".to_string());
                if let Some(loaded_json) = f(helper_name.clone(), empty_session).await {
                    let _ = sub_engine.import_session(&loaded_json);
                }
            }

            // Propagate fast-forward stack
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

            let _ = sub_engine.run(Some(args), sub_initial_stack).await;

            // Save session
            let save_fn = session_save_handler.lock().unwrap().clone();
            if let Some(f) = save_fn {
                if let Ok(completed_json) = sub_engine.export_session() {
                    f(helper_name.clone(), completed_json).await;
                }
            }

            let final_resp = sub_engine
                .session
                .lock()
                .unwrap()
                .turns
                .last()
                .map(|t| t.model_response.clone())
                .unwrap_or_default();

            match sub_ctx.handoff_mode {
                HandoffMode::User => {
                    Ok((helper_name, serde_json::json!({ "__handoff_stop": true })))
                }
                HandoffMode::ThenContinue => {
                    let msg = format!("Helper {} delivered response to user. Continue.", &helper_name);
                    Ok((helper_name, serde_json::json!({ "status": msg })))
                }
                HandoffMode::Return => {
                    Ok((helper_name, serde_json::json!({ "result": final_resp })))
                }
            }
        })
    }

    pub fn write_llm_chunk(&self, chunk: &str) {
        self.orchestrator.lock().unwrap().write(chunk);
    }

    pub fn end_llm_stream(&self) -> Value {
        self.orchestrator.lock().unwrap().end()
    }

    pub fn generate_prompt(&self) -> AuwgentResult<String> {
        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();

        // Inject context into scope so prompt templates can use {{context.field}} (#7)
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
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

        // ── Magic Context DX: Automatic context injection ─────────────────
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            if let Some(obj) = ctx.as_object() {
                let mut filtered_ctx = serde_json::Map::new();
                for (k, v) in obj {
                    let is_empty = match v {
                        Value::Null => true,
                        Value::Array(a) => a.is_empty(),
                        Value::Object(o) => o.is_empty(),
                        Value::String(s) => s.is_empty(),
                        _ => false,
                    };
                    if !is_empty {
                        filtered_ctx.insert(k.clone(), v.clone());
                    }
                }

                if !filtered_ctx.is_empty() {
                    if let Ok(yaml) = serde_yaml::to_string(&Value::Object(filtered_ctx)) {
                        prompt.push_str("\n\n# ADDITIONAL CONTEXT\n");
                        prompt.push_str(yaml.trim());
                    }
                }
            }
        }

        let intents = crate::intents::generate_intents(&self.ir);
        if !intents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&intents);
        }

        Ok(prompt)
    }
}
