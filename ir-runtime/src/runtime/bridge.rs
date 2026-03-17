use crate::runtime::AuwgentEngine;
use crate::runtime::drivers::gemini::GeminiDriver;
use crate::runtime::drivers::openai::OpenAIDriver;
use crate::runtime::drivers::ModelDriver;
use crate::types::AgentIR;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// EngineBridge provides a language-agnostic facade for the Auwgent engine.
/// It encapsulates the Tokio runtime and engine state, reducing duplication
/// across FFI layers (Node.js, Python, etc.).
pub struct EngineBridge {
    pub engine: Arc<Mutex<AuwgentEngine>>,
    pub ir: Arc<AgentIR>,
    pub rt: Arc<tokio::runtime::Runtime>,
}

impl EngineBridge {
    pub fn new(ir_json: String) -> Result<Self, String> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| format!("Failed to parse IR JSON: {}", e))?;

        let engine = AuwgentEngine::new(ir.clone());

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            ir: Arc::new(ir),
            rt: Arc::new(rt),
        })
    }

    pub fn set_gemini_driver(&self, api_key: String) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_driver(
                "gemini",
                Arc::new(GeminiDriver::new(api_key)) as Arc<dyn ModelDriver>,
            );
        });
    }

    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_driver(
                "openai",
                Arc::new(OpenAIDriver::new(api_key, base_url)) as Arc<dyn ModelDriver>,
            );
        });
    }

    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_driver(
                &id,
                Arc::new(OpenAIDriver::new(api_key, Some(base_url))) as Arc<dyn ModelDriver>,
            );
        });
    }

    pub fn set_context(&self, context: Value) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.set_context(context);
        });
    }

    pub fn export_session(&self) -> Result<String, String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let eng = engine.lock().await;
            eng.export_session().map_err(|e| format!("{}", e))
        })
    }

    pub fn import_session(&self, json: String) -> Result<(), String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.import_session(&json).map_err(|e| format!("{}", e))
        })
    }

    pub fn clear_session(&self) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.clear_session();
        });
    }

    pub fn generate_prompt(&self) -> Result<String, String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let eng = engine.lock().await;
            eng.generate_prompt().map_err(|e| format!("{}", e))
        })
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
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.write_llm_chunk(&chunk);
        });
    }

    pub fn end_stream(&self) -> Result<String, String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            let val = eng.end_llm_stream();
            serde_json::to_string(&val).map_err(|e| format!("{}", e))
        })
    }

    pub fn clear_listeners(&self) {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.clear_intent_handlers();
            eng.clear_sub_engine_handlers();
            eng.clear_llm_handlers();
        });
    }

    pub async fn run_async(&self, input: Option<Value>, initial_stack: Option<Vec<String>>) -> Result<String, String> {
        let engine = self.engine.clone();
        let mut eng = engine.lock().await;
        eng.run(input, initial_stack).await.map_err(|e| format!("{}", e))?;
        eng.export_session().map_err(|e| format!("{}", e))
    }

    pub async fn process_intents_async(&self) -> Result<String, String> {
        let engine = self.engine.clone();
        let mut eng = engine.lock().await;
        let (terminal, actions, hard_stop) =
            eng.process_intents().await.map_err(|e| format!("{}", e))?;
        Ok(serde_json::json!({
            "terminal": terminal,
            "actions": actions,
            "hard_stop": hard_stop,
        })
        .to_string())
    }
}
