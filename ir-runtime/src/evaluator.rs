use crate::errors::{AuwgentError, AuwgentResult};
use crate::types::{AgentIR, Expression};
use serde_json::Value;
use std::collections::HashMap;

/// Synchronous tool handler for workflow-level function calls.
/// For async tool execution (engine-level tool_call intents), the engine
/// handles dispatch directly via its own ToolImplementation type.
pub type SyncToolFn = std::sync::Arc<dyn Fn(Vec<Value>) -> Result<Value, String> + Send + Sync>;

pub struct Evaluator<'a> {
    pub ir: &'a AgentIR,
    /// Optional tool registry for resolving FunctionCall expressions within workflows.
    tools: HashMap<String, SyncToolFn>,
}

impl<'a> Evaluator<'a> {
    pub fn new(ir: &'a AgentIR) -> Self {
        Self {
            ir,
            tools: HashMap::new(),
        }
    }

    /// Create an evaluator with a pre-populated tool registry.
    pub fn with_tools(ir: &'a AgentIR, tools: HashMap<String, SyncToolFn>) -> Self {
        Self { ir, tools }
    }

    /// Register a synchronous tool function for workflow-level calls.
    pub fn register_tool(&mut self, name: impl Into<String>, handler: SyncToolFn) {
        self.tools.insert(name.into(), handler);
    }

