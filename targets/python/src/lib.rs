#![deny(clippy::all)]
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use ir_runtime::runtime::bridge::EngineBridge;
use ir_runtime::runtime::engine::IntentControl;

use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════
// AUWGENT — PyO3 FFI class
// ═══════════════════════════════════════════════════════════════════════════

#[pyclass]
pub struct AuwgentNative {
    bridge: EngineBridge,
}

#[pymethods]
impl AuwgentNative {
    #[new]
    pub fn new(ir_json: String) -> PyResult<Self> {
        let bridge = EngineBridge::new(ir_json)
            .map_err(|e| PyValueError::new_err(e))?;
        Ok(Self { bridge })
    }

    pub fn set_gemini_driver(&self, api_key: String) -> PyResult<()> {
        self.bridge.set_gemini_driver(api_key);
        Ok(())
    }

    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) -> PyResult<()> {
        self.bridge.set_openai_driver(api_key, base_url);
        Ok(())
    }

    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) -> PyResult<()> {
        self.bridge.set_custom_driver(id, api_key, base_url);
        Ok(())
    }

    pub fn set_context(&self, context_json: String) -> PyResult<()> {
        let context: Value = serde_json::from_str(&context_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid context json: {}", e)))?;
        self.bridge.set_context(context);
        Ok(())
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.bridge.get_tool_names()
    }

    pub fn get_tool_schemas(&self) -> PyResult<String> {
        self.bridge.get_tool_schemas().map_err(PyRuntimeError::new_err)
    }

    pub fn export_session(&self) -> PyResult<String> {
        self.bridge.export_session().map_err(PyRuntimeError::new_err)
    }

    pub fn import_session(&self, json: String) -> PyResult<()> {
        self.bridge.import_session(json).map_err(PyRuntimeError::new_err)
    }

    pub fn clear_session(&self) -> PyResult<()> {
        self.bridge.clear_session();
        Ok(())
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> PyResult<String> {
        self.bridge.generate_prompt(helper_name).map_err(PyRuntimeError::new_err)
    }

    pub fn write_chunk(&self, chunk: String) -> PyResult<()> {
        self.bridge.write_chunk(chunk);
        Ok(())
    }

    pub fn end_stream(&self) -> PyResult<String> {
        self.bridge.end_stream().map_err(PyRuntimeError::new_err)
    }

    pub fn clear_listeners(&self) -> PyResult<()> {
        self.bridge.clear_listeners();
        Ok(())
    }

    // ==========================================
    // ASYNC AND CALLBACK METHODS
    // ==========================================

    pub fn run<'p>(&self, py: Python<'p>, input: Option<String>, initial_stack_json: Option<String>) -> PyResult<&'p PyAny> {
        let input_val =
            input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));

        let initial_stack: Option<Vec<String>> = initial_stack_json
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok());

        let bridge = self.bridge.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let res: Result<String, String> = bridge.run_async(input_val, initial_stack).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn process_intents<'p>(&self, py: Python<'p>) -> PyResult<&'p PyAny> {
        let bridge = self.bridge.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let res: Result<String, String> = bridge.process_intents_async().await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn embed<'p>(&self, py: Python<'p>, text: String) -> PyResult<&'p PyAny> {
        let bridge = self.bridge.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let res: Result<Vec<f32>, String> = bridge.embed(text).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn embed_batch<'p>(&self, py: Python<'p>, texts: Vec<String>) -> PyResult<&'p PyAny> {
        let bridge = self.bridge.clone();
        pyo3_asyncio::tokio::future_into_py(py, async move {
            let res: Result<Vec<Vec<f32>>, String> = bridge.embed_batch(texts).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn register_tool(&self, name: String, callback: PyObject) -> PyResult<()> {
        let tool_impl: ir_runtime::runtime::engine::ToolImplementation = std::sync::Arc::new(move |args: Value| {
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

        self.bridge.engine.register_tool(&name, tool_impl);

        Ok(())
    }

    pub fn on_intent(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncIntentCallback =
            std::sync::Arc::new(move |name: String, value: Value, agent: String| {
                let callback = callback.clone();
                let value_json = serde_json::to_string(&value).unwrap_or_default();

                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &value_json);
                        let agent_py = pyo3::types::PyString::new(py, &agent);
                        let res = callback.call1(py, (name_py, val_py, agent_py))?;
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

        self.bridge.engine.on_intent(handler);

        Ok(())
    }

    pub fn on_sub_engine_start(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncSessionPreloadCallback =
            std::sync::Arc::new(move |name: String, session: String| {
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

        self.bridge.engine.on_sub_engine_start(handler);
        Ok(())
    }

    pub fn on_sub_engine_complete(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::SessionSaveCallback =
            std::sync::Arc::new(move |name: String, session: String| {
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

        self.bridge.engine.on_sub_engine_complete(handler);
        Ok(())
    }

    pub fn on_llm_start(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncLlmStartCallback =
            std::sync::Arc::new(move |prompt: String, sys: String, ctx: String| {
                let callback = callback.clone();
                Box::pin(async move {
                    let py_result = Python::with_gil(|py| {
                        let p_py = pyo3::types::PyString::new(py, &prompt);
                        let s_py = pyo3::types::PyString::new(py, &sys);
                        let c_py = pyo3::types::PyString::new(py, &ctx);
                        let res = callback.call1(py, (p_py, s_py, c_py))?;
                        pyo3_asyncio::tokio::into_future(res.as_ref(py))
                    });
                    if let Ok(future) = py_result {
                        if let Ok(obj) = future.await {
                            return Python::with_gil(|py| {
                                let s: String = obj.extract(py).unwrap_or_else(|_| "null".to_string());
                                serde_json::from_str(&s).unwrap_or(Value::Null)
                            });
                        }
                    }
                    Value::Null
                })
            });

        self.bridge.engine.on_llm_start(handler);
        Ok(())
    }

    pub fn on_llm_end(&self, callback: PyObject) -> PyResult<()> {
        let handler: ir_runtime::runtime::engine::AsyncLlmEndCallback =
            std::sync::Arc::new(move |res_str: String, sys: String| {
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
        self.bridge.engine.on_llm_end(handler);
        Ok(())
    }

    pub fn on_intent_partial(&self, callback: PyObject) -> PyResult<()> {
        let handler: std::sync::Arc<dyn Fn(String, Value) + Send + Sync> =
            std::sync::Arc::new(move |name: String, value: Value| {
                let callback = callback.clone();
                let value_json = serde_json::to_string(&value).unwrap_or_default();
                Python::with_gil(|py| {
                    let _ = callback.call1(py, (name, value_json));
                });
            });
        self.bridge.engine.on_intent_partial(handler);
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
fn auwgent_sdk(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<AuwgentNative>()?;
    Ok(())
}
