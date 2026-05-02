#![cfg(target_arch = "wasm32")]

use futures_util::FutureExt;
use ir_runtime::runtime::drivers::ModelDriver;
use ir_runtime::runtime::drivers::gemini::GeminiDriver;
use ir_runtime::runtime::drivers::openai::OpenAIDriver;
use ir_runtime::runtime::engine::{AuwgentEngine, IntentControl, ToolImplementation};
use ir_runtime::types::AgentIR;
use js_sys::{Array, Function, Promise};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, future_to_promise};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(message: &str);
}

thread_local! {
    static NEXT_CALLBACK_ID: Cell<u32> = const { Cell::new(1) };
    static CALLBACKS: RefCell<HashMap<u32, Function>> = RefCell::new(HashMap::new());
}

#[wasm_bindgen]
pub struct AuwgentWasm {
    engine: Arc<AuwgentEngine>,
    ir: Arc<AgentIR>,
}

#[wasm_bindgen]
impl AuwgentWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(ir_json: String) -> Result<AuwgentWasm, JsValue> {
        install_panic_hook();
        let ir: AgentIR = serde_json::from_str(&ir_json).map_err(js_error)?;
        Ok(Self {
            engine: Arc::new(AuwgentEngine::new(ir.clone())),
            ir: Arc::new(ir),
        })
    }

    #[wasm_bindgen(js_name = setContext)]
    pub fn set_context(&self, context: JsValue) -> Result<(), JsValue> {
        self.engine.set_context(from_js_value(context)?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setOpenaiDriver)]
    pub fn set_openai_driver(&self, api_key: String) {
        self.engine.register_driver(
            "openai",
            Arc::new(OpenAIDriver::new(api_key, None)) as Arc<dyn ModelDriver>,
        );
    }

    #[wasm_bindgen(js_name = setGroqDriver)]
    pub fn set_groq_driver(&self, api_key: String) {
        self.engine.register_driver(
            "groq",
            Arc::new(OpenAIDriver::new(
                api_key,
                Some("https://api.groq.com/openai/v1".to_string()),
            )) as Arc<dyn ModelDriver>,
        );
    }

    #[wasm_bindgen(js_name = setCustomDriver)]
    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) {
        self.engine.register_driver(
            &id,
            Arc::new(OpenAIDriver::new(api_key, Some(base_url))) as Arc<dyn ModelDriver>,
        );
    }

    #[wasm_bindgen(js_name = setGeminiDriver)]
    pub fn set_gemini_driver(&self, api_key: String) {
        self.engine.register_driver(
            "gemini",
            Arc::new(GeminiDriver::new(api_key)) as Arc<dyn ModelDriver>,
        );
    }

    #[wasm_bindgen(js_name = registerTool)]
    pub fn register_tool(&self, name: String, callback: Function) {
        let callback_id = register_callback(callback);
        let implementation: ToolImplementation = Arc::new(move |args: Value| {
            async move {
                let callback = get_callback(callback_id).map_err(js_value_to_string)?;
                let args = to_js_value(&args).map_err(js_value_to_string)?;
                let result = callback
                    .call1(&JsValue::NULL, &args)
                    .map_err(js_value_to_string)?;
                let resolved = JsFuture::from(Promise::resolve(&result))
                    .await
                    .map_err(js_value_to_string)?;
                from_js_value(resolved).map_err(js_value_to_string)
            }
            .boxed_local()
        });
        self.engine.register_tool(&name, implementation);
    }

    #[wasm_bindgen(js_name = onIntent)]
    pub fn on_intent(&self, callback: Function) {
        let callback_id = register_callback(callback);
        self.engine.on_intent(Arc::new(move |name, value, agent| {
            async move {
                let callback = match get_callback(callback_id) {
                    Ok(callback) => callback,
                    Err(_) => return None,
                };
                let value = match to_js_value(&value) {
                    Ok(value) => value,
                    Err(_) => return None,
                };
                let result = callback
                    .call3(
                        &JsValue::NULL,
                        &JsValue::from_str(&name),
                        &value,
                        &JsValue::from_str(&agent),
                    )
                    .ok()?;
                let resolved = JsFuture::from(Promise::resolve(&result)).await.ok()?;
                parse_intent_control(resolved)
            }
            .boxed_local()
        }));
    }

    #[wasm_bindgen(js_name = onIntentPartial)]
    pub fn on_intent_partial(&self, callback: Function) {
        let callback_id = register_callback(callback);
        self.engine.on_intent_partial(Arc::new(move |name, value, agent| {
            let Ok(callback) = get_callback(callback_id) else {
                return;
            };
            if let Ok(value) = to_js_value(&value) {
                let _ = callback.call3(
                    &JsValue::NULL,
                    &JsValue::from_str(&name),
                    &value,
                    &JsValue::from_str(&agent),
                );
            }
        }));
    }

    #[wasm_bindgen(js_name = onSubEngineStart)]
    pub fn on_sub_engine_start(&self, callback: Function) {
        let callback_id = register_callback(callback);
        self.engine.on_sub_engine_start(Arc::new(move |name, empty_session| {
            async move {
                let callback = get_callback(callback_id).ok()?;
                let result = callback
                    .call2(
                        &JsValue::NULL,
                        &JsValue::from_str(&name),
                        &JsValue::from_str(&empty_session),
                    )
                    .ok()?;
                let resolved = JsFuture::from(Promise::resolve(&result)).await.ok()?;
                resolved.as_string()
            }
            .boxed_local()
        }));
    }

    #[wasm_bindgen(js_name = onSubEngineComplete)]
    pub fn on_sub_engine_complete(&self, callback: Function) {
        let callback_id = register_callback(callback);
        self.engine.on_sub_engine_complete(Arc::new(move |name, completed_session| {
            async move {
                let Ok(callback) = get_callback(callback_id) else {
                    return;
                };
                if let Ok(result) = callback.call2(
                    &JsValue::NULL,
                    &JsValue::from_str(&name),
                    &JsValue::from_str(&completed_session),
                ) {
                    let _ = JsFuture::from(Promise::resolve(&result)).await;
                }
            }
            .boxed_local()
        }));
    }

    #[wasm_bindgen(js_name = onMiddlewareEvent)]
    pub fn on_middleware_event(&self, callback: Function) {
        let callback_id = register_callback(callback);
        self.engine.on_middleware_event(Arc::new(move |event_json| {
            async move {
                let callback = get_callback(callback_id).ok()?;
                let result = callback
                    .call1(&JsValue::NULL, &JsValue::from_str(&event_json))
                    .ok()?;
                let resolved = JsFuture::from(Promise::resolve(&result)).await.ok()?;
                resolved.as_string()
            }
            .boxed_local()
        }));
    }

    #[wasm_bindgen(js_name = clearListeners)]
    pub fn clear_listeners(&self) {
        self.engine.clear_intent_handlers();
        self.engine.clear_sub_engine_handlers();
        self.engine.clear_middleware_handler();
    }

    #[wasm_bindgen(js_name = run)]
    pub fn run(&self, input: Option<String>, initial_stack_json: Option<String>) -> Promise {
        let engine = Arc::clone(&self.engine);
        future_to_promise(async move {
            let input = input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));
            let initial_stack =
                initial_stack_json.and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok());
            engine
                .run(input, initial_stack)
                .await
                .map_err(js_error)?;
            let session = engine.export_session().map_err(js_error)?;
            Ok(JsValue::from_str(&session))
        })
    }

    #[wasm_bindgen(js_name = exportSession)]
    pub fn export_session(&self) -> Result<String, JsValue> {
        self.engine.export_session().map_err(js_error)
    }

    #[wasm_bindgen(js_name = importSession)]
    pub fn import_session(&self, json: String) -> Result<(), JsValue> {
        self.engine.import_session(&json).map_err(js_error)
    }

    #[wasm_bindgen(js_name = clearSession)]
    pub fn clear_session(&self) {
        self.engine.clear_session();
    }

    #[wasm_bindgen(js_name = getMetadata)]
    pub fn get_metadata(&self) -> Result<String, JsValue> {
        let meta = self.engine.last_run_metadata.lock().unwrap();
        serde_json::to_string(&*meta).map_err(js_error)
    }

    #[wasm_bindgen(js_name = generatePrompt)]
    pub fn generate_prompt(&self, helper_name: Option<String>) -> Result<String, JsValue> {
        self.engine.generate_prompt(helper_name).map_err(js_error)
    }

    #[wasm_bindgen(js_name = getToolNames)]
    pub fn get_tool_names(&self) -> Array {
        self.ir
            .tools
            .iter()
            .map(|tool| JsValue::from_str(&tool.name))
            .collect()
    }

    #[wasm_bindgen(js_name = getToolSchemas)]
    pub fn get_tool_schemas(&self) -> Result<String, JsValue> {
        let schemas: Vec<Value> = self
            .ir
            .tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "params": tool.params,
                    "returns": tool.returns,
                })
            })
            .collect();
        serde_json::to_string(&schemas).map_err(js_error)
    }

    #[wasm_bindgen(js_name = writeChunk)]
    pub fn write_chunk(&self, chunk: String) {
        self.engine.write_llm_chunk(&chunk);
    }

    #[wasm_bindgen(js_name = endStream)]
    pub fn end_stream(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.engine.end_llm_stream()).map_err(js_error)
    }

    #[wasm_bindgen(js_name = processIntents)]
    pub fn process_intents(&self) -> Promise {
        let engine = Arc::clone(&self.engine);
        future_to_promise(async move {
            let (terminal, actions, hard_stop) = engine.process_intents().await.map_err(js_error)?;
            Ok(JsValue::from_str(
                &serde_json::json!({
                    "terminal": terminal,
                    "actions": actions,
                    "hard_stop": hard_stop,
                })
                .to_string(),
            ))
        })
    }

    #[wasm_bindgen(js_name = embed)]
    pub fn embed(&self, text: String) -> Promise {
        let engine = Arc::clone(&self.engine);
        future_to_promise(async move {
            let values = engine.embed(&text).await.map_err(js_error)?;
            to_js_value(&values)
        })
    }

    #[wasm_bindgen(js_name = embedBatch)]
    pub fn embed_batch(&self, texts: JsValue) -> Promise {
        let engine = Arc::clone(&self.engine);
        future_to_promise(async move {
            let texts: Vec<String> = from_js_value(texts)?;
            let values = engine.embed_batch(&texts).await.map_err(js_error)?;
            to_js_value(&values)
        })
    }
}

