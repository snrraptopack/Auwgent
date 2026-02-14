use crate::errors::{AuwgentError, AuwgentResult};
use crate::evaluator::Evaluator;
use crate::intent_parser::orchestrator::Orchestrator;
use crate::runtime::drivers::ModelDriver;
use crate::runtime::session::{RunStep, SessionState};
use crate::types::*;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type ToolImplementation = Arc<
    dyn Fn(Value) -> futures_util::future::BoxFuture<'static, Result<Value, String>> + Send + Sync,
>;

pub struct AuwgentEngine {
    ir: AgentIR,
    session: SessionState,
    tools: HashMap<String, ToolImplementation>,
    orchestrator: Orchestrator,
    driver: Option<Box<dyn ModelDriver>>,
    context: Option<Value>,
    /// Pending intents collected by the orchestrator callback
    pending_intents: Arc<Mutex<Vec<(String, Value)>>>,
    /// Accumulated raw response for the current turn
    current_raw_response: String,
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
            println!("\n[INTENT READY]: {} -> {}", name, value);
            if let Ok(mut pending) = intents_for_handler.lock() {
                pending.push((name, value));
            }
        }));

        Self {
            ir,
            session: SessionState::new(),
            tools: HashMap::new(),
            orchestrator,
            driver: None,
            context: None,
            pending_intents,
            current_raw_response: String::new(),
        }
    }

    pub fn set_driver(&mut self, driver: Box<dyn ModelDriver>) {
        self.driver = Some(driver);
    }

    pub fn set_context(&mut self, context: Value) {
        self.context = Some(context);
    }

    pub fn register_tool(&mut self, name: &str, implementation: ToolImplementation) {
        self.tools.insert(name.to_string(), implementation);
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
        let model_name = model_info["modelName"]
            .as_str()
            .unwrap_or("gemini-2.0-flash");
        let config_params = model_info.get("config").cloned();

        // 2. Generate system prompt and set it on the session
        let system_prompt = self.generate_prompt()?;
        self.session.set_system_prompt(&system_prompt);
        self.session.add_step(RunStep::Prompt {
            content: system_prompt.clone(),
        });

        // 3. Build the initial user input
        let initial_user_input = match input {
            Some(Value::String(s)) => s,
            Some(v) => serde_json::to_string(&v).map_err(AuwgentError::Serialization)?,
            None => "".to_string(),
        };

        // Start the first turn
        self.session.start_turn(&initial_user_input);

        let mut loop_count = 0;
        const MAX_LOOPS: usize = 12;

        loop {
            loop_count += 1;
            if loop_count > MAX_LOOPS {
                return Err(AuwgentError::MaxLoopsExceeded(MAX_LOOPS));
            }

            self.current_raw_response.clear();
            self.orchestrator.reset();

            // Build message history from session state
            let messages = self.session.to_messages();

            // Stream from the driver using full message history
            let mut stream = {
                let driver = self.driver.as_ref().ok_or(AuwgentError::NoDriver)?;
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
                            print!("{}", text);
                            use std::io::Write;
                            let _ = std::io::stdout().flush();
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
                        self.session.add_step(RunStep::Error { message: e.clone() });
                        return Err(AuwgentError::StreamError(e));
                    }
                }
            }

            self.orchestrator.end();
            let (terminal, actions) = self.process_intents().await?;
            if terminal {
                has_terminal_output = true;
            }
            if actions {
                actions_performed = true;
            }

            // Record the raw model response in the session turn
            self.session.set_model_response(&self.current_raw_response);
            self.session.add_step(RunStep::ModelResponse {
                content: self.current_raw_response.clone(),
            });

            // Decide if we loop or stop
            if has_terminal_output || !actions_performed {
                break;
            }

            // If we performed tool/workflow actions, start a new turn with the results
            let results_payload = self.build_results_payload();
            println!("\n--- FEEDING RESULTS BACK ---\n{}", results_payload);
            self.session.start_turn(&results_payload);
        }

        Ok(())
    }

    fn build_results_payload(&self) -> String {
        // Use the current turn's steps to build the results
        if let Some(turn) = self.session.turns.last() {
            let results: Vec<String> = turn
                .steps
                .iter()
                .filter_map(|step| {
                    if let RunStep::IntentAction {
                        name,
                        result: Some(res),
                        ..
                    } = step
                    {
                        Some(format!("tool_result:\n  name: {}\n  result: {}", name, res))
                    } else {
                        None
                    }
                })
                .collect();
            results.join("\n\n")
        } else {
            String::new()
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

        for (name, value) in intents {
            match name.as_str() {
                "tool_call" => {
                    self.execute_tool(value).await?;
                    has_actions = true;
                }
                "workflow_call" => {
                    self.execute_workflow(value).await?;
                    has_actions = true;
                }
                "response_schema" | "response_text" => {
                    self.session.add_step(RunStep::ModelOutput {
                        text: None,
                        raw_yaml: Some(
                            serde_json::to_string(&value).map_err(AuwgentError::Serialization)?,
                        ),
                    });
                    has_terminal = true;
                }
                _ => {}
            }
        }
        Ok((has_terminal, has_actions))
    }

    async fn execute_tool(&mut self, call: Value) -> AuwgentResult<()> {
        let tool_name = call["type"].as_str().unwrap_or("");
        let args = call["args"].clone();

        self.session.add_step(RunStep::IntentAction {
            name: tool_name.to_string(),
            args: args.clone(),
            result: None,
        });

        if let Some(imp) = self.tools.get(tool_name) {
            let result = imp(args).await;
            match result {
                Ok(val) => {
                    if let Some(RunStep::IntentAction { result: res, .. }) =
                        self.session.steps.last_mut()
                    {
                        *res = Some(val);
                    }
                }
                Err(e) => {
                    self.session.add_step(RunStep::Error { message: e });
                }
            }
        } else {
            self.session.add_step(RunStep::Error {
                message: format!("Tool not found: {}", tool_name),
            });
        }

        Ok(())
    }

    async fn execute_workflow(&mut self, call: Value) -> AuwgentResult<()> {
        let wf_name = call["type"].as_str().unwrap_or("");
        let args = call["args"].clone();

        if let Some(wf) = self.ir.workflows.iter().find(|w| w.name == wf_name) {
            self.session.add_step(RunStep::IntentAction {
                name: format!("workflow:{}", wf_name),
                args: args.clone(),
                result: None,
            });

            let evaluator = Evaluator::new(&self.ir);
            let mut scope = HashMap::new();
            if let Some(obj) = args.as_object() {
                for (k, v) in obj {
                    scope.insert(k.clone(), v.clone());
                }
            }

            let mut last_result = Value::Null;
            for stmt in &wf.body {
                last_result = evaluator.evaluate(stmt, &mut scope)?;
            }

            if let Some(RunStep::IntentAction { result: res, .. }) = self.session.steps.last_mut() {
                *res = Some(last_result);
            }
        }

        Ok(())
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

    pub fn get_session_steps(&self) -> &Vec<RunStep> {
        &self.session.steps
    }
}
