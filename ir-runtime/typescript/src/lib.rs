#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction};
use napi_derive::napi;

use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::ModelDriver;
use ir_runtime::runtime::drivers::gemini::GeminiDriver;
use ir_runtime::runtime::drivers::openai::OpenAIDriver;
use ir_runtime::runtime::engine::{IntentControl, ToolImplementation};

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
            eng.register_driver(
                "gemini",
                std::sync::Arc::new(GeminiDriver::new(api_key)) as std::sync::Arc<dyn ModelDriver>,
            );
        });
        Ok(())
    }

    /// Set the OpenAI driver with the given API key and optional Custom URL.
    #[napi]
    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) -> Result<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_driver(
                "openai",
                std::sync::Arc::new(OpenAIDriver::new(api_key.clone(), None))
                    as std::sync::Arc<dyn ModelDriver>,
            );
            if let Some(url) = base_url {
                eng.register_driver(
                    "custom",
                    std::sync::Arc::new(OpenAIDriver::new(api_key, Some(url)))
                        as std::sync::Arc<dyn ModelDriver>,
                );
            }
        });
        Ok(())
    }

    #[napi]
    pub fn set_context(&self, context: Value) -> Result<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.set_context(context);
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

    /// Register an intent callback for real-time streaming events.
    ///
    /// The callback fires for every detected intent during the agentic loop:
    /// - `"tool_call"` — LLM requested a tool call (value: { type, args })
    /// - `"tool_result"` — Tool finished (value: { name, result })
    /// - `"response_text"` — LLM text response (value: { text })
    /// - `"response_schema"` — LLM structured output
    /// - `"workflow_call"` — LLM requested a workflow
    /// - `"error"` — An error occurred
    ///
    /// Return value controls behavior:
    /// - `undefined` / `null` → engine auto-executes (default)
    /// - `{ skip: true }` → skip this tool/workflow call
    /// - `{ result: value }` → use this result instead of executing
    ///
    /// ```js
    /// agent.onIntent((name, value) => {
    ///   console.log(`[${name}]`, value);
    /// });
    /// ```
    #[napi(ts_args_type = "callback: (name: string, value: any) => any")]
    pub fn on_intent(&self, callback: JsFunction) -> Result<()> {
        // Create a TSFN that receives (name, value) as a tuple
        let tsfn: ThreadsafeFunction<(String, Value), ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, Value)>| {
                let name = ctx.env.create_string(&ctx.value.0)?;
                let value = ctx.env.to_js_value(&ctx.value.1)?;
                Ok(vec![name.into_unknown(), value])
            })?;

        // Wrap into an AsyncIntentCallback
        let handler: ir_runtime::runtime::engine::AsyncIntentCallback =
            Arc::new(move |name: String, value: Value| {
                let tsfn = tsfn.clone();
                Box::pin(async move {
                    // Call the JS callback and check return value
                    let result = tsfn.call_async::<Promise<Value>>((name, value)).await;
                    match result {
                        Ok(promise) => match promise.await {
                            Ok(ret) => parse_intent_control(&ret),
                            Err(_) => None,
                        },
                        Err(_) => None,
                    }
                })
            });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_intent(handler);
        });

        Ok(())
    }

    /// Register a partial intent callback for streaming updates.
    ///
    /// This fires as YAML data streams in, BEFORE the intent block is
    /// complete. Useful for streaming partial text or showing tool args
    /// as they arrive. Observational only — no control/skip/override.
    ///
    /// ```js
    /// agent.onIntentPartial((name, value) => {
    ///   if (name === 'response_text') {
    ///     process.stdout.write(value.text ?? '');
    ///   }
    /// });
    /// ```
    #[napi(ts_args_type = "callback: (name: string, value: any) => void")]
    pub fn on_intent_partial(&self, callback: JsFunction) -> Result<()> {
        use napi::threadsafe_function::ThreadsafeFunctionCallMode;

        let tsfn: ThreadsafeFunction<(String, Value), ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, Value)>| {
                let name = ctx.env.create_string(&ctx.value.0)?;
                let value = ctx.env.to_js_value(&ctx.value.1)?;
                Ok(vec![name.into_unknown(), value])
            })?;

        // Wrap into a sync callback (partials are fire-and-forget, no await)
        let handler: Arc<dyn Fn(String, Value) + Send + Sync> =
            Arc::new(move |name: String, value: Value| {
                // Non-blocking call — don't await, just fire
                tsfn.call((name, value), ThreadsafeFunctionCallMode::NonBlocking);
            });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_intent_partial(handler);
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

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Parse a JS return value into an IntentControl signal.
/// Returns None if the value is null/undefined (proceed normally).
fn parse_intent_control(val: &Value) -> Option<IntentControl> {
    match val {
        Value::Null => None,
        Value::Object(obj) => {
            if obj.get("skip").and_then(|v| v.as_bool()) == Some(true) {
                Some(IntentControl::Skip)
            } else if let Some(result) = obj.get("result") {
                Some(IntentControl::Override {
                    result: result.clone(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}