    pub fn evaluate(
        &self,
        expr: &Expression,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        match expr {
            Expression::SchemaDirective { path } => {
                // Find the schema in the agent IR
                let schema = match path.as_str() {
                    "input" => &self.ir.input,
                    "output" => &self.ir.output,
                    "context" => &self.ir.context,
                    _ => {
                        return Err(AuwgentError::Evaluation(format!(
                            "Unknown schema path: {}",
                            path
                        )));
                    }
                };

                if let Some(s) = schema {
                    Ok(Value::String(crate::schema::format_schema(
                        s,
                        self.ir.types.as_ref(),
                    )))
                } else {
                    Ok(Value::String("{}".to_string()))
                }
            }
            Expression::Literal { value } => Ok(value.clone()),
            Expression::VarRef { value } => scope
                .get(value)
                .cloned()
                .ok_or_else(|| AuwgentError::VariableNotFound(value.clone())),
            Expression::MemberAccess { object, properties } => {
                // 1. Evaluate the base object (e.g., "input")
                let mut current = self.evaluate(object, scope)?;

                // 2. Traverse the properties (e.g., ".name")
                for prop in properties {
                    match current {
                        Value::Object(map) => {
                            if let Some(val) = map.get(prop) {
                                current = val.clone();
                            } else {
                                return Err(AuwgentError::PropertyNotFound {
                                    property: prop.clone(),
                                    context: "object".to_string(),
                                });
                            }
                        }
                        _ => {
                            return Err(AuwgentError::PropertyNotFound {
                                property: prop.clone(),
                                context: "non-object value".to_string(),
                            });
                        }
                    }
                }
                Ok(current)
            }
            Expression::Template { value } | Expression::Parts { value } => {
                // Evaluate all parts first
                let mut results = Vec::new();
                let mut all_strings = true;

                for part in value {
                    let val = self.evaluate(part, scope)?;
                    if !val.is_string() {
                        all_strings = false;
                    }
                    results.push((part, val));
                }

                // If all parts are strings, join them with smart whitespace handling
                if all_strings {
                    let joined = self.join_and_dedent(results);

                    // Only trim full result if it was a "Parts" expression (top-level prompt)
                    if matches!(expr, Expression::Parts { .. }) {
                        Ok(Value::String(joined.trim().to_string()))
                    } else {
                        Ok(Value::String(joined))
                    }
                } else {
                    // Otherwise return array of values
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::Object { value } => {
                let mut map = serde_json::Map::new();
                for (k, v) in value {
                    let evaluated_val = self.evaluate(v, scope)?;
                    map.insert(k.clone(), evaluated_val);
                }
                Ok(Value::Object(map))
            }

            Expression::InlineIf {
                condition,
                then,
                else_block,
            } => {
                // 1. Evaluate left and right sides
                let left = self.evaluate(&condition.left, scope)?;
                let right = self.evaluate(&condition.right, scope)?;

                // 2. Perform comparison
                let is_true = self.compare_values(&left, &right, &condition.operator)?;

                // 3. Choose which block to execute
                let block = if is_true { then } else { else_block };

                // 4. Evaluate the chosen block (similar to Parts/Template)
                let mut results = Vec::new();
                for part in block {
                    results.push((part, self.evaluate(part, scope)?));
                }

                // Join if strings (same logic as Template)
                if results.iter().all(|(_, v)| v.is_string()) {
                    let joined = self.join_and_dedent(results);
                    Ok(Value::String(joined.trim().to_string()))
                } else {
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::ContextRef { property } => {
                if let Some(Value::Object(ctx)) = scope.get("context") {
                    ctx.get(property)
                        .cloned()
                        .ok_or_else(|| AuwgentError::PropertyNotFound {
                            property: property.clone(),
                            context: "context".to_string(),
                        })
                } else {
                    Err(AuwgentError::Evaluation(
                        "Context object not found in scope".to_string(),
                    ))
                }
            }

            Expression::If {
                condition,
                then,
                else_block,
            } => {
                // Condition is an enum (Comparison or Boolean), so we need to extract it
                let (left_expr, right_expr, op) = match condition {
                    crate::types::Condition::Comparison(cmp) => {
                        (&cmp.left, &cmp.right, &cmp.operator)
                    }
                    crate::types::Condition::Boolean { value } => {
                        // For now, treat boolean as "value == true"
                        (
                            value,
                            &Box::new(Expression::Literal {
                                value: Value::Bool(true),
                            }),
                            &"==".to_string(),
                        )
                    }
                };

                // 1. Evaluate left and right sides
                let left = self.evaluate(left_expr, scope)?;
                let right = self.evaluate(right_expr, scope)?;

                // 2. Reuse comparison logic
                let is_true = self.compare_values(&left, &right, op)?;

                // 3. Execute block
                let block = if is_true { then } else { else_block };

                // Simple block execution
                let mut last_result = Value::Null;
                for stmt in block {
                    last_result = self.evaluate(stmt, scope)?;
                }
                Ok(last_result)
            }

            Expression::Return { value } => self.evaluate(value, scope),

            Expression::PromptRef {
                params,
                args,
                value,
                ..
            } => {
                // 1. Create a local scope by cloning the current one and adding parameters
                // Evaluate args in current scope first
                let mut evaluated_args = Vec::new();
                for arg_expr in args {
                    evaluated_args.push(self.evaluate(arg_expr, scope)?);
                }

                let mut local_scope = scope.clone();
                for (param_name, val) in params.iter().zip(evaluated_args.into_iter()) {
                    local_scope.insert(param_name.clone(), val);
                }

                // 2. Evaluate the prompt block with the local scope
                let mut results = Vec::new();
                for part in value {
                    results.push((part, self.evaluate(part, &mut local_scope)?));
                }

                // 3. Join results if all are strings
                if results.iter().all(|(_, v)| v.is_string()) {
                    let joined = self.join_and_dedent(results);
                    Ok(Value::String(joined.trim().to_string()))
                } else {
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate(left, scope)?;
                let right_val = self.evaluate(right, scope)?;
                if op == "+" {
                    let l_str = left_val
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| left_val.to_string());
                    let r_str = right_val
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| right_val.to_string());

                    let l_trimmed = l_str.trim_end();
                    // Preserve explicit structure of right side (e.g. if it starts with newlines)
                    // Only add a separator if the right side doesn't already have one.
                    let separator = if r_str.starts_with('\n') || r_str.starts_with("\r\n") {
                        ""
                    } else {
                        "\n"
                    };

                    Ok(Value::String(format!(
                        "{}{}{}",
                        l_trimmed, separator, r_str
                    )))
                } else {
                    Err(AuwgentError::UnsupportedOperator(op.clone()))
                }
            }

            Expression::InlinePrompt { parts } => {
                let mut results = Vec::new();
                for part in parts {
                    results.push((part, self.evaluate(part, scope)?));
                }

                if results.iter().all(|(_, v)| v.is_string()) {
                    let joined = self.join_and_dedent(results);
                    Ok(Value::String(joined))
                } else {
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::PromptExamples { examples } => {
                let mut formatted = String::from("\n\n# Example\n");
                for (i, example) in examples.iter().enumerate() {
                    formatted.push_str(&format!(
                        "User: {}\nAssistant: {}\n",
                        example.user, example.assistant
                    ));
                    // Add newline between examples, but not after the last one if we want to be strict,
                    // though a trailing newline is usually fine in prompts.
                    if i < examples.len() - 1 {
                        formatted.push('\n');
                    }
                }
                // Ensure it ends with a newline to separate from following text
                formatted.push('\n');
                Ok(Value::String(formatted))
            }

            Expression::VariableDeclaration { name, value } => {
                let val = self.evaluate(value, scope)?;
                scope.insert(name.clone(), val);
                Ok(Value::Null)
            }

            Expression::FunctionCall {
                value: func_name,
                args,
            } => {
                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(self.evaluate(arg, scope)?);
                }

                // Look up tool in registry
                if let Some(handler) = self.tools.get(func_name.as_str()) {
                    handler(arg_values).map_err(|e| AuwgentError::ToolExecution {
                        tool_name: func_name.clone(),
                        message: e,
                    })
                } else {
                    Err(AuwgentError::UnknownFunction(func_name.clone()))
                }
            }

            Expression::HelperCall {
                value: helper_name,
                args,
            } => {
                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(self.evaluate(arg, scope)?);
                }

                // Validate helper exists
                if !self.ir.helpers.iter().any(|h| h.name == *helper_name) {
                    return Err(AuwgentError::UnknownHelper(helper_name.clone()));
                }

                // Actually running a helper from inside the sync evaluator is tricky because
                // execute_sub_agent is async. Right now, evaluator.rs is completely synchronous.
                // We will return a structured JSON to the Engine to handle this.
                Ok(serde_json::json!({
                    "__requires_async_helper_call": true,
                    "helper_name": helper_name,
                    "args": arg_values
                }))
            }

            Expression::Transfer { target, mode } => {
                let target_val = self.evaluate(target, scope)?;
                let target_str = target_val.as_str().unwrap_or("").to_string();

                // Similar to HelperCall, we return a structured payload for the engine to await.
                Ok(serde_json::json!({
                    "__requires_async_transfer": true,
                    "target": target_str,
                    "mode": mode
                }))
            }

            Expression::Parallel { body } => {
                println!("Executing Parallel Block ({} tasks)...", body.len());
                let mut results = Vec::new();
                // TODO: Implement actual parallel execution (requires async/threading)
                for expr in body {
                    results.push(self.evaluate(expr, scope)?);
                }
                Ok(Value::Array(results))
            }

            Expression::Expression { value } => self.evaluate(value, scope),
        }
    }

    pub fn evaluate_model(
        &self,
        config: &crate::types::ModelConfig,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        self.evaluate_provider(&config.model, scope)
    }

    pub fn evaluate_provider(
        &self,
        provider: &crate::types::ModelProvider,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        match provider {
            crate::types::ModelProvider::Gemini { model_name, config } => {
                let mut res = serde_json::Map::new();
                res.insert("provider".to_string(), Value::String("gemini".to_string()));
                res.insert(
                    "url".to_string(),
                    Value::String("https://generativelanguage.googleapis.com/v1beta".to_string()),
                );
                res.insert("modelName".to_string(), Value::String(model_name.clone()));

                if let Some(expr) = config {
                    let evaluated_config = self.evaluate(expr, scope)?;
                    res.insert("config".to_string(), evaluated_config);
                }

                Ok(Value::Object(res))
            }
            crate::types::ModelProvider::OpenAI { model_name, config } => {
                let mut res = serde_json::Map::new();
                res.insert("provider".to_string(), Value::String("openai".to_string()));
                res.insert(
                    "url".to_string(),
                    Value::String("https://api.openai.com/v1".to_string()),
                );
                res.insert("modelName".to_string(), Value::String(model_name.clone()));

                if let Some(expr) = config {
                    let evaluated_config = self.evaluate(expr, scope)?;
                    res.insert("config".to_string(), evaluated_config);
                }

                Ok(Value::Object(res))
            }
            crate::types::ModelProvider::Custom {
                id,
                url,
                model_name,
                config,
            } => {
                let mut res = serde_json::Map::new();
                res.insert("provider".to_string(), Value::String("custom".to_string()));
                res.insert("id".to_string(), Value::String(id.clone()));
                res.insert("url".to_string(), Value::String(url.clone()));
                res.insert("modelName".to_string(), Value::String(model_name.clone()));

                if let Some(expr) = config {
                    let evaluated_config = self.evaluate(expr, scope)?;
                    res.insert("config".to_string(), evaluated_config);
                }

                Ok(Value::Object(res))
            }
            crate::types::ModelProvider::ModelRef { name } => {
                let mut res = serde_json::Map::new();
                res.insert("ref".to_string(), Value::String(name.clone()));
                Ok(Value::Object(res))
            }
        }
    }

    /// Reusable comparison helper to eliminate duplicated match logic.
    fn compare_values(&self, left: &Value, right: &Value, op: &str) -> AuwgentResult<bool> {
        match op {
            "==" => Ok(left == right),
            "!=" => Ok(left != right),
            ">" => Ok(left
                .as_f64()
                .zip(right.as_f64())
                .map_or(false, |(l, r)| l > r)),
            "<" => Ok(left
                .as_f64()
                .zip(right.as_f64())
                .map_or(false, |(l, r)| l < r)),
            ">=" => Ok(left
                .as_f64()
                .zip(right.as_f64())
                .map_or(false, |(l, r)| l >= r)),
            "<=" => Ok(left
                .as_f64()
                .zip(right.as_f64())
                .map_or(false, |(l, r)| l <= r)),
            _ => Err(AuwgentError::UnsupportedOperator(op.to_string())),
        }
    }

    // Helper method for smart dedent logic
    fn join_and_dedent(&self, parts: Vec<(&Expression, Value)>) -> String {
        let mut joined = String::new();
        for (expr, val) in parts {
            let s = val.as_str().unwrap_or("");

            // Smart Dedent Logic:
            // If we are about to append a block result (InlineIf, If, or Schema),
            // and the buffer ends with whitespace (indentation), trim it.
            if matches!(
                expr,
                Expression::InlineIf { .. }
                    | Expression::If { .. }
                    | Expression::SchemaDirective { .. }
            ) {
                let trimmed = joined.trim_end_matches(|c| c == ' ' || c == '\t');
                let len = trimmed.len();
                joined.truncate(len);
            }

            // Indentation Fix: If this is a literal string part (from template text),
            // it might carry accumulated indentation from the source file.
            if matches!(expr, Expression::Literal { .. }) {
                // If it's a literal, clean it up with dedent
                // but only if it looks like a multiline block or starts with newline
                if s.contains('\n') {
                    let dedented = self.dedent(s);
                    // If the previous part ended with newline, we should probably trim start of this one.
                    if joined.ends_with('\n') {
                        // If we are appending to a newline, we want to ensure we don't double up or leave weird gaps.
                        // dedent() returns a clean block.
                        joined.push_str(&dedented);
                    } else {
                        // If we are appending to existing text (inline), we might not want to dedent the *first* line
                        // if it matters, but usually for prompt templates, consistent left-alignment is prefered.
                        joined.push_str(&dedented);
                    }
                } else {
                    joined.push_str(s);
                }
            } else {
                joined.push_str(s);
            }
        }
        joined
    }

    fn dedent(&self, s: &str) -> String {
        // Normalize tabs to 4 spaces to handle mixed indentation
        let s_expanded = s.replace('\t', "    ");
        let lines: Vec<&str> = s_expanded.lines().collect();

        if lines.is_empty() {
            return String::new();
        }

        // 1. Calculate common indentation from the second line onwards
        let mut min_indent = usize::MAX;

        // If there's only one line, we just trim it?
        // Or do we treat it as having 0 indent if we follow cleandoc?
        // Let's just calculate from all lines if only 1, or just trim start.
        if lines.len() == 1 {
            return lines[0].trim_start().to_string();
        }

        for (i, line) in lines.iter().enumerate() {
            // Skip the first line for indentation calculation
            if i == 0 {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }

            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            if indent < min_indent {
                min_indent = indent;
            }
        }

        if min_indent == usize::MAX {
            min_indent = 0;
        }

        // 2. Strip indentation
        let mut dedented = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                dedented.push('\n');
            }

            if i == 0 {
                // For the first line, we just trim leading whitespace
                dedented.push_str(line.trim_start());
            } else {
                if line.len() >= min_indent {
                    dedented.push_str(&line[min_indent..]);
                } else {
                    dedented.push_str(line.trim_start()); // Fallback for shorter lines (empty)
                }
            }
        }

        // Preserve trailing newline if original had it
        if s.ends_with('\n') && !dedented.ends_with('\n') {
            dedented.push('\n');
        }

        dedented
    }
}
