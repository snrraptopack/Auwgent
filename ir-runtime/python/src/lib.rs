#![deny(clippy::all)]

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

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
// AUWGENT — PyO3 FFI class
// ═══════════════════════════════════════════════════════════════════════════

#[pyclass]
pub struct AuwgentNative {
    engine: Arc<Mutex<AuwgentEngine>>,
    ir: Arc<AgentIR>,
    rt: Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl AuwgentNative {
    #[new]
    pub fn new(ir_json: String) -> PyResult<Self> {
        let ir: AgentIR = serde_json::from_str(&ir_json)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse IR JSON: {}", e)))?;

        let engine = AuwgentEngine::new(ir.clone());

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create tokio runtime: {}", e))
            })?;

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            ir: Arc::new(ir),
            rt: Arc::new(rt),
        })
    }

    pub fn set_gemini_driver(&self, api_key: String) -> PyResult<()> {
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

    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) -> PyResult<()> {
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

    pub fn set_context(&self, context_json: String) -> PyResult<()> {
        let context: Value = serde_json::from_str(&context_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid context json: {}", e)))?;

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.set_context(context);
        });
        Ok(())
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.ir.tools.iter().map(|t| t.name.clone()).collect()
    }

    pub fn get_tool_schemas(&self) -> PyResult<String> {
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
        serde_json::to_string(&schemas).map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
    }

    pub fn export_session(&self) -> PyResult<String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let eng = engine.lock().await;
            eng.export_session()
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn import_session(&self, json: String) -> PyResult<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.import_session(&json)
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn clear_session(&self) -> PyResult<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.clear_session();
        });
        Ok(())
    }

    pub fn generate_prompt(&self) -> PyResult<String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let eng = engine.lock().await;
            eng.generate_prompt()
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn write_chunk(&self, chunk: String) -> PyResult<()> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.write_llm_chunk(&chunk);
        });
        Ok(())
    }

    pub fn end_stream(&self) -> PyResult<String> {
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            let val = eng.end_llm_stream();
            serde_json::to_string(&val).map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    // ==========================================
    // ASYNC AND CALLBACK METHODS
    // ==========================================

    pub fn run<'p>(&self, py: Python<'p>, input: Option<String>) -> PyResult<&'p PyAny> {
        let engine = self.engine.clone();
        let input_val =
            input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));

        pyo3_asyncio::tokio::future_into_py(py, async move {
            let mut eng = engine.lock().await;
            let res = eng
                .run(input_val)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
            let export = eng
                .export_session()
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
            Ok(export)
        })
    }

    pub fn process_intents<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        let engine = self.engine.clone();

        pyo3_asyncio::tokio::future_into_py(py, async move {
            let mut eng = engine.lock().await;
            let (terminal, actions, hard_stop) = eng
                .process_intents()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
            let json_res = serde_json::json!({
                "terminal": terminal,
                "actions": actions,
                "hard_stop": hard_stop,
            })
            .to_string();
            Ok(json_res)
        })
    }

    pub fn register_tool(&self, name: String, callback: PyObject) -> PyResult<()> {
        let tool_impl: ToolImplementation = Arc::new(move |args: Value| {
            let callback = callback.clone();
            let args_json = serde_json::to_string(&args).unwrap_or_default();

            Box::pin(async move {
                let future = Python::with_gil(|py| {
                    let args_py = pyo3::types::PyString::new(py, &args_json);
                    let res = callback.call1(py, (args_py,))?;
                    pyo3_asyncio::tokio::into_future(res.as_ref(py))
                })
                .map_err(|e| e.to_string())?;

                let awaited_result = future.await.map_err(|e| e.to_string())?;

                let result_str: String =
                    Python::with_gil(|py| awaited_result.extract::<String>(py))
                        .map_err(|e| e.to_string())?;

                let val: Value = serde_json::from_str(&result_str).map_err(|e| e.to_string())?;
                Ok(val)
            })
        });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.register_tool(&name, tool_impl);
        });

        Ok(())
    }

    pub fn on_intent(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncIntentCallback =
            Arc::new(move |name: String, value: Value| {
                let callback = callback.clone();
                let value_json = serde_json::to_string(&value).unwrap_or_default();

                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &value_json);
                        let res = callback.call1(py, (name_py, val_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });

                    if let Ok(future) = py_result {
                        if let Ok(py_obj) = future.await {
                            let result_str: Option<String> = Python::with_gil(|py| {
                                py_obj.extract::<Option<String>>(py).unwrap_or(None)
                            });
                            if let Some(s) = result_str {
                                if let Ok(val) = serde_json::from_str::<Value>(&s) {
                                    return parse_intent_control(&val);
                                }
                            }
                        }
                    }
                    None
                })
            });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_intent(handler);
        });

        Ok(())
    }

    pub fn on_sub_engine_start(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncSessionPreloadCallback =
            Arc::new(move |name: String, session: String| {
                let callback = callback.clone();
                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &session);
                        let res = callback.call1(py, (name_py, val_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });
                    if let Ok(future) = py_result {
                        if let Ok(obj) = future.await {
                            return Python::with_gil(|py| {
                                obj.extract::<Option<String>>(py).unwrap_or(None)
                            });
                        }
                    }
                    None
                })
            });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_sub_engine_start(handler);
        });
        Ok(())
    }

    pub fn on_sub_engine_complete(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::SessionSaveCallback =
            Arc::new(move |name: String, session: String| {
                let callback = callback.clone();
                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &session);
                        let res = callback.call1(py, (name_py, val_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });
                    if let Ok(future) = py_result {
                        let _ = future.await;
                    }
                })
            });

        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_sub_engine_complete(handler);
        });
        Ok(())
    }

    pub fn on_llm_start(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncLlmStartCallback =
            Arc::new(move |prompt: String, sys: String| {
                let callback = callback.clone();
                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let p_py = pyo3::types::PyString::new(py, &prompt);
                        let s_py = pyo3::types::PyString::new(py, &sys);
                        let res = callback.call1(py, (p_py, s_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });
                    if let Ok(future) = py_result {
                        if let Ok(obj) = future.await {
                            return Python::with_gil(|py| {
                                obj.extract::<Option<String>>(py).unwrap_or(None)
                            });
                        }
                    }
                    None
                })
            });
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_llm_start(handler);
        });
        Ok(())
    }

    pub fn on_llm_end(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncLlmEndCallback =
            Arc::new(move |res_str: String, sys: String| {
                let callback = callback.clone();
                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let r_py = pyo3::types::PyString::new(py, &res_str);
                        let s_py = pyo3::types::PyString::new(py, &sys);
                        let res = callback.call1(py, (r_py, s_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });
                    if let Ok(future) = py_result {
                        let _ = future.await;
                    }
                })
            });
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_llm_end(handler);
        });
        Ok(())
    }

    pub fn on_intent_partial(&self, callback: PyObject) -> PyResult<()> {
        let handler: Arc<dyn Fn(String, Value) + Send + Sync> =
            Arc::new(move |name: String, value: Value| {
                let callback = callback.clone();
                let value_json = serde_json::to_string(&value).unwrap_or_default();
                Python::with_gil(|py| {
                    let _ = callback.call1(py, (name, value_json));
                });
            });
        let engine = self.engine.clone();
        self.rt.block_on(async {
            let mut eng = engine.lock().await;
            eng.on_intent_partial(handler);
        });
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

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

#[pymodule]
fn auwgent_native(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<AuwgentNative>()?;
    Ok(())
}
