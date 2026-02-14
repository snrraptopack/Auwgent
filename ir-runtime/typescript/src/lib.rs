#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction};
use napi_derive::napi;

use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::gemini::GeminiDriver;
use ir_runtime::runtime::engine::ToolImplementation;

use ir_runtime::types::AgentIR;

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

// ═══════════════════════════════════════════════════════════════════════════
// AUWGENT — napi-rs FFI class
// ═══════════════════════════════════════════════════════════════════════════

#[napi]
pub struct Auwgent {
    engine: Arc<Mutex<AuwgentEngine>>,
    /// The parsed IR — kept for introspection (tool listing, etc.)
    ir: Arc<AgentIR>,
    /// Tokio runtime for async ops
    rt: Arc<tokio::runtime::Runtime>,
}

#[napi]
impl Auwgent {
    /// Create an Auwgent engine from a JSON IR string.
    ///
    /// ```js
    /// const agent = new Auwgent(fs.readFileSync('main.agent.json', 'utf8'));
    /// ```
    #[napi(constructor)]
    pub fn new(ir_json: String) -> Result<Self> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse IR JSON: {}", e)))?;

        let engine = AuwgentEngine::new(ir.clone());

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::from_reason(format!("Failed to create tokio runtime: {}", e)))?;

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            ir: Arc::new(ir),
            rt: Arc::new(rt),
        })
    }

    /// Set the Gemini driver with the given API key.
    #[napi]
    pub fn set_gemini_driver(&self, api_key: String) -> Result<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.set_driver(Box::new(GeminiDriver::new(api_key)));
        });
        Ok(())
    }

    /// Register a tool by name. The callback receives a JSON args object
    /// and must return a JSON result.
    ///
    /// ```js
    /// agent.registerTool('search', async (args) => {
    ///   const result = await search(args.query);
    ///   return { results: result };
    /// });
    /// ```
    #[napi(ts_args_type = "name: string, callback: (args: any) => Promise<any>")]
    pub fn register_tool(&self, name: String, callback: JsFunction) -> Result<()> {
        // Create a ThreadsafeFunction that can be called from any thread
        let tsfn: ThreadsafeFunction<Value, ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx| {
                // Convert serde_json::Value -> napi JsValue via serde
                let js_val = ctx.env.to_js_value(&ctx.value)?;
                Ok(vec![js_val])
            })?;

        // Wrap the TSFN into a ToolImplementation closure
        let tool_impl: ToolImplementation = Arc::new(move |args: Value| {
            let tsfn = tsfn.clone();
            Box::pin(async move {
                // Call the JS function from the Rust async context
                let result = tsfn.call_async::<Promise<Value>>(args).await;
                match result {
                    Ok(promise) => promise.await.map_err(|e| format!("Tool JS error: {}", e)),
                    Err(e) => Err(format!("Tool call failed: {}", e)),
                }
            })
        });

        let engine = self.engine.clone();
        let tool_name = name.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_tool(&tool_name, tool_impl);
        });

        Ok(())
    }

    /// Run the agentic loop with the given input.
    /// Returns the exported session state as JSON.
    ///
    /// ```js
    /// const session = await agent.run('Hello, agent!');
    /// ```
    #[napi]
    pub async fn run(&self, input: Option<String>) -> Result<String> {
        let engine = self.engine.clone();

        let input_val =
            input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));

        let rt = self.rt.clone();
        let result: std::result::Result<String, String> = tokio::task::spawn_blocking(move || {
            rt.block_on(async {
                let mut eng = engine.lock().await;
                eng.run(input_val).await.map_err(|e| format!("{}", e))?;
                eng.export_session().map_err(|e| format!("{}", e))
            })
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task join error: {}", e)))?;

        result.map_err(|e| Error::from_reason(e))
    }

    /// Export the current session state as a JSON string.
    /// The host can persist this and restore it later with `importSession()`.
    #[napi]
    pub fn export_session(&self) -> Result<String> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let eng = engine.lock().await;
            eng.export_session()
                .map_err(|e| Error::from_reason(format!("{}", e)))
        })
    }

    /// Import a previously exported session state.
    #[napi]
    pub fn import_session(&self, json: String) -> Result<()> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.import_session(&json)
                .map_err(|e| Error::from_reason(format!("{}", e)))
        })
    }

    /// Clear the session (start a fresh conversation).
    #[napi]
    pub fn clear_session(&self) -> Result<()> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.clear_session();
        });
        Ok(())
    }

    /// Generate the system prompt (useful for debugging).
    #[napi]
    pub fn generate_prompt(&self) -> Result<String> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let eng = engine.lock().await;
            eng.generate_prompt()
                .map_err(|e| Error::from_reason(format!("{}", e)))
        })
    }

    /// Get all tool names defined in the IR.
    /// Used by the TypeScript wrapper for type-safe tool registration.
    #[napi]
    pub fn get_tool_names(&self) -> Vec<String> {
        self.ir.tools.iter().map(|t| t.name.clone()).collect()
    }

    /// Get tool schemas as JSON (for TypeScript type generation).
    #[napi]
    pub fn get_tool_schemas(&self) -> Result<String> {
        let schemas: Vec<Value> = self
            .ir
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "params": t.params,
                    "returns": t.returns,
                })
            })
            .collect();
        serde_json::to_string(&schemas).map_err(|e| Error::from_reason(format!("{}", e)))
    }

    /// Get the final session steps (for debugging).
    #[napi]
    pub fn get_session_steps(&self) -> Result<String> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let eng = engine.lock().await;
            let steps = eng.get_session_steps();
            serde_json::to_string(steps).map_err(|e| Error::from_reason(format!("{}", e)))
        })
    }

    /// Write a chunk directly to the orchestrator (for simulation/testing).
    #[napi]
    pub fn write_chunk(&self, chunk: String) -> Result<()> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.write_llm_chunk(&chunk);
        });
        Ok(())
    }

    /// Finalize the LLM stream (for simulation/testing).
    #[napi]
    pub fn end_stream(&self) -> Result<String> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        rt.block_on(async {
            let mut eng = engine.lock().await;
            let val = eng.end_llm_stream();
            serde_json::to_string(&val).map_err(|e| Error::from_reason(format!("{}", e)))
        })
    }

    /// Process any pending intents (for simulation/testing).
    #[napi]
    pub async fn process_intents(&self) -> Result<String> {
        let engine = self.engine.clone();
        let rt = self.rt.clone();
        let result: std::result::Result<String, String> = tokio::task::spawn_blocking(move || {
            rt.block_on(async {
                let mut eng = engine.lock().await;
                let (terminal, actions) =
                    eng.process_intents().await.map_err(|e| format!("{}", e))?;
                Ok(serde_json::json!({
                    "terminal": terminal,
                    "actions": actions,
                })
                .to_string())
            })
        })
        .await
        .map_err(|e| Error::from_reason(format!("Task join error: {}", e)))?;

        result.map_err(|e| Error::from_reason(e))
    }
}
