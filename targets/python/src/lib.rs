#![deny(clippy::all)]
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use ir_runtime::runtime::bridge::EngineBridge;
use ir_runtime::runtime::engine::IntentControl;

use serde_json::Value;
use std::sync::Arc;

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

    pub fn set_groq_driver(&self, api_key: String) -> PyResult<()> {
        self.bridge.set_groq_driver(api_key);
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

    pub fn get_metadata(&self) -> PyResult<String> {
        self.bridge.get_metadata().map_err(PyRuntimeError::new_err)
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

    pub fn run<'py>(
        &self,
        py: Python<'py>,
        input: Option<String>,
        initial_stack_json: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let input_val =
            input.map(|s| serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)));

        let initial_stack: Option<Vec<String>> = initial_stack_json
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok());

        let bridge = self.bridge.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let res: Result<String, String> = bridge.run_async(input_val, initial_stack).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn process_intents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let bridge = self.bridge.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let res: Result<String, String> = bridge.process_intents_async().await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn embed<'py>(&self, py: Python<'py>, text: String) -> PyResult<Bound<'py, PyAny>> {
        let bridge = self.bridge.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let res: Result<Vec<f32>, String> = bridge.embed(text).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn embed_batch<'py>(&self, py: Python<'py>, texts: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
        let bridge = self.bridge.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let res: Result<Vec<Vec<f32>>, String> = bridge.embed_batch(texts).await;
            res.map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        })
    }

    pub fn register_tool(&self, name: String, callback: Py<PyAny>) -> PyResult<()> {
        // Wrap in Arc so it can be cheaply cloned across async boundaries
        let callback = Arc::new(callback);

        let tool_impl: ir_runtime::runtime::engine::ToolImplementation =
            std::sync::Arc::new(move |args: Value| {
                let callback = Arc::clone(&callback);
                let args_json = serde_json::to_string(&args).unwrap_or_default();

                Box::pin(async move {
                    let future = Python::attach(|py| {
                        let cb = callback.clone_ref(py);
                        let args_py = pyo3::types::PyString::new(py, &args_json);
                        let res = cb.call1(py, (args_py,))?;
                        res.extract::<String>(py)
                    })
                    .map_err(|e: PyErr| e.to_string())?;

                    let val: Value =
                        serde_json::from_str(&future).map_err(|e| e.to_string())?;
                    Ok(val)
                })
            });

        self.bridge.register_tool(&name, tool_impl);
        Ok(())
    }

    pub fn on_intent(&self, callback: Py<PyAny>) -> PyResult<()> {
        let callback = Arc::new(callback);

        let handler: ir_runtime::runtime::engine::AsyncIntentCallback =
            std::sync::Arc::new(move |name: String, value: Value, agent: String| {
                let callback = Arc::clone(&callback);
                let value_json = serde_json::to_string(&value).unwrap_or_default();

                Box::pin(async move {
                    let py_result = Python::attach(|py| {
                        let cb = callback.clone_ref(py);
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &value_json);
                        let agent_py = pyo3::types::PyString::new(py, &agent);
                        let res = cb.call1(py, (name_py, val_py, agent_py))?;
                        pyo3_async_runtimes::tokio::into_future(res.into_bound(py))
                    });

                    if let Ok(future) = py_result {
                        if let Ok(py_obj) = future.await {
                            let result_str: Option<String> = Python::attach(|py| {
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

        self.bridge.on_intent(handler);
        Ok(())
    }

    pub fn on_sub_engine_start(&self, callback: Py<PyAny>) -> PyResult<()> {
        let callback = Arc::new(callback);

        let handler: ir_runtime::runtime::engine::AsyncSessionPreloadCallback =
            std::sync::Arc::new(move |name: String, session: String| {
                let callback = Arc::clone(&callback);
                Box::pin(async move {
                    let py_result = Python::attach(|py| {
                        let cb = callback.clone_ref(py);
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &session);
                        let res = cb.call1(py, (name_py, val_py))?;
                        pyo3_async_runtimes::tokio::into_future(res.into_bound(py))
                    });
                    if let Ok(future) = py_result {
                        if let Ok(obj) = future.await {
                            return Python::attach(|py| {
                                obj.extract::<Option<String>>(py).unwrap_or(None)
                            });
                        }
                    }
                    None
                })
            });

        self.bridge.on_sub_engine_start(handler);
        Ok(())
    }

    pub fn on_sub_engine_complete(&self, callback: Py<PyAny>) -> PyResult<()> {
        let callback = Arc::new(callback);

        let handler: ir_runtime::runtime::engine::SessionSaveCallback =
            std::sync::Arc::new(move |name: String, session: String| {
                let callback = Arc::clone(&callback);
                Box::pin(async move {
                    let py_result = Python::attach(|py| {
                        let cb = callback.clone_ref(py);
                        let name_py = pyo3::types::PyString::new(py, &name);
                        let val_py = pyo3::types::PyString::new(py, &session);
                        let res = cb.call1(py, (name_py, val_py))?;
                        pyo3_async_runtimes::tokio::into_future(res.into_bound(py))
                    });
                    if let Ok(future) = py_result {
                        let _ = future.await;
                    }
                })
            });

        self.bridge.on_sub_engine_complete(handler);
        Ok(())
    }

    pub fn on_middleware_event(&self, callback: Py<PyAny>) -> PyResult<()> {
        let callback = Arc::new(callback);

        self.bridge
            .on_middleware_event(std::sync::Arc::new(move |event_json: String| {
                let callback = Arc::clone(&callback);
                Box::pin(async move {
                    let py_result = Python::attach(|py| {
                        let cb = callback.clone_ref(py);
                        let event_py = pyo3::types::PyString::new(py, &event_json);
                        let res = cb.call1(py, (event_py,))?;
                        pyo3_async_runtimes::tokio::into_future(res.into_bound(py))
                    });
                    if let Ok(future) = py_result {
                        if let Ok(obj) = future.await {
                            return Python::attach(|py| {
                                obj.extract::<Option<String>>(py).unwrap_or(None)
                            });
                        }
                    }
                    None
                })
            }));
        Ok(())
    }

    pub fn on_intent_partial(&self, callback: Py<PyAny>) -> PyResult<()> {
        let callback = Arc::new(callback);

        let handler: std::sync::Arc<dyn Fn(String, Value, String) + Send + Sync> =
            std::sync::Arc::new(move |name: String, value: Value, agent: String| {
                let callback = Arc::clone(&callback);
                let value_json = serde_json::to_string(&value).unwrap_or_default();
                Python::attach(|py| {
                    let cb = callback.clone_ref(py);
                    let _ = cb.call1(py, (name, value_json, agent));
                });
            });
        self.bridge.on_intent_partial(handler);
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
fn _auwgent_sdk(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AuwgentNative>()?;
    Ok(())
}
