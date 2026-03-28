/// Block-based orchestrator for @@marker protocol
/// Handles both predefined intents (tool_call, response_text, etc.) and custom intents
use crate::flat_args::{
    alias_map_from_specs, flatten_helper_input_specs, flatten_named_field_specs,
    flatten_output_specs, unflatten_object,
};
use crate::types::TypeDefinition;
use function_parser::{
    ASTValue, BlockScanner, BlockType, parse_assignment_object, parse_ts_object,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub type IntentHandler = Arc<dyn Fn(String, Value) + Send + Sync>;

pub struct BlockOrchestrator {
    buffer: String,
    intent_keys: HashSet<String>, // Both predefined and custom intent names
    intent_handler: Option<IntentHandler>,
    partial_handler: Option<IntentHandler>,
    emitted_identities: HashSet<String>,
    tool_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    workflow_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    helper_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    custom_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    output_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
}

impl BlockOrchestrator {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            intent_keys: HashSet::new(),
            intent_handler: None,
            partial_handler: None,
            emitted_identities: HashSet::new(),
            tool_alias_maps: HashMap::new(),
            workflow_alias_maps: HashMap::new(),
            helper_alias_maps: HashMap::new(),
            custom_alias_maps: HashMap::new(),
            output_alias_maps: HashMap::new(),
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

    pub fn register_tool_shape(
        &mut self,
        tool_name: &str,
        params: &Value,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        let specs = flatten_named_field_specs(params, types);
        if !specs.is_empty() {
            self.tool_alias_maps
                .insert(tool_name.to_string(), alias_map_from_specs(&specs));
        }
    }

    pub fn register_workflow_shape(
        &mut self,
        workflow_name: &str,
        params: &Value,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        let specs = flatten_named_field_specs(params, types);
        if !specs.is_empty() {
            self.workflow_alias_maps
                .insert(workflow_name.to_string(), alias_map_from_specs(&specs));
        }
    }

    pub fn register_helper_shape(
        &mut self,
        helper_name: &str,
        input_ir: Option<&Value>,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        let specs = flatten_helper_input_specs(input_ir, types);
        if !specs.is_empty() {
            self.helper_alias_maps
                .insert(helper_name.to_string(), alias_map_from_specs(&specs));
        }
    }

    pub fn register_custom_intent_shape(
        &mut self,
        intent_name: &str,
        fields: &Value,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        let specs = flatten_named_field_specs(fields, types);
        if !specs.is_empty() {
            self.custom_alias_maps
                .insert(intent_name.to_string(), alias_map_from_specs(&specs));
        }
    }

    pub fn register_output_shape(
        &mut self,
        output: &Value,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        for (schema_name, specs) in flatten_output_specs(output, types) {
            if !specs.is_empty() {
                self.output_alias_maps
                    .insert(schema_name, alias_map_from_specs(&specs));
            }
        }
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
            if !trimmed.is_empty()
                && self.intent_keys.contains("response_text")
                && !is_incomplete_response_text_open(trimmed)
            {
                let intent = serde_json::json!({ "text": trimmed });
                self.emit_intent("response_text", intent, is_final, true);
            }
            return;
        }

        // Terminal intent types use last-complete-wins strategy
        // Only response_schema is truly terminal (last-wins)
        // response_text can appear multiple times and all should be emitted
        let terminal_types: HashSet<&str> = ["response_schema"].iter().cloned().collect();
        let mut terminal_intents: std::collections::HashMap<String, Vec<Value>> =
            std::collections::HashMap::new();

        for block in blocks.iter() {
            match &block.block_type {
                BlockType::Chat => {
                    if !block.content.is_empty() {
                        let intent = serde_json::json!({ "text": block.content });

                        if terminal_types.contains("response_text") {
                            terminal_intents
                                .entry("response_text".to_string())
                                .or_insert_with(Vec::new)
                                .push(intent);
                        } else {
                            self.emit_intent("response_text", intent, is_final, false);
                        }
                    }
                }

                BlockType::Tool => {
                    if let Some(tool_name) = block.target_name.as_deref() {
                        if let Some(fields) = parse_block_fields(&block.content) {
                            let args_json =
                                self.unflatten_tool_args(tool_name, ast_to_json_object(&fields));
                            let intent = serde_json::json!({
                                "type": tool_name,
                                "args": args_json
                            });
                            self.emit_intent("tool_call", intent, is_final, false);
                        }
                    }
                }

                BlockType::Workflow => {
                    if let Some(workflow_name) = block.target_name.as_deref() {
                        if let Some(fields) = parse_block_fields(&block.content) {
                            let args_json = self.unflatten_workflow_args(
                                workflow_name,
                                ast_to_json_object(&fields),
                            );
                            let intent = serde_json::json!({
                                "type": workflow_name,
                                "args": args_json
                            });
                            self.emit_intent("workflow_call", intent, is_final, false);
                        }
                    }
                }

                BlockType::Helper => {
                    if let Some(helper_name) = block.target_name.as_deref() {
                        if let Some(fields) = parse_block_fields(&block.content) {
                            let args_json = self
                                .unflatten_helper_args(helper_name, ast_to_json_object(&fields));
                            let intent = serde_json::json!({
                                "type": helper_name,
                                "args": args_json
                            });
                            self.emit_intent("helper_call", intent, is_final, false);
                        }
                    }
                }

                BlockType::Out => {
                    if let Ok(obj_ast) = parse_schema_content(&block.content) {
                        let obj_json = self.unflatten_schema_response(
                            block.target_name.as_deref().unwrap_or("Output"),
                            ast_to_json(&obj_ast),
                        );
                        let intent = serde_json::json!({
                            "type": block.target_name.as_ref().map(|s| s.as_str()).unwrap_or(""),
                            "response": obj_json
                        });

                        if terminal_types.contains("response_schema") {
                            terminal_intents
                                .entry("response_schema".to_string())
                                .or_insert_with(Vec::new)
                                .push(intent);
                        } else {
                            self.emit_intent("response_schema", intent, is_final, false);
                        }
                    }
                }

                BlockType::Custom(intent_name) => {
                    if let Some(fields) = parse_block_fields(&block.content) {
                        let args_json =
                            self.unflatten_custom_args(intent_name, ast_to_json_object(&fields));
                        self.emit_intent(&intent_name, args_json, is_final, true);
                    } else {
                        let intent = serde_json::json!({ "content": block.content });
                        self.emit_intent(&intent_name, intent, is_final, true);
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
        let content_hash = format!(
            "{}:{}",
            name,
            serde_json::to_string(&value).unwrap_or_default()
        );

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

    fn unflatten_tool_args(&self, tool_name: &str, args: Value) -> Value {
        self.tool_alias_maps
            .get(tool_name)
            .map(|aliases| unflatten_object(&args, aliases))
            .unwrap_or(args)
    }

    fn unflatten_workflow_args(&self, workflow_name: &str, args: Value) -> Value {
        self.workflow_alias_maps
            .get(workflow_name)
            .map(|aliases| unflatten_object(&args, aliases))
            .unwrap_or(args)
    }

    fn unflatten_helper_args(&self, helper_name: &str, args: Value) -> Value {
        self.helper_alias_maps
            .get(helper_name)
            .map(|aliases| unflatten_object(&args, aliases))
            .unwrap_or(args)
    }

    fn unflatten_custom_args(&self, intent_name: &str, args: Value) -> Value {
        self.custom_alias_maps
            .get(intent_name)
            .map(|aliases| unflatten_object(&args, aliases))
            .unwrap_or(args)
    }

    fn unflatten_schema_response(&self, schema_name: &str, response: Value) -> Value {
        self.output_alias_maps
            .get(schema_name)
            .or_else(|| self.output_alias_maps.get("Output"))
            .map(|aliases| unflatten_object(&response, aliases))
            .unwrap_or(response)
    }
}

fn parse_block_fields(content: &str) -> Option<std::collections::HashMap<String, ASTValue>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Some(std::collections::HashMap::new());
    }

    if trimmed.starts_with('{') {
        match parse_ts_object(trimmed).ok()? {
            ASTValue::Object(obj) => Some(obj),
            _ => None,
        }
    } else {
        parse_assignment_object(trimmed).ok()
    }
}

fn parse_schema_content(content: &str) -> Result<ASTValue, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(ASTValue::Object(std::collections::HashMap::new()));
    }

    if trimmed.starts_with('{') {
        parse_ts_object(trimmed)
    } else {
        parse_assignment_object(trimmed).map(ASTValue::Object)
    }
}

fn is_incomplete_response_text_open(input: &str) -> bool {
    const OPEN_TAG: &str = "<response_text>";
    let trimmed = input.trim_start();
    !trimmed.is_empty()
        && trimmed.len() < OPEN_TAG.len()
        && OPEN_TAG.starts_with(trimmed)
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
