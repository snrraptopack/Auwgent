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
            pending_intents,
            current_raw_response: String::new(),
        }
    }

    pub fn set_driver(&mut self, driver: Box<dyn ModelDriver>) {
        self.driver = Some(driver);
    }

    pub fn register_tool(&mut self, name: &str, implementation: ToolImplementation) {
        self.tools.insert(name.to_string(), implementation);
    }

    /// Execute the agentic loop.
    pub async fn run(&mut self, input: Option<Value>) -> Result<(), Box<dyn std::error::Error>> {
        let evaluator = Evaluator::new(&self.ir);

        // 1. Evaluate Model Info
        let model_entry = self.ir.model_config.first().ok_or("No model config")?;
        let default_config = model_entry
            .default_config
            .as_ref()
            .ok_or("No default config")?;

        let mut scope = HashMap::new();
        if let Some(val) = input.as_ref() {
            scope.insert("input".to_string(), val.clone());
        }

        let model_info = evaluator.evaluate_model(default_config, &mut scope)?;
        let model_name = model_info["modelName"]
            .as_str()
            .unwrap_or("gemini-2.0-flash");
        let config_params = model_info.get("config").cloned();

        // Add the initial prompt to the session
        let initial_system_prompt = self.generate_prompt()?;
        self.session.add_step(RunStep::Prompt {
            content: initial_system_prompt.clone(),
        });

        let mut current_user_input = match input {
            Some(Value::String(s)) => s,
            Some(v) => serde_json::to_string(&v)?,
            None => "".to_string(),
        };

        let mut loop_count = 0;
        const MAX_LOOPS: usize = 12;

        loop {
            loop_count += 1;
            if loop_count > MAX_LOOPS {
                return Err("Max agentic loops reached".into());
            }

            self.current_raw_response.clear();
            self.orchestrator.reset();

            // Scope the driver borrow to avoid holding it while calling self.process_intents()
            let mut stream = {
                let driver = self
                    .driver
                    .as_ref()
                    .ok_or("No driver configured for AuwgentEngine")?;
                driver
                    .stream_generate_content(
                        model_name,
                        &current_user_input,
                        Some(&initial_system_prompt),
                        config_params.clone(),
                    )
                    .await?
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
                        self.session.add_step(RunStep::Error { message: e });
                        return Err("LLM Stream error".into());
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

            // Record the raw model response in session
            self.session.add_step(RunStep::ModelResponse {
                content: self.current_raw_response.clone(),
            });

            // Decide if we loop or stop
            if has_terminal_output || !actions_performed {
                break;
            }

            // If we performed actions (tools/workflows), we need to feed the results back
            // We build a new "user input" which is the collection of results from this turn
            current_user_input = self.build_results_payload();
            println!("\n--- FEEDING RESULTS BACK ---\n{}", current_user_input);
        }

        Ok(())
    }

    fn build_results_payload(&self) -> String {
        let mut results = Vec::new();
        // Look at the latest actions in the session
        for step in self.session.steps.iter().rev() {
            match step {
                RunStep::IntentAction { name, result, .. } => {
                    if let Some(res) = result {
                        results.push(format!("tool_result:\n  name: {}\n  result: {}", name, res));
                    }
                }
                RunStep::Prompt { .. } | RunStep::ModelResponse { .. } => break, // Stop at the start of this turn
                _ => {}
            }
        }
        results.reverse();
        results.join("\n\n")
    }

    pub async fn process_intents(&mut self) -> Result<(bool, bool), Box<dyn std::error::Error>> {
        let intents = {
            let mut pending = self.pending_intents.lock().unwrap();
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
                        raw_yaml: Some(serde_json::to_string(&value)?),
                    });
                    has_terminal = true;
                }
                _ => {}
            }
        }
        Ok((has_terminal, has_actions))
    }

    async fn execute_tool(&mut self, call: Value) -> Result<(), Box<dyn std::error::Error>> {
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

    async fn execute_workflow(&mut self, call: Value) -> Result<(), Box<dyn std::error::Error>> {
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

    pub fn generate_prompt(&self) -> Result<String, Box<dyn std::error::Error>> {
        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();

        let entry = self.ir.model_config.first().ok_or("No model config")?;
        let default = entry.default_config.as_ref().ok_or("No default config")?;

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
