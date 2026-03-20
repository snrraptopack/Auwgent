#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction};
use napi_derive::napi;

use ir_runtime::runtime::bridge::EngineBridge;
use ir_runtime::runtime::engine::IntentControl;

use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════
// AUWGENT — napi-rs FFI class
// ═══════════════════════════════════════════════════════════════════════════

#[napi]
pub struct Auwgent {
    bridge: EngineBridge,
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
        let bridge = EngineBridge::new(ir_json).map_err(Error::from_reason)?;
        Ok(Self { bridge })
    }

    /// Set the Gemini driver with the given API key.
    #[napi]
    pub fn set_gemini_driver(&self, api_key: String) -> Result<()> {
        self.bridge.set_gemini_driver(api_key);
        Ok(())
    }

    /// Set the OpenAI driver with the given API key.
    #[napi]
    pub fn set_openai_driver(&self, api_key: String) -> Result<()> {
        self.bridge.set_openai_driver(api_key, None);
        Ok(())
    }

    /// Set a custom OpenAI-compatible driver with a unique ID.
    #[napi]
    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) -> Result<()> {
        self.bridge.set_custom_driver(id, api_key, base_url);
        Ok(())
    }

    #[napi]
    pub fn set_context(&self, context: Value) -> Result<()> {
        self.bridge.set_context(context);
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
        let tool_impl: ir_runtime::runtime::engine::ToolImplementation = std::sync::Arc::new(move |args: Value| {
            let tsfn = tsfn.clone();
            Box::pin(async move {
                // Call the JS function from the Rust async context
                let result = tsfn.call_async::<Promise<Value>>(args).await;
                match result {
                    Ok(promise) => promise.await.map_err(|e| e.reason.clone()),
                    Err(e) => Err(e.reason.clone()),
                }
            })
        });

        self.bridge.engine.register_tool(&name, tool_impl);

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
            std::sync::Arc::new(move |name: String, value: Value| {
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

        self.bridge.engine.on_intent(handler);

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
        let handler: std::sync::Arc<dyn Fn(String, Value) + Send + Sync> =
            std::sync::Arc::new(move |name: String, value: Value| {
                // Non-blocking call — don't await, just fire
                tsfn.call((name, value), ThreadsafeFunctionCallMode::NonBlocking);
            });

        self.bridge.engine.on_intent_partial(handler);

        Ok(())
    }

    /// Hook for TypeScript to preload a helper session before sub_engine.run()
    #[napi(
        ts_args_type = "callback: (helperName: string, emptySessionJson: string) => Promise<string | undefined>"
    )]
    pub fn on_sub_engine_start(&self, callback: JsFunction) -> Result<()> {
        let tsfn: ThreadsafeFunction<(String, String), ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, String)>| {
                let name = ctx.env.create_string(&ctx.value.0)?;
                let session = ctx.env.create_string(&ctx.value.1)?;
                Ok(vec![name.into_unknown(), session.into_unknown()])
            })?;

        let handler: ir_runtime::runtime::engine::AsyncSessionPreloadCallback =
            std::sync::Arc::new(move |name: String, empty_session: String| {
                let tsfn = tsfn.clone();
                Box::pin(async move {
                    let result = tsfn
                        .call_async::<Promise<Option<String>>>((name, empty_session))
                        .await;
                    match result {
                        Ok(promise) => promise.await.unwrap_or(None),
                        Err(_) => None,
                    }
                })
            });

        self.bridge.engine.on_sub_engine_start(handler);

        Ok(())
    }

    /// Hook for TypeScript to save a helper session after sub_engine.run()
    #[napi(
        ts_args_type = "callback: (helperName: string, completedSessionJson: string) => Promise<void>"
    )]
    pub fn on_sub_engine_complete(&self, callback: JsFunction) -> Result<()> {
        let tsfn: ThreadsafeFunction<(String, String), ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, String)>| {
                let name = ctx.env.create_string(&ctx.value.0)?;
                let session = ctx.env.create_string(&ctx.value.1)?;
                Ok(vec![name.into_unknown(), session.into_unknown()])
            })?;

        let handler: ir_runtime::runtime::engine::SessionSaveCallback =
            std::sync::Arc::new(move |name: String, completed_session: String| {
                let tsfn = tsfn.clone();
                Box::pin(async move {
                    let result = tsfn
                        .call_async::<Promise<()>>((name, completed_session))
                        .await;
                    if let Ok(promise) = result {
                        let _ = promise.await;
                    }
                })
            });

        self.bridge.engine.on_sub_engine_complete(handler);

        Ok(())
    }

    /// Hook for TypeScript to receive the prompt before LLM generation
    #[napi(
        ts_args_type = "callback: (prompt: string, systemPrompt: string, contextJson: string) => Promise<{ prompt?: string, stack?: string[] } | undefined>"
    )]
    pub fn on_llm_start(&self, callback: JsFunction) -> Result<()> {
        let tsfn: ThreadsafeFunction<(String, String, String), ErrorStrategy::Fatal> =
            callback.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, String, String)>| {
                let prompt = ctx.env.create_string(&ctx.value.0)?;
                let sys = ctx.env.create_string(&ctx.value.1)?;
                let context = ctx.env.create_string(&ctx.value.2)?;
                Ok(vec![
                    prompt.into_unknown(),
                    sys.into_unknown(),
                    context.into_unknown(),
                ])
            })?;

        let handler: ir_runtime::runtime::engine::AsyncLlmStartCallback =
            std::sync::Arc::new(move |prompt_str: String, sys_str: String, ctx_str: String| {
                let tsfn = tsfn.clone();
                Box::pin(async move {
                    let result = tsfn
                        .call_async::<Promise<Value>>((prompt_str, sys_str, ctx_str))
                        .await;
                    match result {
                        Ok(promise) => promise.await.unwrap_or(Value::Null),
                        Err(_) => Value::Null,
                    }
                })
            });

        self.bridge.engine.on_llm_start(handler);

        Ok(())
    }

    /// Hook for TypeScript to receive the unparsed response after LLM generation
    #[napi(
        ts_args_type = "callback: (responseString: string, systemPrompt: string) => Promise<void>"
    )]
    pub fn on_llm_end(&self, callback: JsFunction) -> Result<()> {
        let tsfn: ThreadsafeFunction<(String, String), ErrorStrategy::Fatal> = callback
            .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<(String, String)>| {
                let response = ctx.env.create_string(&ctx.value.0)?;
                let sys = ctx.env.create_string(&ctx.value.1)?;
                Ok(vec![response.into_unknown(), sys.into_unknown()])
            })?;

        let handler: ir_runtime::runtime::engine::AsyncLlmEndCallback =
            std::sync::Arc::new(move |response_string: String, sys_string: String| {
                let tsfn = tsfn.clone();
                Box::pin(async move {
                    let result = tsfn
                        .call_async::<Promise<()>>((response_string, sys_string))
                        .await;
                    if let Ok(promise) = result {
                        let _ = promise.await;
                    }
                })
            });

        self.bridge.engine.on_llm_end(handler);

        Ok(())
    }

    #[napi]
    pub fn clear_listeners(&self) -> Result<()> {
        self.bridge.clear_listeners();
        Ok(())
    }

    /// Run the agentic loop with the given input.
    /// Returns the exported session state as JSON.
    ///
    /// `initial_stack_json`: Optional JSON array of agent names for Stack-Aware Resumption.
    /// Example: `'["Main", "Broker", "RiskValidator"]'`
    ///
    /// ```js
    /// const session = await agent.run('Hello, agent!');
    /// // or with stack resumption:
    /// const session = await agent.run('Hello', JSON.stringify(savedStack));
    /// ```
    #[napi]
    pub async fn run(&self, input: Option<String>, initial_stack_json: Option<String>) -> Result<String> {
        let input_val =
            input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));

        // Parse the optional initial stack JSON array
        let initial_stack: Option<Vec<String>> = initial_stack_json
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok());

        self.bridge.run_async(input_val, initial_stack).await.map_err(Error::from_reason)
    }

    /// Export the current session state as a JSON string.
    /// The host can persist this and restore it later with `importSession()`.
    #[napi]
    pub fn export_session(&self) -> Result<String> {
        self.bridge.export_session().map_err(Error::from_reason)
    }

    /// Import a previously exported session state.
    #[napi]
    pub fn import_session(&self, json: String) -> Result<()> {
        self.bridge.import_session(json).map_err(Error::from_reason)
    }

    /// Clear the session (start a fresh conversation).
    #[napi]
    pub fn clear_session(&self) -> Result<()> {
        self.bridge.clear_session();
        Ok(())
    }

    /// Generate the system prompt (useful for debugging).
    #[napi]
    pub fn generate_prompt(&self, helper_name: Option<String>) -> Result<String> {
        self.bridge.generate_prompt(helper_name).map_err(Error::from_reason)
    }

    /// Get all tool names defined in the IR.
    /// Used by the TypeScript wrapper for type-safe tool registration.
    #[napi]
    pub fn get_tool_names(&self) -> Vec<String> {
        self.bridge.get_tool_names()
    }

    /// Get tool schemas as JSON (for TypeScript type generation).
    #[napi]
    pub fn get_tool_schemas(&self) -> Result<String> {
        self.bridge.get_tool_schemas().map_err(Error::from_reason)
    }

    /// Write a chunk directly to the orchestrator (for simulation/testing).
    #[napi]
    pub fn write_chunk(&self, chunk: String) -> Result<()> {
        self.bridge.write_chunk(chunk);
        Ok(())
    }

    /// Finalize the LLM stream (for simulation/testing).
    #[napi]
    pub fn end_stream(&self) -> Result<String> {
        self.bridge.end_stream().map_err(Error::from_reason)
    }

    /// Process any pending intents (for simulation/testing).
    #[napi]
    pub async fn process_intents(&self) -> Result<String> {
        self.bridge
            .process_intents_async()
            .await
            .map_err(Error::from_reason)
    }

    #[napi]
    pub async fn embed(&self, text: String) -> Result<Vec<f32>> {
        self.bridge.embed(text).await.map_err(Error::from_reason)
    }

    /// Generate embeddings for a batch of texts.
    #[napi]
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.bridge
            .embed_batch(texts)
            .await
            .map_err(Error::from_reason)
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
