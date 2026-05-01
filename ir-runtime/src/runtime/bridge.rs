use crate::runtime::AuwgentEngine;
use crate::runtime::drivers::ModelDriver;
use crate::runtime::drivers::gemini::GeminiDriver;
use crate::runtime::drivers::openai::OpenAIDriver;
use crate::runtime::engine::{
    AsyncIntentCallback, AsyncMiddlewareEventCallback, AsyncSessionPreloadCallback,
    SessionSaveCallback, ToolImplementation,
};
use crate::types::AgentIR;
use serde_json::Value;
use std::sync::{Arc, OnceLock};

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
    pub fn new(ir_json: String) -> Result<Self, String> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| format!("Failed to parse IR JSON: {}", e))?;

        Ok(Self {
            engine: Arc::new(AuwgentEngine::new(ir.clone())),
            ir: Arc::new(ir),
            rt: shared_runtime()?.clone(),
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

    pub fn set_context(&self, context: Value) {
        self.engine.set_context(context);
    }

    pub fn register_tool(&self, name: &str, implementation: ToolImplementation) {
        self.engine.register_tool(name, implementation);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        self.engine.on_intent(handler);
    }

    pub fn on_intent_partial(&self, handler: Arc<dyn Fn(String, Value, String) + Send + Sync>) {
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
    }

    pub async fn run_async(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> Result<String, String> {
        self.engine
            .run(input, initial_stack)
            .await
            .map_err(|e| format!("{}", e))?;
        self.engine.export_session().map_err(|e| format!("{}", e))
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
