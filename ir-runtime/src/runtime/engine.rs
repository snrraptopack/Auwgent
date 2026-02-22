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

// ═══════════════════════════════════════════════════════════════════════════
// AUWGENT ENGINE
// ═══════════════════════════════════════════════════════════════════════════

pub struct AuwgentEngine {
    ir: AgentIR,
    session: SessionState,
    tools: HashMap<String, ToolImplementation>,
    orchestrator: Orchestrator,
    drivers: HashMap<String, Box<dyn ModelDriver>>,
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
        }
    }

    pub fn register_driver(&mut self, provider_type: &str, driver: Box<dyn ModelDriver>) {
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
    pub async fn run(&mut self, input: Option<Value>) -> AuwgentResult<()> {
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
        let provider_type = model_info["type"].as_str().unwrap_or("gemini");
        let model_name = model_info["modelName"]
            .as_str()
            .unwrap_or("gemini-2.0-flash");
        let config_params = model_info.get("config").cloned();

        // 2. Generate system prompt and set it on the session
        let system_prompt = self.generate_prompt()?;
        self.session.set_system_prompt(&system_prompt);

        // 3. Build the initial user input
        let initial_user_input = match input {
            Some(Value::String(s)) => s,
            Some(v) => serde_json::to_string(&v).map_err(AuwgentError::Serialization)?,
            None => "".to_string(),
        };

        // Start the first turn
        self.session.start_turn(&initial_user_input);

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

            // Build message history from session state
            let messages = self.session.to_messages();

            // Stream from the driver using full message history
            let mut stream = {
                let driver = self
                    .drivers
                    .get(provider_type)
                    .ok_or_else(|| AuwgentError::NoDriver)?;
                driver
                    .stream_generate(model_name, &messages, config_params.clone())
                    .await
                    .map_err(AuwgentError::Driver)?
            };

            let mut has_terminal_output = false;
            let mut actions_performed = false;

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(text) => {
                        if !text.is_empty() {
                            self.current_raw_response.push_str(&text);
                        }
                        self.orchestrator.write(&text);
                        let (terminal, actions) = self.process_intents().await?;
                        if terminal {
                            has_terminal_output = true;
                        }
                        if actions {
                            actions_performed = true;
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
            eprintln!(
                "[RAW MODEL OUTPUT turn {}]\n{}\n[/RAW]",
                loop_count, self.current_raw_response
            );

            // Finalize parsing and get the full parsed JSON from the intent parser
            let parsed_response = self.orchestrator.end();
            let (terminal, actions) = self.process_intents().await?;
            if terminal {
                has_terminal_output = true;
            }
            if actions {
                actions_performed = true;
            }

            // Save the parsed JSON as model_response.
            // The intent parser already converts the LLM's YAML output to JSON,
            // so we just serialize it directly.
            if parsed_response != Value::Null {
                let parsed = serde_json::to_string(&parsed_response).unwrap_or_default();
                self.session.set_model_response(&parsed);
            } else {
                self.session.set_model_response(&self.current_raw_response);
            }

            // Decide if we loop or stop
            if has_terminal_output || !actions_performed {
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
            // Format as YAML blocks matching the intent schema
            let result_str = match serde_json::to_string(result) {
                Ok(s) => s,
                Err(_) => format!("{:?}", result),
            };
            parts.push(format!(
                "tool_result:\n  name: {}\n  result: {}",
                name, result_str
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

    pub async fn process_intents(&mut self) -> AuwgentResult<(bool, bool)> {
        let intents = {
            let mut pending = self
                .pending_intents
                .lock()
                .expect("pending_intents mutex poisoned");
            std::mem::take(&mut *pending)
        };

        let mut has_terminal = false;
        let mut has_actions = false;
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

        Ok((has_terminal, has_actions))
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

    async fn execute_workflow(&self, call: &Value) -> AuwgentResult<(String, Value)> {
        let wf_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        if let Some(wf) = self.ir.workflows.iter().find(|w| w.name == wf_name) {
            // Create evaluator WITH tools so workflow body can call functions (#4)
            let mut tool_fns: HashMap<String, crate::evaluator::SyncToolFn> = HashMap::new();
            for (name, imp) in &self.tools {
                let imp = imp.clone();
                let name_clone = name.clone();
                tool_fns.insert(
                    name.clone(),
                    Box::new(move |fn_args: Vec<Value>| {
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

            let evaluator = Evaluator::with_tools(&self.ir, tool_fns);
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
            for stmt in &wf.body {
                last_result = evaluator.evaluate(stmt, &mut scope)?;
            }

            Ok((wf_name, last_result))
        } else {
            Ok((
                wf_name.clone(),
                serde_json::json!({ "error": format!("Workflow not found: {}", wf_name) }),
            ))
        }
    }

    /// Execute a helper by running a sub-engine with the helper's model config and prompt.
    async fn execute_helper(&mut self, call: &Value) -> AuwgentResult<(String, Value)> {
        let helper_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        if let Some(helper) = self.ir.helpers.iter().find(|h| h.name == helper_name) {
            // Build a sub-prompt from the helper's model config
            let evaluator = Evaluator::new(&self.ir);
            let mut scope = HashMap::new();
            if let Some(obj) = args.as_object() {
                for (k, v) in obj {
                    scope.insert(k.clone(), v.clone());
                }
            }
            if let Some(ctx) = self.context.as_ref() {
                scope.insert("context".to_string(), ctx.clone());
            }

            // Evaluate the helper's prompt
            let helper_prompt = if let Some(entry) = helper.model_config.first() {
                if let Some(cfg) = &entry.default_config {
                    let prompt_val = evaluator.evaluate(&cfg.prompt, &mut scope)?;
                    prompt_val.as_str().unwrap_or("").to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Append helper-specific intents
            let helper_intents = crate::intents::generate_helper_intents(&self.ir, &helper_name);
            let full_prompt = if helper_intents.is_empty() {
                helper_prompt
            } else {
                format!("{}\n\n{}", helper_prompt, helper_intents)
            };

            // Evaluate the helper's model info to get credentials and provider type
            let (provider_type, model_name) = if let Some(entry) = helper.model_config.first() {
                if let Some(cfg) = &entry.default_config {
                    let model_info = evaluator.evaluate_model(cfg, &mut scope)?;
                    let p_type = model_info["type"].as_str().unwrap_or("gemini").to_string();
                    let m_name = model_info["modelName"]
                        .as_str()
                        .unwrap_or("gemini-2.0-flash")
                        .to_string();
                    (p_type, m_name)
                } else {
                    ("gemini".to_string(), "gemini-2.0-flash".to_string())
                }
            } else {
                ("gemini".to_string(), "gemini-2.0-flash".to_string())
            };

            // If we have a driver for the requested type, run a single LLM call for the helper
            if let Some(driver) = self.drivers.get(&provider_type) {
                let messages = vec![
                    crate::runtime::session::Message::system(&full_prompt),
                    crate::runtime::session::Message::user(
                        serde_json::to_string(&args).unwrap_or_default(),
                    ),
                ];

                let mut stream = driver
                    .stream_generate(&model_name, &messages, None)
                    .await
                    .map_err(AuwgentError::Driver)?;

                let mut response = String::new();
                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(text) => response.push_str(&text),
                        Err(e) => return Err(AuwgentError::StreamError(e)),
                    }
                }

                Ok((helper_name, Value::String(response)))
            } else {
                Ok((
                    helper_name,
                    serde_json::json!({ "error": format!("No driver registered for provider type '{}'", provider_type) }),
                ))
            }
        } else {
            Ok((
                helper_name.clone(),
                serde_json::json!({ "error": format!("Helper not found: {}", helper_name) }),
            ))
        }
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
