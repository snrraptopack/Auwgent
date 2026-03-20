use function_parser::ast::ASTValue;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::Arc;

pub type IntentHandler = Arc<dyn Fn(String, Value) + Send + Sync>;

pub struct FunctionOrchestrator {
    buffer: String,
    intent_keys: HashSet<String>,
    intent_handler: Option<IntentHandler>,
    partial_handler: Option<IntentHandler>,
    emitted_identities: HashSet<usize>,
}

impl FunctionOrchestrator {
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
        self.check_intents(false);
    }

    pub fn end(&mut self) -> Value {
        self.check_intents(true);
        Value::Null
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.emitted_identities.clear();
    }

    fn check_intents(&mut self, is_final: bool) {
        let mut parser = function_parser::Parser::new(&self.buffer);
        let intents = parser.parse();

        // Fallback: If no intents are found and there's text, treat it as a raw response_text
        if intents.is_empty() {
            let trimmed = self.buffer.trim();
            if !trimmed.is_empty() && self.intent_keys.contains("response_text") {
                let json_val = serde_json::json!({ "text": trimmed });
                if is_final {
                    if !self.emitted_identities.contains(&0) {
                        if let Some(handler) = &self.intent_handler {
                            handler("response_text".to_string(), json_val);
                            self.emitted_identities.insert(0);
                        }
                    }
                } else {
                    if let Some(handler) = &self.partial_handler {
                        handler("response_text".to_string(), json_val);
                    }
                }
                return;
            }
        }

        for (i, intent) in intents.into_iter().enumerate() {
            if !self.intent_keys.contains(&intent.name) {
                continue;
            }

            let json_val = ast_to_json_object(&intent.fields);
            let is_complete = intent.is_complete || is_final;

            if !is_complete {
                if let Some(handler) = &self.partial_handler {
                    handler(intent.name.clone(), json_val);
                }
            } else {
                if !self.emitted_identities.contains(&i) {
                    if let Some(handler) = &self.intent_handler {
                        handler(intent.name.clone(), json_val);
                        self.emitted_identities.insert(i);
                    }
                }
            }
        }
    }
}

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
