use crate::errors::{AuwgentError, AuwgentResult};
use crate::evaluator::Evaluator;
use crate::intent_parser::block_orchestrator::BlockOrchestrator as Orchestrator;
use crate::runtime::drivers::{ModelDriver, ModelEvent, TokenUsage, FinishReason};
pub use crate::runtime::engine_types::{
    AsyncErrorCallback, AsyncIntentCallback, AsyncLlmEndCallback, AsyncLlmStartCallback,
    AsyncMiddlewareEventCallback, AsyncRunCompleteCallback, AsyncRunStartCallback,
    AsyncSessionPreloadCallback, IntentCallback, IntentControl, RunMetadata, SessionSaveCallback,
    ToolImplementation, TurnMetadata,
};
use crate::runtime::middleware;
use crate::runtime::session::SessionState;
use crate::types::*;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod execution;

fn empty_response_marker(finish_reason: Option<&FinishReason>) -> String {
    match finish_reason {
        Some(FinishReason::ContentFilter) => {
            "(no response: blocked by model safety/content filter)".to_string()
        }
        Some(FinishReason::Length) => {
            "(no response: generation ended due to max token limit)".to_string()
        }
        Some(FinishReason::ToolCalls) => {
            "(no response: model indicated tool calls but no parsable tool intent was produced)"
                .to_string()
        }
        Some(FinishReason::Stop) => "(no response: model returned an empty completion)".to_string(),
        Some(FinishReason::Other(reason)) => {
            format!("(no response: finish reason = {})", reason)
        }
        None => "(no response: stream completed without content)".to_string(),
    }
}

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
    /// Track intents that were emitted as partials to avoid duplicate complete emissions
    emitted_partial_intents: Arc<Mutex<std::collections::HashSet<(String, String)>>>,
    /// Tool/workflow results accumulated during the current turn
    /// Format: (name, args, result) where args is the input and result is the output
    pending_tool_results: Arc<Mutex<Vec<(String, Value, Value)>>>,
    /// Accumulated raw response for the current turn
    current_raw_response: Arc<Mutex<String>>,
    /// Last terminal response payload produced during the current LLM cycle
    last_turn_response_value: Arc<Mutex<Value>>,
    /// User-facing intent callback
    intent_handler: Arc<Mutex<Option<AsyncIntentCallback>>>,
    partial_intent_handler: Arc<Mutex<Option<Arc<dyn Fn(String, Value, String) + Send + Sync>>>>,
    session_preload_handler: Arc<Mutex<Option<AsyncSessionPreloadCallback>>>,
    session_save_handler: Arc<Mutex<Option<SessionSaveCallback>>>,
    llm_start_handler: Arc<Mutex<Option<AsyncLlmStartCallback>>>,
    llm_end_handler: Arc<Mutex<Option<AsyncLlmEndCallback>>>,
    run_start_handler: Arc<Mutex<Option<AsyncRunStartCallback>>>,
    run_complete_handler: Arc<Mutex<Option<AsyncRunCompleteCallback>>>,
    error_handler: Arc<Mutex<Option<AsyncErrorCallback>>>,
    middleware_event_handler: Arc<Mutex<Option<AsyncMiddlewareEventCallback>>>,
    fast_forward_stack: Arc<Mutex<Option<Vec<String>>>>,
    /// Tracking if a terminal response (text/schema) was emitted during the current run
    terminal_response_emitted: Arc<Mutex<bool>>,
    /// Tracking if a FINAL terminal response (text/schema) was emitted.
    /// Custom intents count as terminal but NOT final (focus stays on the agent).
    final_response_emitted: Arc<Mutex<bool>>,
    /// Original user input for the current run cycle (used for teleportation follow-ups)
    user_input: Arc<Mutex<Option<serde_json::Value>>>,
    /// Accumulated run metadata across turns
    pub last_run_metadata: Arc<Mutex<RunMetadata>>,
}

