// Workflow execution.
// This file evaluates workflow bodies, prepares sync tool adapters for the
// evaluator, and bridges workflow-directed helper transfers when needed.
use super::*;

impl AuwgentEngine {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) async fn execute_workflow(
        &self,
        call: &Value,
    ) -> AuwgentResult<(String, Value, Value)> {
        let wf_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let body_clone = {
            let wf = match self.ir.workflows.iter().find(|w| w.name == wf_name) {
                Some(w) => w,
                None => {
                    return Ok((
                        wf_name.clone(),
                        args.clone(),
                        serde_json::json!({ "error": format!("Workflow not found: {}", wf_name) }),
                    ));
                }
            };
            wf.body.clone()
        };

        let mut tool_fns: HashMap<String, crate::evaluator::SyncToolFn> = HashMap::new();
        {
            let tools = self.tools.lock().unwrap();
            let ir_tools = &self.ir.tools;

            for (name, imp) in &*tools {
                let imp = imp.clone();
                let name_clone = name.clone();

                let param_names: Vec<String> = ir_tools
                    .iter()
                    .find(|t| t.name == *name)
                    .and_then(|t| t.params.0.as_object())
                    .map(|params: &serde_json::Map<String, Value>| {
                        let mut names: Vec<_> = params.keys().cloned().collect();
                        names.sort();
                        names
                    })
                    .unwrap_or_default();

                tool_fns.insert(
                    name.clone(),
                    std::sync::Arc::new(move |fn_args: Vec<Value>| {
                        let arg_val = if param_names.is_empty() {
                            if fn_args.len() == 1 {
                                fn_args.into_iter().next().unwrap_or(Value::Null)
                            } else {
                                Value::Array(fn_args)
                            }
                        } else {
                            let mut args_obj: serde_json::Map<String, Value> =
                                serde_json::Map::new();
                            for (i, param_name) in param_names.iter().enumerate() {
                                if let Some(arg_value) = fn_args.get(i) {
                                    args_obj.insert(param_name.clone(), arg_value.clone());
                                }
                            }
                            Value::Object(args_obj)
                        };

                        let rt = tokio::runtime::Handle::current();
                        let imp = imp.clone();
                        std::thread::spawn(move || rt.block_on(imp(arg_val)))
                            .join()
                            .map_err(|_| format!("Tool '{}' panicked", name_clone))?
                    }),
                );
            }
        }

        let ir_clone = self.ir.clone();

        let mut scope = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                scope.insert(k.clone(), v.clone());
            }
        }
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let mut last_result = Value::Null;
        for stmt in &body_clone {
            let eval_result = {
                let evaluator = Evaluator::with_tools(&ir_clone, tool_fns.clone());
                evaluator.evaluate(stmt, &mut scope)?
            };

            if let Some(obj) = eval_result.as_object() {
                if obj
                    .get("__requires_async_helper_call")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj
                        .get("helper_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let helper_args = obj.get("args").unwrap_or(&Value::Null).clone();

                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": helper_args
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;
                    last_result = sub_result;
                    continue;
                }

                if obj
                    .get("__requires_async_transfer")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": {}
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;

                    if let Some(res_obj) = sub_result.as_object()
                        && res_obj
                            .get("__handoff_stop")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        return Ok((wf_name, args, sub_result));
                    }

                    last_result = sub_result;
                    continue;
                }
            }

            last_result = eval_result;
        }

        Ok((wf_name, args, last_result))
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn execute_workflow(
        &self,
        call: &Value,
    ) -> AuwgentResult<(String, Value, Value)> {
        let wf_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let body_clone = {
            let wf = match self.ir.workflows.iter().find(|w| w.name == wf_name) {
                Some(w) => w,
                None => {
                    return Ok((
                        wf_name.clone(),
                        args.clone(),
                        serde_json::json!({ "error": format!("Workflow not found: {}", wf_name) }),
                    ));
                }
            };
            wf.body.clone()
        };

        let mut scope = HashMap::new();
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                scope.insert(k.clone(), v.clone());
            }
        }
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let mut last_result = Value::Null;
        for stmt in &body_clone {
            last_result = self.evaluate_workflow_expr_wasm(stmt, &mut scope).await?;

            if let Some(obj) = last_result.as_object() {
                if obj
                    .get("__requires_async_helper_call")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj
                        .get("helper_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let helper_args = obj.get("args").unwrap_or(&Value::Null).clone();

                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": helper_args
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;
                    last_result = sub_result;
                    continue;
                }

                if obj
                    .get("__requires_async_transfer")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    let target_helper = obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    let helper_call = serde_json::json!({
                        "type": target_helper,
                        "args": {}
                    });
                    let (_, _, sub_result) = self.execute_helper(&helper_call).await?;

                    if let Some(res_obj) = sub_result.as_object()
                        && res_obj
                            .get("__handoff_stop")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        return Ok((wf_name, args, sub_result));
                    }

                    last_result = sub_result;
                }
            }
        }

        Ok((wf_name, args, last_result))
    }

    #[cfg(target_arch = "wasm32")]
    fn evaluate_workflow_expr_wasm<'a>(
        &'a self,
        expr: &'a Expression,
        scope: &'a mut HashMap<String, Value>,
    ) -> futures_util::future::LocalBoxFuture<'a, AuwgentResult<Value>> {
        use futures_util::FutureExt;

        async move {
            match expr {
                Expression::VariableDeclaration { name, value } => {
                    let val = self.evaluate_workflow_expr_wasm(value, scope).await?;
                    scope.insert(name.clone(), val);
                    Ok(Value::Null)
                }
                Expression::FunctionCall {
                    value: func_name,
                    args,
                } => {
                    let mut arg_values = Vec::new();
                    for arg in args {
                        arg_values.push(self.evaluate_workflow_expr_wasm(arg, scope).await?);
                    }
                    let arg_val = self.workflow_tool_args(func_name, arg_values);
                    let tool = self
                        .tools
                        .lock()
                        .unwrap()
                        .get(func_name)
                        .cloned()
                        .ok_or_else(|| AuwgentError::UnknownFunction(func_name.clone()))?;

                    tool(arg_val).await.map_err(|message| AuwgentError::ToolExecution {
                        tool_name: func_name.clone(),
                        message,
                    })
                }
                Expression::Return { value }
                | Expression::Expression { value } => {
                    self.evaluate_workflow_expr_wasm(value, scope).await
                }
                _ => {
                    let evaluator = Evaluator::new(&self.ir);
                    evaluator.evaluate(expr, scope)
                }
            }
        }
        .boxed_local()
    }

    #[cfg(target_arch = "wasm32")]
    fn workflow_tool_args(&self, tool_name: &str, fn_args: Vec<Value>) -> Value {
        let param_names: Vec<String> = self
            .ir
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .and_then(|t| t.params.0.as_object())
            .map(|params: &serde_json::Map<String, Value>| {
                let mut names: Vec<_> = params.keys().cloned().collect();
                names.sort();
                names
            })
            .unwrap_or_default();

        if param_names.is_empty() {
            if fn_args.len() == 1 {
                fn_args.into_iter().next().unwrap_or(Value::Null)
            } else {
                Value::Array(fn_args)
            }
        } else {
            let mut args_obj = serde_json::Map::new();
            for (i, param_name) in param_names.iter().enumerate() {
                if let Some(arg_value) = fn_args.get(i) {
                    args_obj.insert(param_name.clone(), arg_value.clone());
                }
            }
            Value::Object(args_obj)
        }
    }
}
