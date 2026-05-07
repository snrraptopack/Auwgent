use crate::runtime::AuwgentEngine;
use crate::runtime::drivers::ModelDriver;
use crate::runtime::drivers::gemini::GeminiDriver;
use crate::runtime::drivers::openai::OpenAIDriver;
use crate::runtime::engine::{
    AsyncIntentCallback, AsyncMiddlewareEventCallback, AsyncSessionPreloadCallback,
    PartialIntentCallback, SessionSaveCallback, ToolImplementation,
};
use crate::types::AgentIR;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// EngineBridge provides a language-agnostic facade for the Auwgent engine.
/// It encapsulates the Tokio runtime and engine state, reducing duplication
/// across FFI layers (Node.js, Python, etc.).
#[derive(Clone)]
pub struct EngineBridge {
    pub engine: Arc<AuwgentEngine>,
    pub ir: Arc<AgentIR>,
    pub rt: Arc<tokio::runtime::Runtime>,
}

impl EngineBridge {
    pub fn generate_prompt_from_ir(
        ir_json: String,
        context: Option<Value>,
        helper_name: Option<String>,
    ) -> Result<String, String> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| format!("Failed to parse IR JSON: {}", e))?;
        let engine = AuwgentEngine::new(ir);
        if let Some(context) = context {
            engine.set_context(context);
        }
        engine
            .generate_prompt(helper_name)
            .map_err(|e| format!("{}", e))
    }

    pub fn new(ir_json: String) -> Result<Self, String> {
        let timing = TimingProbe::new("rust.bridge.new");
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| format!("Failed to parse IR JSON: {}", e))?;
        timing.mark("parsed ir");

        let engine = Arc::new(AuwgentEngine::new(ir.clone()));
        timing.mark("constructed engine");

        let rt = shared_runtime()?.clone();
        timing.mark("loaded shared runtime");

        Ok(Self {
            engine,
            ir: Arc::new(ir),
            rt,
        })
    }

    pub fn new_current_thread(ir_json: String) -> Result<Self, String> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| format!("Failed to parse IR JSON: {}", e))?;

        Self::with_runtime(ir, tokio::runtime::Builder::new_current_thread())
    }

    fn with_runtime(ir: AgentIR, mut builder: tokio::runtime::Builder) -> Result<Self, String> {
        let engine = AuwgentEngine::new(ir.clone());

        let rt = builder
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            engine: Arc::new(engine),
            ir: Arc::new(ir),
            rt: Arc::new(rt),
        })
    }

    pub fn set_gemini_driver(&self, api_key: String) {
        self.engine.register_driver(
            "gemini",
            Arc::new(GeminiDriver::new(api_key)) as Arc<dyn ModelDriver>,
        );
    }

    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) {
        self.engine.register_driver(
            "openai",
            Arc::new(OpenAIDriver::new(api_key, base_url)) as Arc<dyn ModelDriver>,
        );
    }

    pub fn set_groq_driver(&self, api_key: String) {
        self.engine.register_driver(
            "groq",
            Arc::new(OpenAIDriver::new(
                api_key,
                Some("https://api.groq.com/openai/v1".to_string()),
            )) as Arc<dyn ModelDriver>,
        );
    }

    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) {
        self.engine.register_driver(
            &id,
            Arc::new(OpenAIDriver::new(api_key, Some(base_url))) as Arc<dyn ModelDriver>,
        );
    }

    pub fn register_driver(&self, provider_type: String, driver: Arc<dyn ModelDriver>) {
        self.engine.register_driver(&provider_type, driver);
    }

    pub fn set_context(&self, context: Value) {
        self.engine.set_context(context);
    }

    pub fn register_tool(&self, name: &str, implementation: ToolImplementation) {
        self.engine.register_tool(name, implementation);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        self.engine.on_intent(handler);
    }

    pub fn on_intent_partial(&self, handler: PartialIntentCallback) {
        self.engine.on_intent_partial(handler);
    }

    pub fn on_sub_engine_start(&self, handler: AsyncSessionPreloadCallback) {
        self.engine.on_sub_engine_start(handler);
    }

    pub fn on_sub_engine_complete(&self, handler: SessionSaveCallback) {
        self.engine.on_sub_engine_complete(handler);
    }

    pub fn on_middleware_event(&self, handler: AsyncMiddlewareEventCallback) {
        self.engine.on_middleware_event(handler);
    }

    pub fn get_metadata(&self) -> Result<String, String> {
        let meta = self.engine.last_run_metadata.lock().unwrap();
        serde_json::to_string(&*meta).map_err(|e| format!("{}", e))
    }

    pub fn export_session(&self) -> Result<String, String> {
        self.engine.export_session().map_err(|e| format!("{}", e))
    }

    pub fn import_session(&self, json: String) -> Result<(), String> {
        self.engine
            .import_session(&json)
            .map_err(|e| format!("{}", e))
    }

    pub fn clear_session(&self) {
        self.engine.clear_session();
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> Result<String, String> {
        self.engine
            .generate_prompt(helper_name)
            .map_err(|e| format!("{}", e))
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.ir.tools.iter().map(|t| t.name.clone()).collect()
    }

    pub fn get_tool_schemas(&self) -> Result<String, String> {
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
        serde_json::to_string(&schemas).map_err(|e| format!("{}", e))
    }

    pub fn write_chunk(&self, chunk: String) {
        self.engine.write_llm_chunk(&chunk);
    }

    pub fn end_stream(&self) -> Result<String, String> {
        let val = self.engine.end_llm_stream();
        serde_json::to_string(&val).map_err(|e| format!("{}", e))
    }

    /// Drain structured output events as newline-delimited JSON.
    pub fn drain_structured_output_jsonl(&self) -> String {
        self.engine.drain_structured_output_jsonl_text()
    }

    /// Drain structured output events as a JSON array of JSONL lines.
    pub fn drain_structured_output_jsonl_lines(&self) -> Result<String, String> {
        serde_json::to_string(&self.engine.drain_structured_output_jsonl())
            .map_err(|e| format!("{}", e))
    }

    pub fn clear_listeners(&self) {
        self.engine.clear_intent_handlers();
        self.engine.clear_sub_engine_handlers();
        self.engine.clear_middleware_handler();
    }

    pub async fn run_async(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> Result<String, String> {
        let timing = TimingProbe::new("rust.bridge.run_async");
        self.engine
            .run(input, initial_stack)
            .await
            .map_err(|e| format!("{}", e))?;
        timing.mark("engine run complete");
        self.engine.export_session().map_err(|e| format!("{}", e))
    }

    pub async fn begin_run_async(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> Result<String, String> {
        let session = self
            .engine
            .begin_manual_run(input, initial_stack)
            .await
            .map_err(|e| format!("{}", e))?;
        serde_json::to_string(&session).map_err(|e| format!("{}", e))
    }

    pub async fn apply_llm_start_async(&self, prompt: String) -> Result<String, String> {
        let prompt = self
            .engine
            .apply_manual_llm_start(prompt)
            .await
            .map_err(|e| format!("{}", e))?;
        serde_json::to_string(&serde_json::json!({ "prompt": prompt }))
            .map_err(|e| format!("{}", e))
    }

    pub async fn apply_llm_end_async(&self, response: Value) -> Result<String, String> {
        self.engine
            .apply_manual_llm_end(response)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(serde_json::json!({ "ok": true }).to_string())
    }

    pub async fn complete_run_async(&self) -> Result<String, String> {
        let session = self
            .engine
            .complete_manual_run()
            .await
            .map_err(|e| format!("{}", e))?;
        serde_json::to_string(&session).map_err(|e| format!("{}", e))
    }

    pub async fn apply_error_async(
        &self,
        error: Value,
        include_session: bool,
    ) -> Result<String, String> {
        let swallowed = self
            .engine
            .apply_manual_error(error, include_session)
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(serde_json::json!({ "swallowed": swallowed }).to_string())
    }

    pub async fn process_intents_async(&self) -> Result<String, String> {
        let (terminal, actions, hard_stop) = self
            .engine
            .process_intents()
            .await
            .map_err(|e| format!("{}", e))?;
        Ok(serde_json::json!({
            "terminal": terminal,
            "actions": actions,
            "hard_stop": hard_stop,
        })
        .to_string())
    }

    pub async fn embed(&self, text: String) -> Result<Vec<f32>, String> {
        self.engine.embed(&text).await.map_err(|e| format!("{}", e))
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        self.engine
            .embed_batch(&texts)
            .await
            .map_err(|e| format!("{}", e))
    }
}

fn shared_runtime() -> Result<&'static Arc<tokio::runtime::Runtime>, String> {
    static SHARED_RUNTIME: OnceLock<Result<Arc<tokio::runtime::Runtime>, String>> = OnceLock::new();

    SHARED_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map(Arc::new)
                .map_err(|e| format!("Failed to create tokio runtime: {}", e))
        })
        .as_ref()
        .map_err(|err| err.clone())
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> u128 {
    js_sys::Date::now() as u128
}

struct TimingProbe {
    label: &'static str,
    start_ms: u128,
    enabled: bool,
}

impl TimingProbe {
    fn new(label: &'static str) -> Self {
        let enabled = timing_enabled();
        let probe = Self {
            label,
            start_ms: current_time_ms(),
            enabled,
        };
        if enabled {
            eprintln!("[auwgent][timing][rust] {} +0ms start", label);
        }
        probe
    }

    fn mark(&self, message: &str) {
        if self.enabled {
            let elapsed = current_time_ms().saturating_sub(self.start_ms);
            eprintln!(
                "[auwgent][timing][rust] {} +{}ms {}",
                self.label,
                elapsed,
                message
            );
        }
    }
}

fn timing_enabled() -> bool {
    std::env::var("AUWGENT_DEBUG_TIMING")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}
