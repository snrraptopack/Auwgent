/// Block-based orchestrator for @@marker protocol
/// Handles both predefined intents (tool_call, response_text, etc.) and custom intents

use function_parser::{BlockScanner, BlockType, parse_function_calls, parse_ts_object, ASTValue};
use serde_json::{Value, Map};
use std::collections::HashSet;
use std::sync::Arc;

pub type IntentHandler = Arc<dyn Fn(String, Value) + Send + Sync>;

pub struct BlockOrchestrator {
    buffer: String,
    intent_keys: HashSet<String>, // Both predefined and custom intent names
    intent_handler: Option<IntentHandler>,
    partial_handler: Option<IntentHandler>,
    emitted_identities: HashSet<String>,
}

impl BlockOrchestrator {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            intent_keys: HashSet::new(),
            intent_handler: None,
            partial_handler: None,
            emitted_identities: HashSet::new(),
        }
    }

    pub fn register_intent(&mut self, key: &str) {
        self.intent_keys.insert(key.to_string());
    }

    pub fn on_intent_ready(&mut self, handler: IntentHandler) {
        self.intent_handler = Some(handler);
    }

    pub fn on_intent_partial(&mut self, handler: IntentHandler) {
        self.partial_handler = Some(handler);
    }

    pub fn write(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        self.check_blocks(false);
    }

    pub fn end(&mut self) -> Value {
        self.check_blocks(true);
        Value::Null
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.emitted_identities.clear();
    }

    fn check_blocks(&mut self, is_final: bool) {
        let mut scanner = BlockScanner::new(&self.buffer);
        let blocks = scanner.scan();

        if blocks.is_empty() {
            // Fallback: treat entire buffer as implicit chat
            let trimmed = self.buffer.trim();
            if !trimmed.is_empty() && self.intent_keys.contains("response_text") {
                let intent = serde_json::json!({ "text": trimmed });
                self.emit_intent("response_text", intent, is_final, true);
            }
            return;
        }

        // Terminal intent types use last-complete-wins strategy
        // Only response_schema is truly terminal (last-wins)
        // response_text can appear multiple times and all should be emitted
        let terminal_types: HashSet<&str> = ["response_schema"].iter().cloned().collect();
        let mut terminal_intents: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();

        for block in blocks.iter() {
            match &block.block_type {
                BlockType::Chat => {
                    if !block.content.is_empty() {
                        let intent = serde_json::json!({ "text": block.content });
                        
                        if terminal_types.contains("response_text") {
                            terminal_intents.entry("response_text".to_string())
                                .or_insert_with(Vec::new)
                                .push(intent);
                        } else {
                            self.emit_intent("response_text", intent, is_final, false);
                        }
                    }
                }

                BlockType::Tool => {
                    let calls = parse_function_calls(&block.content);
                    for call in calls {
                        let args_json = ast_to_json_object(&call.args);
                        let intent = serde_json::json!({
                            "type": call.name,
                            "args": args_json
                        });
                        self.emit_intent("tool_call", intent, is_final, false);
                    }
                }

                BlockType::Workflow => {
                    let calls = parse_function_calls(&block.content);
                    if let Some(call) = calls.first() {
                        let args_json = ast_to_json_object(&call.args);
                        let intent = serde_json::json!({
                            "type": call.name,
                            "args": args_json
                        });
                        self.emit_intent("workflow_call", intent, is_final, false);
                    }
                }

                BlockType::Helper => {
                    let calls = parse_function_calls(&block.content);
                    if let Some(call) = calls.first() {
                        let args_json = ast_to_json_object(&call.args);
                        let intent = serde_json::json!({
                            "type": call.name,
                            "args": args_json
                        });
                        self.emit_intent("helper_call", intent, is_final, false);
                    }
                }

                BlockType::Out => {
                    if let Ok(obj_ast) = parse_ts_object(&block.content) {
                        let obj_json = ast_to_json(&obj_ast);
                        let intent = serde_json::json!({
                            "type": block.schema_name.as_ref().map(|s| s.as_str()).unwrap_or(""),
                            "response": obj_json
                        });
                        
                        if terminal_types.contains("response_schema") {
                            terminal_intents.entry("response_schema".to_string())
                                .or_insert_with(Vec::new)
                                .push(intent);
                        } else {
                            self.emit_intent("response_schema", intent, is_final, false);
                        }
                    }
                }

                BlockType::Custom(intent_name) => {
                    // Custom intent - parse content as key-value assignments
                    // Try parsing as function call first (for backwards compatibility)
                    let calls = parse_function_calls(&block.content);
                    if let Some(call) = calls.first() {
                        // Function call format: IntentName(key = value, ...)
                        let args_json = ast_to_json_object(&call.args);
                        self.emit_intent(&intent_name, args_json, is_final, true);
                    } else {
                        // Try parsing as bare key-value assignments: key = value, key2 = value2
                        // Wrap in braces to use the TS object parser
                        let wrapped = format!("{{{}}}", block.content);
                        if let Ok(obj_ast) = parse_ts_object(&wrapped) {
                            if let ASTValue::Object(fields) = obj_ast {
                                let args_json = ast_to_json_object(&fields);
                                self.emit_intent(&intent_name, args_json, is_final, true);
                            } else {
                                // Fallback: treat content as raw text
                                let intent = serde_json::json!({ "content": block.content });
                                self.emit_intent(&intent_name, intent, is_final, true);
                            }
                        } else {
                            // Fallback: treat content as raw text
                            let intent = serde_json::json!({ "content": block.content });
                            self.emit_intent(&intent_name, intent, is_final, true);
                        }
                    }
                }

                BlockType::Result | BlockType::Error => {
                    // System-injected blocks - not emitted as intents
                }
            }
        }

        // Emit terminal intents using last-wins strategy
        for (intent_name, instances) in terminal_intents {
            if let Some(last_intent) = instances.last() {
                self.emit_intent(&intent_name, last_intent.clone(), is_final, true);
            }
        }
    }

    fn emit_intent(&mut self, name: &str, value: Value, is_final: bool, _is_terminal: bool) {
        let content_hash = format!("{}:{}", name, serde_json::to_string(&value).unwrap_or_default());

        // IMPORTANT: All intents should only be emitted when is_final = true
        // This prevents duplicate emissions during streaming as the LLM generates arguments token by token
        // Previously, action intents were emitted immediately during streaming, but this caused
        // tools to be called multiple times with partial arguments (e.g., name="The", name="Theoph", name="Theophilus")
        
        if is_final {
            // Final emission - emit if not already emitted
            if !self.emitted_identities.contains(&content_hash) {
                if let Some(handler) = &self.intent_handler {
                    handler(name.to_string(), value.clone());
                    self.emitted_identities.insert(content_hash.clone());
                }
            }
        }
        
        // Always fire partial handler during streaming (for UI updates)
        if !is_final {
            if let Some(handler) = &self.partial_handler {
                handler(name.to_string(), value);
            }
        }
    }
}

// Convert ASTValue to serde_json::Value
fn ast_to_json(val: &ASTValue) -> Value {
    match val {
        ASTValue::String(s) => Value::String(s.clone()),
        ASTValue::Number(n) => {
            if let Some(num) = serde_json::Number::from_f64(*n) {
                Value::Number(num)
            } else {
                Value::Null
            }
        }
        ASTValue::Boolean(b) => Value::Bool(*b),
        ASTValue::Null => Value::Null,
        ASTValue::Array(arr) => {
            let vec: Vec<Value> = arr.iter().map(ast_to_json).collect();
            Value::Array(vec)
        }
        ASTValue::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), ast_to_json(v));
            }
            Value::Object(map)
        }
    }
}

fn ast_to_json_object(fields: &std::collections::HashMap<String, ASTValue>) -> Value {
    let mut map = Map::new();
    for (k, v) in fields {
        map.insert(k.clone(), ast_to_json(v));
    }
    Value::Object(map)
}
