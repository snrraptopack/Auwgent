use crate::errors::{AuwgentError, AuwgentResult};
use crate::types::{AgentIR, Expression};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// Synchronous tool handler for workflow-level function calls.
/// For async tool execution (engine-level tool_call intents), the engine
/// handles dispatch directly via its own ToolImplementation type.
pub type SyncToolFn = std::sync::Arc<dyn Fn(Vec<Value>) -> Result<Value, String> + Send + Sync>;

pub struct Evaluator<'a> {
    pub ir: &'a AgentIR,
    /// Optional tool registry for resolving FunctionCall expressions within workflows.
    tools: HashMap<String, SyncToolFn>,
    context_usage: RefCell<HashSet<String>>,
    context_tracking_suppressed: Cell<usize>,
}

impl<'a> Evaluator<'a> {
    pub fn new(ir: &'a AgentIR) -> Self {
        Self {
            ir,
            tools: HashMap::new(),
            context_usage: RefCell::new(HashSet::new()),
            context_tracking_suppressed: Cell::new(0),
        }
    }

    /// Create an evaluator with a pre-populated tool registry.
    pub fn with_tools(ir: &'a AgentIR, tools: HashMap<String, SyncToolFn>) -> Self {
        Self {
            ir,
            tools,
            context_usage: RefCell::new(HashSet::new()),
            context_tracking_suppressed: Cell::new(0),
        }
    }

    /// Register a synchronous tool function for workflow-level calls.
    pub fn register_tool(&mut self, name: impl Into<String>, handler: SyncToolFn) {
        self.tools.insert(name.into(), handler);
    }

    pub fn clear_context_usage(&self) {
        self.context_usage.borrow_mut().clear();
    }

    pub fn context_usage(&self) -> HashSet<String> {
        self.context_usage.borrow().clone()
    }

    pub fn collect_context_references(
        &self,
        expr: &Expression,
        scope: &HashMap<String, Value>,
    ) -> HashSet<String> {
        let mut refs = HashSet::new();
        self.collect_context_references_from_expr(expr, scope, &mut refs);
        refs
    }

    fn record_context_property(&self, property: &str) {
        if self.context_tracking_suppressed.get() > 0 {
            return;
        }

        self.context_usage.borrow_mut().insert(property.to_string());
    }

    fn with_context_tracking_suppressed<T>(
        &self,
        f: impl FnOnce() -> AuwgentResult<T>,
    ) -> AuwgentResult<T> {
        self.context_tracking_suppressed
            .set(self.context_tracking_suppressed.get() + 1);
        let result = f();
        self.context_tracking_suppressed
            .set(self.context_tracking_suppressed.get() - 1);
        result
    }

    fn record_all_context_properties(&self, scope: &HashMap<String, Value>) {
        if let Some(Value::Object(ctx)) = scope.get("context") {
            for key in ctx.keys() {
                self.record_context_property(key);
            }
        }
    }