impl AuwgentEngine {
    pub fn new(ir: AgentIR) -> Self {
        let mut orchestrator = Orchestrator::new();

        // Register standard Auwgent intentsn
        orchestrator.register_intent("tool_call");
        orchestrator.register_intent("workflow_call");
        orchestrator.register_intent("response_schema");
        orchestrator.register_intent("response_text");
        orchestrator.register_intent("helper_call");

        // Register custom intents from IR
        if let Some(custom) = &ir.custom_intents {
            for ci in custom {
                orchestrator.register_intent(&ci.name);
                orchestrator.register_custom_intent_shape(
                    &ci.name,
                    &ci.fields.0,
                    ir.types.as_ref(),
                );
            }
        }

        for tool in &ir.tools {
            orchestrator.register_tool_shape(&tool.name, &tool.params.0, ir.types.as_ref());
        }

        for workflow in &ir.workflows {
            orchestrator.register_workflow_shape(
                &workflow.name,
                &workflow.params.0,
                ir.types.as_ref(),
            );
        }

        for helper in &ir.helpers {
            orchestrator.register_helper_shape(
                &helper.name,
                helper.input.as_ref().map(|v| &v.0),
                ir.types.as_ref(),
            );
        }

        if let Some(output) = &ir.output {
            orchestrator.register_output_shape(&output.0, ir.types.as_ref());
        }

        let pending_intents = Arc::new(Mutex::new(Vec::new()));
        let intents_for_handler = Arc::clone(&pending_intents);
        let emitted_partial_intents = Arc::new(Mutex::new(std::collections::HashSet::new()));

        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            if let Ok(mut pending) = intents_for_handler.lock() {
                // Check if this exact intent (by name and value) is already pending
                // This prevents duplicates when the same intent is emitted during
                // streaming and then again during finalization
                let already_pending = pending.iter().any(|(n, v)| n == &name && v == &value);

                if !already_pending {
                    pending.push((name, value));
                }
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
            emitted_partial_intents,
            pending_tool_results: Arc::new(Mutex::new(Vec::new())),
            current_raw_response: Arc::new(Mutex::new(String::new())),
            last_turn_response_value: Arc::new(Mutex::new(Value::Null)),
            intent_handler: Arc::new(Mutex::new(None)),
            partial_intent_handler: Arc::new(Mutex::new(None)),
            session_preload_handler: Arc::new(Mutex::new(None)),
            session_save_handler: Arc::new(Mutex::new(None)),
            llm_start_handler: Arc::new(Mutex::new(None)),
            llm_end_handler: Arc::new(Mutex::new(None)),
            run_start_handler: Arc::new(Mutex::new(None)),
            run_complete_handler: Arc::new(Mutex::new(None)),
            error_handler: Arc::new(Mutex::new(None)),
            middleware_event_handler: Arc::new(Mutex::new(None)),
            fast_forward_stack: Arc::new(Mutex::new(None)),
            terminal_response_emitted: Arc::new(Mutex::new(false)),
            final_response_emitted: Arc::new(Mutex::new(false)),
            user_input: Arc::new(Mutex::new(None)),
            last_run_metadata: Arc::new(Mutex::new(RunMetadata::default())),
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

    pub fn on_run_start(&self, handler: AsyncRunStartCallback) {
        *self.run_start_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_run_complete(&self, handler: AsyncRunCompleteCallback) {
        *self.run_complete_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_error(&self, handler: AsyncErrorCallback) {
        *self.error_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_middleware_event(&self, handler: AsyncMiddlewareEventCallback) {
        *self.middleware_event_handler.lock().unwrap() = Some(handler);
    }

    pub fn register_driver(&self, provider_type: &str, driver: Arc<dyn ModelDriver>) {
        self.drivers
            .lock()
            .unwrap()
            .insert(provider_type.to_string(), driver);
    }

    pub fn set_context(&self, context: Value) {
        *self.context.lock().unwrap() = Some(context);
    }

    pub fn register_tool(&self, name: &str, implementation: ToolImplementation) {
        self.tools
            .lock()
            .unwrap()
            .insert(name.to_string(), implementation);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        *self.intent_handler.lock().unwrap() = Some(handler);
    }

    /// Register a sync intent callback (convenience wrapper).
    pub fn on_intent_sync(&self, handler: IntentCallback) {
        let handler = handler.clone();
        *self.intent_handler.lock().unwrap() = Some(Arc::new(move |name, value, agent| {
            let result = handler(name, value, agent);
            Box::pin(async move { result })
        }));
    }

    /// Register a partial intent callback.
    ///
    /// This fires as <reponse_text> or [block] data streams in, BEFORE the intent block is
    /// complete. Useful for:
    /// - Streaming partial `response_text` to the UI as tokens arrive
    /// - Showing tool call args as they're being typed by the LLM
    /// - Progress indicators for long structured outputs
    ///
    /// Register a partial intent callback.
    pub fn on_intent_partial(&self, handler: Arc<dyn Fn(String, Value, String) + Send + Sync>) {
        // Wire into the orchestrator's partial handler
        let user_handler = handler.clone();
        let emitted_partials = Arc::clone(&self.emitted_partial_intents);
        let agent_name = self.ir.name.clone();
        let partial_cursor: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

        self.orchestrator
            .lock()
            .unwrap()
            .on_intent_partial(Arc::new(move |name, value| {
                // Track this partial emission to prevent duplicate complete emission
                let value_hash = serde_json::to_string(&value).unwrap_or_default();
                let partial_key = (name.clone(), value_hash);

                if let Ok(mut partials) = emitted_partials.lock() {
                    partials.insert(partial_key);
                }

                let value = if name == "response_text" {
                    if let Some(text) = value
                        .get("snapshot")
                        .and_then(|snapshot| snapshot.get("text"))
                        .and_then(Value::as_str)
                    {
                        let segment = value
                            .get("segment")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        let key = format!("{agent_name}:{name}:{segment}");
                        let previous = partial_cursor
                            .lock()
                            .ok()
                            .and_then(|cursor| cursor.get(&key).cloned())
                            .unwrap_or_default();
                        let delta = if text.starts_with(&previous) {
                            &text[previous.len()..]
                        } else {
                            text
                        };
                        if let Ok(mut cursor) = partial_cursor.lock() {
                            cursor.insert(key, text.to_string());
                        }
                        let mut updated = value.clone();
                        if let Value::Object(ref mut map) = updated {
                            map.insert("delta".to_string(), Value::String(delta.to_string()));
                        }
                        updated
                    } else {
                        value
                    }
                } else {
                    value
                };

                // Call user handler with current agent name
                user_handler(name, value, agent_name.clone());
            }));
        *self.partial_intent_handler.lock().unwrap() = Some(handler);
    }

    pub fn clear_intent_handlers(&self) {
        *self.intent_handler.lock().unwrap() = None;
        *self.partial_intent_handler.lock().unwrap() = None;
        self.orchestrator.lock().unwrap().on_intent_partial(Arc::new(|_, _| {}));
    }

    pub fn clear_sub_engine_handlers(&self) {
        *self.session_preload_handler.lock().unwrap() = None;
        *self.session_save_handler.lock().unwrap() = None;
    }

    pub fn clear_llm_handlers(&self) {
        *self.llm_start_handler.lock().unwrap() = None;
        *self.llm_end_handler.lock().unwrap() = None;
    }

    pub fn clear_run_handlers(&self) {
        *self.run_start_handler.lock().unwrap() = None;
        *self.run_complete_handler.lock().unwrap() = None;
        *self.error_handler.lock().unwrap() = None;
    }

    pub fn clear_middleware_handler(&self) {
        *self.middleware_event_handler.lock().unwrap() = None;
    }

    // ── Session export/import for host runtime hooks ──────────────────────

    /// Export the session state as JSON for the host to persist.
    pub fn export_session(&self) -> AuwgentResult<String> {
        self.session
            .lock()
            .unwrap()
            .export()
            .map_err(AuwgentError::Serialization)
    }

    /// Import a session state from JSON, restoring conversation history.
    pub fn import_session(&self, json: &str) -> AuwgentResult<()> {
        *self.session.lock().unwrap() =
            SessionState::import(json).map_err(AuwgentError::Serialization)?;
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

        let embedding_provider =
            default_config
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

        let model_name = provider_info["modelName"].as_str().ok_or_else(|| {
            AuwgentError::MissingConfig("modelName is required for embedding".into())
        })?;

        let config_params = provider_info.get("config").cloned();

        let driver = self
            .drivers
            .lock()
            .unwrap()
            .get(provider_id)
            .ok_or(AuwgentError::NoDriver)?
            .clone();

        Ok((driver, model_name.to_string(), config_params))
    }

    fn build_event_context(
        &self,
        active_agent: &str,
        raw_block: Option<String>,
        system_prompt: Option<String>,
    ) -> Value {
        let session = self.session.lock().unwrap();
        serde_json::json!({
            "activeAgent": active_agent,
            "stack": session.stack,
            "rootAgent": session.stack.first().cloned().unwrap_or_else(|| self.ir.name.clone()),
            "systemPrompt": system_prompt.or_else(|| session.system_prompt.clone()),
            "rawBlock": raw_block,
        })
    }

    async fn fire_middleware_event(&self, event: Value) -> Option<Value> {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        middleware::fire_middleware_event(handler, event).await
    }

    async fn apply_intent_middleware(
        &self,
        name: &str,
        value: &Value,
        active_agent: &str,
    ) -> Option<IntentControl> {
        let raw_block = value
            .get("_raw")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
        let event = serde_json::json!({
            "type": "intent",
            "name": name,
            "value": value,
            "context": self.build_event_context(active_agent, raw_block, None),
        });
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        middleware::apply_intent_middleware(handler, event).await
    }

    async fn apply_llm_start_middleware(
        &self,
        prompt: &str,
        system_prompt: &str,
        active_agent: &str,
    ) -> Option<Value> {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        middleware::apply_llm_start_middleware(handler, serde_json::json!({
            "type": "llm_start",
            "prompt": prompt,
            "context": self.build_event_context(active_agent, None, Some(system_prompt.to_string())),
        }))
        .await
    }

    async fn notify_llm_end_middleware(
        &self,
        response: &Value,
        system_prompt: &str,
        active_agent: &str,
        turn_metadata: &TurnMetadata,
    ) {
        let handler = self.middleware_event_handler.lock().unwrap().clone();
        let context = self.build_event_context(active_agent, None, Some(system_prompt.to_string()));
        middleware::notify_llm_end_middleware(handler, response, context, turn_metadata).await;
    }

    fn strip_raw_field(&self, mut value: Value) -> Value {
        if let Value::Object(ref mut map) = value {
            map.remove("_raw");
        }
        value
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
        // println!("[DEBUG] AuwgentEngine::run - Agent: {}, Input: {:?}, Initial Stack: {:?}", self.ir.name, input, initial_stack);
        // ── Stack-Aware Resumption ─────────────────────────────────────────
        {
            let mut session = self.session.lock().unwrap();

            // 1. Initialize session stack if empty
            if session.stack.is_empty() {
                session.stack = vec![self.ir.name.clone()];
            }

            // 2. If an explicit stack was passed (e.g. from SDK), sync it to session
            if let Some(stack) = initial_stack {
                session.stack = stack;
            }

            // 3. Set fast-forward focus based on session stack
            // Root agent (index 0) is US, so we skip it for teleportation logic
            if session.stack.len() > 1 {
                // println!("[DEBUG] Teleportation active. Target stack: {:?}", session.stack);
                *self.fast_forward_stack.lock().unwrap() = Some(session.stack[1..].to_vec());
            } else {
                *self.fast_forward_stack.lock().unwrap() = None;
            }
        }

        // let initial_input_val = {
        //     let mut session = self.session.lock().unwrap();
        //     if session.turns.is_empty() && session.initial_input.is_none() {
        //         session.initial_input = input.clone();
        //     }
        //     session.initial_input.clone()
        // };

        let mut scope = HashMap::new();
        {
            // Always insert context into scope — use the actual value if set, otherwise
            // an empty object so context.* references evaluate to null instead of crashing.
            let ctx_val = self.context.lock().unwrap()
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            scope.insert("context".to_string(), ctx_val.clone());
            scope.insert("ctx".to_string(), ctx_val);
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
        let system_prompt = self.generate_prompt(None)?;
        self.session
            .lock()
            .unwrap()
            .set_system_prompt(&system_prompt);

        if let Some(response) = self
            .fire_middleware_event(serde_json::json!({
                "type": "run_start",
                "session": serde_json::from_str::<Value>(&self.export_session()?).map_err(AuwgentError::Serialization)?,
                "context": self.build_event_context(&self.ir.name, None, Some(system_prompt.clone())),
            }))
            .await
        {
            if let Some(updated_session) = response.get("session") {
                self.import_session(&serde_json::to_string(updated_session).map_err(AuwgentError::Serialization)?)?;
                self.sync_fast_forward_from_session();
            }
        }

        let run_start_handler = self.run_start_handler.lock().unwrap().clone();
        if let Some(h) = run_start_handler {
            let session_json = self.export_session()?;
            let context_json = self.serialize_host_context()?;
            if let Some(updated_session_json) = h(session_json, context_json).await {
                self.import_session(&updated_session_json)?;
                self.sync_fast_forward_from_session();
            }
        }

        // Regenerate the system prompt now that run_start middleware has had a chance
        // to call setContext(). This ensures any context injected during onRunStart
        // is reflected in the current run's system prompt, not just the next one.
        let system_prompt = self.generate_prompt(None)?;
        self.session
            .lock()
            .unwrap()
            .set_system_prompt(&system_prompt);

        // 3. Build the initial user input
        let initial_user_input = match input.as_ref() {
            Some(Value::String(s)) => Some(s.clone()),
            Some(v) => Some(serde_json::to_string(v).map_err(AuwgentError::Serialization)?),
            None => None,
        };

        *self.terminal_response_emitted.lock().unwrap() = false;
        *self.final_response_emitted.lock().unwrap() = false;

        // Start the first turn if an explicit input is provided AND we are not teleporting.
        // If we ARE teleporting, the turn will be started by the target agent.
        let is_teleporting = self.fast_forward_stack.lock().unwrap().is_some();

        if let Some(user_input) = initial_user_input {
            if !is_teleporting {
                self.session.lock().unwrap().start_turn(&user_input);
            }
        } else if self.session.lock().unwrap().turns.is_empty() {
            // Safety fallback: if no input and session is empty, start one
            if !is_teleporting {
                self.session.lock().unwrap().start_turn("");
            }
        }

        // Read max loops from lifecycle config, fallback to 12
        let max_loops: usize = self
            .ir
            .lifecycle
            .as_ref()
            .and_then(|lc| lc.0.get("maxMessages").and_then(|v| v.as_u64()))
            .map(|v| v as usize)
            .unwrap_or(12);

        *self.last_run_metadata.lock().unwrap() = RunMetadata::default();

        let mut loop_count = 0;

        let run_result = async {
            loop {
            loop_count += 1;
            if loop_count > max_loops {
                return Err(AuwgentError::MaxLoopsExceeded(max_loops));
            }

            if loop_count == 1 {
                *self.user_input.lock().unwrap() = input.clone();
            }

            self.current_raw_response.lock().unwrap().clear();
            *self.last_turn_response_value.lock().unwrap() = Value::Null;
            self.pending_tool_results.lock().unwrap().clear();
            self.emitted_partial_intents.lock().unwrap().clear();
            self.orchestrator.lock().unwrap().reset();

            // ── Stack-Aware Resumption: TELEPORTATION ─────────────────────
            // If we have a fast-forward stack, skip the LLM entirely and
            // inject a synthetic helper_call intent to jump straight to
            // the target agent. This is the core of Execution-Tunneling.
            let next_helper = {
                let ffs_lock = self.fast_forward_stack.lock().unwrap();
                ffs_lock.as_ref().and_then(|ffs| ffs.first().cloned())
            };

            if let Some(mut next_helper) = next_helper {
                // If the next helper is ACTUALLY this agent, consume it and look deeper
                if next_helper == self.ir.name {
                    let mut lock = self.fast_forward_stack.lock().unwrap();
                    if let Some(ffs) = lock.as_mut() {
                        if !ffs.is_empty() {
                            ffs.remove(0);
                        }
                        if ffs.is_empty() {
                            *lock = None;
                            continue; // Exit teleportation, go to LLM
                        }
                        next_helper = ffs.first().cloned().unwrap();
                    } else {
                        continue;
                    }
                }

                // Inject synthetic helper_call — no LLM needed
                let synthetic_intent = serde_json::json!({
                    "type": next_helper,
                    "args": {
                        "user_text": input.as_ref().and_then(|v| v.as_str()).unwrap_or("")
                    }
                });

                self.pending_intents
                    .lock()
                    .unwrap()
                    .push(("helper_call".to_string(), synthetic_intent));

                // Process the synthetic intent
                let (_terminal, actions, hard_stop) = self.process_intents().await?;

                if hard_stop {
                    // Record the teleportation turn in the parent session if it finished here
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

            // If this is the first turn (the actual user prompt, not a tool feedback cycle),
            // fire the onLlmStart interceptor. It can return a modified string.
            if loop_count == 1 {
                let start_handler = self.llm_start_handler.lock().unwrap().clone();
                if let Some(h) = start_handler {
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
                        .map(|t| t.input.clone())
                        .unwrap_or_default();

                    let context_json = {
                        let ctx_lock = self.context.lock().unwrap();
                        serde_json::to_string(ctx_lock.as_ref().unwrap_or(&Value::Null))
                            .unwrap_or_default()
                    };

                    let mut result = h(input_text.clone(), sys_prompt.clone(), context_json).await;

                    if let Some(middleware_result) = self
                        .apply_llm_start_middleware(&input_text, &sys_prompt, &self.ir.name)
                        .await
                    {
                        if let Some(modified) = middleware_result.get("prompt").and_then(|v| v.as_str()) {
                            result["prompt"] = Value::String(modified.to_string());
                        }
                        if let Some(new_stack) = middleware_result.get("stack").and_then(|v| v.as_array()) {
                            result["stack"] = Value::Array(new_stack.clone());
                        }
                    }

                    if let Some(modified) = result.get("prompt").and_then(|v| v.as_str()) {
                        self.session.lock().unwrap().set_input(modified.to_string());
                    }

                    if let Some(new_stack) = result.get("stack").and_then(|v| v.as_array()) {
                        let stack_vec: Vec<String> = new_stack
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !stack_vec.is_empty() {
                            let mut session = self.session.lock().unwrap();
                            session.stack = stack_vec;

                            // Re-evaluate teleportation!
                            if session.stack.len() > 1 {
                                *self.fast_forward_stack.lock().unwrap() =
                                    Some(session.stack[1..].to_vec());
                                drop(session); // Important: release lock before continuing
                                continue; // Restart loop to trigger teleportation check at line 489
                            }
                        }
                    }
                }
            }

            // Build message history from session state (AFTER potential interception)
            let messages = self.session.lock().unwrap().to_messages();

            // Stream from the driver using full message history
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
                Ok(s) => s,
                Err(e) => {
                    // Critical failure during the request phase (e.g. network error, invalid API key)
                    self.fire_intent("error".to_string(), serde_json::json!({ "message": e }))
                        .await;

                    // Ensure the session is not left with a hanging turn
                    self.session
                        .lock()
                        .unwrap()
                        .set_model_response(format!("(request error: {})", e));

                    return Err(AuwgentError::Driver(e));
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
                        let process_res = self.process_intents().await;
                        let (_terminal, actions, hard_stop) = match process_res {
                            Ok(res) => res,
                            Err(e) => {
                                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                                if !raw_resp.is_empty() {
                                    self.session.lock().unwrap().set_model_response(&raw_resp);
                                }
                                return Err(e);
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
                    Ok(ModelEvent::Usage(usage)) => {
                        turn_usage = usage;
                    }
                    Ok(ModelEvent::FinishReason(fr)) => {
                        turn_finish_reason = Some(fr);
                    }
                    Ok(ModelEvent::Metadata(meta)) => {
                        turn_usage = meta.usage;
                        turn_finish_reason = meta.finish_reason;
                    }
                    Err(e) => {
                        // Fire error as intent if handler exists
                        self.fire_intent(
                            "error".to_string(),
                            serde_json::json!({ "message": e.clone() }),
                        )
                        .await;

                        // Ensure we don't leave a hanging turn with no response in the session
                        // if we are about to return an error.
                        self.session
                            .lock()
                            .unwrap()
                            .set_model_response(format!("(error: {})", e));

                        return Err(AuwgentError::StreamError(e));
                    }
                }
            }

            let turn_metadata = TurnMetadata {
                turn_index: loop_count - 1,
                usage: turn_usage.clone(),
                finish_reason: turn_finish_reason.clone(),
                model: model_name.to_string(),
            };

            {
                let mut meta_lock = self.last_run_metadata.lock().unwrap();
                meta_lock.aggregate.prompt_tokens += turn_usage.prompt_tokens;
                meta_lock.aggregate.completion_tokens += turn_usage.completion_tokens;
                meta_lock.aggregate.total_tokens += turn_usage.total_tokens;
                meta_lock.turns.push(turn_metadata.clone());
            }

            // Finalize parsing
            let _final_val = self.orchestrator.lock().unwrap().end();

            let process_res = self.process_intents().await;
            let (_terminal, actions, hard_stop) = match process_res {
                Ok(res) => res,
                Err(e) => {
                    let raw_resp = self.current_raw_response.lock().unwrap().clone();
                    if !raw_resp.is_empty() {
                        self.session.lock().unwrap().set_model_response(&raw_resp);
                    }
                    return Err(e);
                }
            };
            if actions {
                actions_performed = true;
            }

            // Fire LLM end hook
            let end_handler = self.llm_end_handler.lock().unwrap().clone();
            if let Some(h) = end_handler {
                let sys_prompt = self
                    .session
                    .lock()
                    .unwrap()
                    .system_prompt
                    .clone()
                    .unwrap_or_default();
                let raw_resp = self.current_raw_response.lock().unwrap().clone();
                h(raw_resp, sys_prompt).await;
            }

            let sys_prompt = self
                .session
                .lock()
                .unwrap()
                .system_prompt
                .clone()
                .unwrap_or_default();
            self.notify_llm_end_middleware(
                &(self.last_turn_response_value()),
                &sys_prompt,
                &self.ir.name,
                &turn_metadata,
            )
            .await;

            // Save the raw LLM output in the session history so the exact
            // textual response is visible in logs and follow-up turns.
            let raw_resp = self.current_raw_response.lock().unwrap().clone();
            if !raw_resp.is_empty() {
                self.session.lock().unwrap().set_model_response(&raw_resp);
            }

            // Decide if we loop or stop
            if hard_stop {
                break;
            }

            // If the model performed actions, we MUST loop to feed the results back.
            // We only stop if there are no pending tool/helper results to provide.
            if !actions_performed {
                // Ensure the session has SOME model response before exiting the loop.
                // If the loop is ending and 'set_model_response' was never called,
                // we should at least mark it.
                let mut session = self.session.lock().unwrap();
                if let Some(turn) = session.current_turn_mut()
                    && turn.model_response.is_empty()
                {
                    turn.model_response =
                        empty_response_marker(turn_finish_reason.as_ref());
                }
                break;
            }

            // Feed tool/workflow results back to the LLM as the next turn's input
            let results_payload = self.build_results_payload();
            self.session.lock().unwrap().start_turn(&results_payload);
            }

            Ok(())
        }
        .await;

        match run_result {
            Ok(()) => {
                if let Some(_) = self
                    .fire_middleware_event(serde_json::json!({
                        "type": "run_complete",
                        "session": serde_json::from_str::<Value>(&self.export_session()?).map_err(AuwgentError::Serialization)?,
                        "context": self.build_event_context(&self.ir.name, None, None),
                    }))
                    .await
                {
                }
                let run_complete_handler = self.run_complete_handler.lock().unwrap().clone();
                if let Some(h) = run_complete_handler {
                    let session_json = self.export_session()?;
                    let context_json = self.serialize_host_context()?;
                    h(session_json, context_json).await;
                }
                Ok(())
            }
            Err(err) => {
                // If the run failed and no text was captured for the active turn,
                // persist the error text so follow-up turns do not silently collapse
                // into the generic "(no response)" placeholder.
                {
                    let mut session = self.session.lock().unwrap();
                    if let Some(turn) = session.current_turn_mut()
                        && turn.model_response.is_empty()
                    {
                        turn.model_response = format!("(error: {})", err);
                    }
                }

                let middleware_response = self
                    .fire_middleware_event(serde_json::json!({
                        "type": "error",
                        "error": { "message": err.to_string() },
                        "session": self.export_session().ok().and_then(|session| serde_json::from_str::<Value>(&session).ok()),
                        "context": self.build_event_context(&self.ir.name, None, None),
                    }))
                    .await;
                let error_handler = self.error_handler.lock().unwrap().clone();
                if let Some(h) = error_handler {
                    let error_json = serde_json::json!({ "message": err.to_string() }).to_string();
                    let session_json = self.export_session().ok();
                    let context_json = self.serialize_host_context()?;
                    if h(error_json, session_json, context_json).await {
                        return Ok(());
                    }
                }
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

    fn serialize_host_context(&self) -> AuwgentResult<String> {
        let session = self.session.lock().unwrap();
        let context_json = serde_json::json!({
            "activeAgent": self.ir.name,
            "stack": session.stack,
            "systemPrompt": session.system_prompt,
        });
        serde_json::to_string(&context_json).map_err(AuwgentError::Serialization)
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> AuwgentResult<String> {
        if let Some(name) = helper_name {
            let sub_ctx = crate::runtime::helper_runner::build_sub_agent_context(&self.ir, &name)?;
            let sub_engine = AuwgentEngine::new(sub_ctx.ir);

            // Propagate context so prompt evaluation works
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

        // Always inject context into scope — use the actual value if set, otherwise
        // an empty object so context.* references evaluate gracefully instead of crashing.
        {
            let ctx_val = self.context.lock().unwrap()
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
        let prompt_val = evaluator.evaluate(&parsed_prompt, &mut scope)?;
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

        let intents = crate::intents::generate_block_protocol_prompt(&self.ir);
        if !intents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&intents);
        }

        Ok(prompt)
    }
}