fn parse_intent_control(value: JsValue) -> Option<IntentControl> {
    if value.is_null() || value.is_undefined() {
        return None;
    }
    let value: Value = from_js_value(value).ok()?;
    match value {
        Value::Object(obj) => {
            if obj.get("skip").and_then(Value::as_bool) == Some(true) {
                Some(IntentControl::Skip)
            } else {
                obj.get("result")
                    .cloned()
                    .map(|result| IntentControl::Override { result })
            }
        }
        _ => None,
    }
}

fn register_callback(callback: Function) -> u32 {
    let id = NEXT_CALLBACK_ID.with(|next| {
        let id = next.get();
        next.set(id.saturating_add(1));
        id
    });
    CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().insert(id, callback);
    });
    id
}

fn install_panic_hook() {
    static INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INSTALLED.get_or_init(|| {
        std::panic::set_hook(Box::new(|info| {
            error(&format!("Auwgent WASM panic: {info}"));
        }));
    });
}

fn get_callback(id: u32) -> Result<Function, JsValue> {
    CALLBACKS.with(|callbacks| {
        callbacks
            .borrow()
            .get(&id)
            .cloned()
            .ok_or_else(|| JsValue::from_str("JavaScript callback was not registered"))
    })
}

fn from_js_value<T>(value: JsValue) -> Result<T, JsValue>
where
    T: serde::de::DeserializeOwned,
{
    serde_wasm_bindgen::from_value(value).map_err(js_error)
}

fn to_js_value<T>(value: &T) -> Result<JsValue, JsValue>
where
    T: serde::Serialize,
{
    value
        .serialize(
            &serde_wasm_bindgen::Serializer::new()
                .serialize_maps_as_objects(true)
                .serialize_missing_as_null(true),
        )
        .map_err(js_error)
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn js_value_to_string(value: JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "JavaScript callback failed".to_string())
}