    fn extract_context_member_access<'b>(&self, expr: &'b Expression) -> Option<&'b [String]> {
        if let Expression::MemberAccess { object, properties } = expr {
            if let Expression::VarRef { value } = object.as_ref() {
                if value == "context" || value == "ctx" {
                    return Some(properties.as_slice());
                }
            }
        }

        None
    }

    fn collect_context_references_from_expr(
        &self,
        expr: &Expression,
        scope: &HashMap<String, Value>,
        refs: &mut HashSet<String>,
    ) {
        match expr {
            Expression::ContextRef { property } => {
                refs.insert(property.clone());
            }
            Expression::MemberAccess { object, properties } => {
                if let Expression::VarRef { value } = object.as_ref() {
                    if (value == "context" || value == "ctx") && !properties.is_empty() {
                        refs.insert(properties[0].clone());
                    }
                }
                self.collect_context_references_from_expr(object, scope, refs);
            }
            Expression::IndexAccess { object, index } => {
                if object == "context" || object == "ctx" {
                    if let Expression::Literal { value } = index.as_ref() {
                        if let Some(key) = value.0.as_str() {
                            refs.insert(key.to_string());
                        }
                    }
                }
                self.collect_context_references_from_expr(index, scope, refs);
            }
            Expression::Parts { value }
            | Expression::Array { value }
            | Expression::Parallel { body: value } => {
                for part in value {
                    self.collect_context_references_from_expr(part, scope, refs);
                }
            }
            Expression::Template { value } | Expression::InlinePrompt { parts: value } => {
                for part in value {
                    if let Ok(parsed) = serde_json::from_value::<Expression>(part.0.clone()) {
                        self.collect_context_references_from_expr(&parsed, scope, refs);
                    }
                }
            }
            Expression::Object { value } => {
                for expr in value.values() {
                    self.collect_context_references_from_expr(expr, scope, refs);
                }
            }
            Expression::InlineIf {
                condition,
                then,
                else_block,
            }
            | Expression::If {
                condition,
                then,
                else_block,
            } => {
                if let Ok(cond) =
                    serde_json::from_value::<crate::types::Condition>(condition.0.clone())
                {
                    self.collect_context_references_from_condition(&cond, scope, refs);
                }
                for expr in then {
                    self.collect_context_references_from_expr(expr, scope, refs);
                }
                for expr in else_block {
                    self.collect_context_references_from_expr(expr, scope, refs);
                }
            }
            Expression::PromptRef { args, value, .. } => {
                for arg in args {
                    self.collect_context_references_from_expr(arg, scope, refs);
                }
                for part in value {
                    if let Ok(parsed) = serde_json::from_value::<Expression>(part.0.clone()) {
                        self.collect_context_references_from_expr(&parsed, scope, refs);
                    }
                }
            }
            Expression::Return { value }
            | Expression::Expression { value }
            | Expression::Transfer { target: value, .. } => {
                self.collect_context_references_from_expr(value, scope, refs);
            }
            Expression::VariableDeclaration { value, .. } => {
                self.collect_context_references_from_expr(value, scope, refs);
            }
            Expression::BinaryOp { left, right, .. } => {
                self.collect_context_references_from_expr(left, scope, refs);
                self.collect_context_references_from_expr(right, scope, refs);
            }
            Expression::FunctionCall { args, .. }
            | Expression::HelperCall { args, .. }
            | Expression::PromptCall { args, .. } => {
                for arg in args {
                    self.collect_context_references_from_expr(arg, scope, refs);
                }
            }
            Expression::SchemaDirective { path } => {
                if path == "context" {
                    if let Some(Value::Object(ctx)) = scope.get("context") {
                        refs.extend(ctx.keys().cloned());
                    }
                }
            }
            Expression::Literal { .. }
            | Expression::VarRef { .. }
            | Expression::PromptExamples { .. } => {}
            _ => {}
        }
    }

    fn collect_context_references_from_condition(
        &self,
        cond: &crate::types::Condition,
        scope: &HashMap<String, Value>,
        refs: &mut HashSet<String>,
    ) {
        match cond {
            crate::types::Condition::Comparison(cmp) => {
                self.collect_context_references_from_expr(&cmp.left, scope, refs);
                self.collect_context_references_from_expr(&cmp.right, scope, refs);
            }
            crate::types::Condition::Boolean { value } => {
                self.collect_context_references_from_expr(value, scope, refs);
            }
            crate::types::Condition::ContextRef { property } => {
                refs.insert(property.clone());
            }
            crate::types::Condition::Logical { left, right, .. } => {
                self.collect_context_references_from_condition(left, scope, refs);
                self.collect_context_references_from_condition(right, scope, refs);
            }
        }
    }

    pub fn evaluate(
        &self,
        expr: &Expression,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        match expr {
            Expression::SchemaDirective { path } => match path.as_str() {
                "input" => {
                    if let Some(schema) = &self.ir.input {
                        Ok(Value::String(crate::schema::format_schema(
                            &schema.0,
                            self.ir.types.as_ref(),
                        )))
                    } else {
                        Ok(Value::String("{}".to_string()))
                    }
                }
                "output" => {
                    if let Some(schema) = &self.ir.output {
                        Ok(Value::String(crate::schema::format_output_schema_blocks(
                            &schema.0,
                            self.ir.types.as_ref(),
                        )))
                    } else {
                        Ok(Value::String("{}".to_string()))
                    }
                }
                "context" => {
                    self.record_all_context_properties(scope);
                    if let Some(schema) = &self.ir.context {
                        Ok(Value::String(crate::schema::format_schema(
                            &schema.0,
                            self.ir.types.as_ref(),
                        )))
                    } else {
                        Ok(Value::String("{}".to_string()))
                    }
                }
                _ => Err(AuwgentError::Evaluation(format!(
                    "Unknown schema path: {}",
                    path
                ))),
            },
            Expression::Literal { value } => Ok(value.0.clone()),
            Expression::VarRef { value } => scope
                .get(value)
                .cloned()
                .ok_or_else(|| AuwgentError::VariableNotFound(value.clone())),
            Expression::MemberAccess { object, properties } => {
                let context_label = if let Some(props) = self.extract_context_member_access(expr) {
                    if let Some(first) = props.first() {
                        self.record_context_property(first);
                    }
                    "context"
                } else {
                    "object"
                };
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
                                    context: context_label.to_string(),
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
            Expression::Template { value } => {
                // Evaluate all parts first
                let mut results = Vec::new();

                let mut parsed_exprs = Vec::new();
                for part in value {
                    let parsed: Expression = serde_json::from_value(part.0.clone()).unwrap();
                    parsed_exprs.push(parsed);
                }

                for parsed in &parsed_exprs {
                    let val = self.evaluate(parsed, scope)?;
                    results.push((parsed, val));
                }

                let joined = self.join_and_dedent(results);
                Ok(Value::String(joined))
            }
            Expression::Parts { value } => {
                let mut results = Vec::new();

                for part in value {
                    let val = self.evaluate(part, scope)?;
                    results.push((part, val));
                }

                let joined = self.join_and_dedent(results);
                Ok(Value::String(joined.trim().to_string()))
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
                // 1. Evaluate condition
                let parsed_cond: crate::types::Condition =
                    serde_json::from_value(condition.0.clone()).map_err(|e| {
                        AuwgentError::Evaluation(format!("Condition parse error: {}", e))
                    })?;
                let is_true = self.with_context_tracking_suppressed(|| {
                    self.evaluate_condition(&parsed_cond, scope)
                })?;

                // 2. Choose which block to execute
                let block = if is_true { then } else { else_block };

                // 3. Evaluate the chosen block (similar to Parts/Template)
                let mut results = Vec::new();
                for part in block {
                    results.push((part, self.evaluate(part, scope)?));
                }

                if results.is_empty() {
                    return Ok(Value::Null);
                }

                let joined = self.join_and_dedent(results);
                Ok(Value::String(joined.trim().to_string()))
            }

            Expression::ContextRef { property } => {
                self.record_context_property(property);
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
                // 1. Evaluate condition
                let parsed_cond: crate::types::Condition =
                    serde_json::from_value(condition.0.clone()).map_err(|e| {
                        AuwgentError::Evaluation(format!("Condition parse error: {}", e))
                    })?;
                let is_true = self.with_context_tracking_suppressed(|| {
                    self.evaluate_condition(&parsed_cond, scope)
                })?;

                // 2. Choose which block to execute
                let block = if is_true { then } else { else_block };

                // 3. Simple block execution
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
                    let parsed: Expression = serde_json::from_value(part.0.clone()).unwrap();
                    let evaluated = self.evaluate(&parsed, &mut local_scope)?;

                    // Here we don't have a direct reference to parsed easily for join_and_dedent
                    // because join_and_dedent takes &Expression. We can just use an empty literal as dummy
                    results.push(evaluated);
                }

                let mut s = String::new();
                for v in results {
                    s.push_str(&self.value_to_prompt_string(&v));
                }
                Ok(Value::String(s.trim().to_string()))
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
                    let parsed: Expression = serde_json::from_value(part.0.clone()).unwrap();
                    let evaluated = self.evaluate(&parsed, scope)?;
                    results.push(evaluated);
                }

                if results.iter().all(|v| v.is_string()) {
                    let mut s = String::new();
                    for v in results {
                        s.push_str(v.as_str().unwrap());
                    }
                    Ok(Value::String(s))
                } else {
                    Ok(Value::Array(results.into_iter().collect()))
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
            Expression::Array { value } => {
                let mut results = Vec::new();
                for expr in value {
                    results.push(self.evaluate(expr, scope)?);
                }
                Ok(Value::Array(results))
            }
            Expression::IndexAccess { object, index } => {
                let obj_val = scope
                    .get(object)
                    .cloned()
                    .ok_or_else(|| AuwgentError::VariableNotFound(object.clone()))?;
                let idx_val = self.evaluate(index, scope)?;
                if let Value::Array(arr) = obj_val {
                    if let Some(idx) = idx_val.as_u64() {
                        if let Some(val) = arr.get(idx as usize) {
                            return Ok(val.clone());
                        }
                    }
                }
                Err(AuwgentError::Evaluation("Invalid index access".to_string()))
            }
            _ => Err(AuwgentError::Evaluation(format!(
                "Unsupported expression evaluator node: {:?}",
                expr
            ))),
        }
    }

    fn evaluate_model_config_expr(
        &self,
        expr: &Expression,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        match self.evaluate(expr, scope) {
            Ok(value) => Ok(value),
            Err(AuwgentError::VariableNotFound(_)) => self.evaluate_json_like_expr(expr, scope),
            Err(err) => Err(err),
        }
    }

    fn evaluate_json_like_expr(
        &self,
        expr: &Expression,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<Value> {
        match expr {
            Expression::VarRef { value } => Ok(Value::String(value.clone())),
            Expression::Object { value } => {
                let mut map = serde_json::Map::new();
                for (key, child) in value {
                    map.insert(key.clone(), self.evaluate_json_like_expr(child, scope)?);
                }
                Ok(Value::Object(map))
            }
            Expression::Array { value } => {
                let mut items = Vec::with_capacity(value.len());
                for child in value {
                    items.push(self.evaluate_json_like_expr(child, scope)?);
                }
                Ok(Value::Array(items))
            }
            _ => self.evaluate(expr, scope),
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
                    let parsed: Expression = serde_json::from_value(expr.0.clone()).unwrap();
                    let evaluated_config = self.evaluate_model_config_expr(&parsed, scope)?;
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
                    let parsed: Expression = serde_json::from_value(expr.0.clone()).unwrap();
                    let evaluated_config = self.evaluate_model_config_expr(&parsed, scope)?;
                    res.insert("config".to_string(), evaluated_config);
                }

                Ok(Value::Object(res))
            }
            crate::types::ModelProvider::Groq { model_name, config } => {
                let mut res = serde_json::Map::new();
                res.insert("provider".to_string(), Value::String("groq".to_string()));
                res.insert(
                    "url".to_string(),
                    Value::String("https://api.groq.com/openai/v1".to_string()),
                );
                res.insert("modelName".to_string(), Value::String(model_name.clone()));

                if let Some(expr) = config {
                    let parsed: Expression = serde_json::from_value(expr.0.clone()).unwrap();
                    let evaluated_config = self.evaluate_model_config_expr(&parsed, scope)?;
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
                    let parsed: Expression = serde_json::from_value(expr.0.clone()).unwrap();
                    let evaluated_config = self.evaluate_model_config_expr(&parsed, scope)?;
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

    pub fn evaluate_condition(
        &self,
        cond: &crate::types::Condition,
        scope: &mut HashMap<String, Value>,
    ) -> AuwgentResult<bool> {
        match cond {
            crate::types::Condition::Comparison(cmp) => {
                let left = self.evaluate(&cmp.left, scope)?;
                let right = self.evaluate(&cmp.right, scope)?;
                self.compare_values(&left, &right, &cmp.operator)
            }
            crate::types::Condition::Boolean { value } => {
                if let Some(props) = self.extract_context_member_access(value) {
                    if let Some(first) = props.first() {
                        self.record_context_property(first);
                    }
                }
                let val = self.evaluate(value, scope)?;
                Ok(self.is_truthy(&val))
            }
            crate::types::Condition::ContextRef { property } => {
                self.record_context_property(property);
                if let Some(Value::Object(ctx)) = scope.get("context") {
                    let val = ctx.get(property).unwrap_or(&Value::Null);
                    Ok(self.is_truthy(val))
                } else {
                    Ok(false)
                }
            }
            crate::types::Condition::Logical { op, left, right } => {
                let l = self.evaluate_condition(left, scope)?;
                if op == "&&" || op == "and" {
                    if !l {
                        return Ok(false);
                    }
                    self.evaluate_condition(right, scope)
                } else if op == "||" || op == "or" {
                    if l {
                        return Ok(true);
                    }
                    self.evaluate_condition(right, scope)
                } else {
                    Err(AuwgentError::Evaluation(format!(
                        "Unsupported logical op: {}",
                        op
                    )))
                }
            }
        }
    }

    fn is_truthy(&self, val: &Value) -> bool {
        match val {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Number(n) => n.as_f64().unwrap_or(1.0) != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
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
            let s = self.value_to_prompt_string(&val);

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
                    let dedented = self.dedent(&s);
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
                    joined.push_str(&s);
                }
            } else {
                joined.push_str(&s);
            }
        }
        joined
    }

    fn value_to_prompt_string(&self, value: &Value) -> String {
        match value {
            Value::Null => String::new(),
            Value::String(text) => text.clone(),
            _ => value.to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_config_allows_json_like_bare_identifiers() {
        let ir: AgentIR = serde_json::from_value(json!({
            "name": "Hello",
            "modelConfig": [],
            "input": null,
            "output": null,
            "context": null,
            "tools": [],
            "workflows": [],
            "helpers": [],
            "components": [],
            "tests": []
        }))
        .unwrap();
        let evaluator = Evaluator::new(&ir);
        let expr: Expression = serde_json::from_value(json!({
            "type": "object",
            "value": {
                "somefield": {
                    "type": "object",
                    "value": {
                        "another": {
                            "type": "varRef",
                            "value": "value"
                        }
                    }
                }
            }
        }))
        .unwrap();

        let mut scope = HashMap::new();
        let result = evaluator
            .evaluate_model_config_expr(&expr, &mut scope)
            .unwrap();

        assert_eq!(result, json!({ "somefield": { "another": "value" } }));
    }
}
