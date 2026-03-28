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
    last_partial_payloads: HashMap<String, String>,
    tool_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    tool_arg_templates: HashMap<String, Value>,
    workflow_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    workflow_arg_templates: HashMap<String, Value>,
    helper_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    helper_arg_templates: HashMap<String, Value>,
    custom_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    custom_templates: HashMap<String, Value>,
    output_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    output_response_templates: HashMap<String, Value>,
}

impl BlockOrchestrator {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            intent_keys: HashSet::new(),
            intent_handler: None,
            partial_handler: None,
            emitted_identities: HashSet::new(),
            last_partial_payloads: HashMap::new(),
            tool_alias_maps: HashMap::new(),
            tool_arg_templates: HashMap::new(),
            workflow_alias_maps: HashMap::new(),
            workflow_arg_templates: HashMap::new(),
            helper_alias_maps: HashMap::new(),
            helper_arg_templates: HashMap::new(),
            custom_alias_maps: HashMap::new(),
            custom_templates: HashMap::new(),
            output_alias_maps: HashMap::new(),
            output_response_templates: HashMap::new(),
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
        self.tool_arg_templates.insert(
            tool_name.to_string(),
            build_named_field_template(params, types),
        );
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
        self.workflow_arg_templates.insert(
            workflow_name.to_string(),
            build_named_field_template(params, types),
        );
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
        self.helper_arg_templates.insert(
            helper_name.to_string(),
            build_helper_input_template(input_ir, types),
        );
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
        self.custom_templates.insert(
            intent_name.to_string(),
            build_named_field_template(fields, types),
        );
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
        self.output_response_templates
            .extend(build_output_templates(output, types));
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
        self.last_partial_payloads.clear();
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

        for (block_index, block) in blocks.iter().enumerate() {
            match &block.block_type {
                BlockType::Chat => {
                    if !block.content.is_empty() {
                        let intent = if is_final {
                            serde_json::json!({ "text": block.content })
                        } else {
                            build_partial_text_payload(&block.content, block_index)
                        };

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
                        let parsed = parse_partial_block_fields(&block.content)
                            .map(|fields| self.unflatten_tool_args(tool_name, ast_to_json_object(&fields)));
                        if is_final && parsed.is_none() {
                            continue;
                        }
                        let args_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
                        let intent = serde_json::json!({
                            "type": tool_name,
                            "args": args_json
                        });
                        let intent = if is_final {
                            intent
                        } else {
                            let snapshot = serde_json::json!({
                                "type": tool_name,
                                "args": self.merge_tool_args(tool_name, &intent["args"]),
                            });
                            build_partial_structured_payload(snapshot, &block.content, block_index)
                        };
                        self.emit_intent("tool_call", intent, is_final, false);
                    }
                }

                BlockType::Workflow => {
                    if let Some(workflow_name) = block.target_name.as_deref() {
                        let parsed = parse_partial_block_fields(&block.content).map(|fields| {
                            self.unflatten_workflow_args(workflow_name, ast_to_json_object(&fields))
                        });
                        if is_final && parsed.is_none() {
                            continue;
                        }
                        let args_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
                        let intent = serde_json::json!({
                            "type": workflow_name,
                            "args": args_json
                        });
                        let intent = if is_final {
                            intent
                        } else {
                            let snapshot = serde_json::json!({
                                "type": workflow_name,
                                "args": self.merge_workflow_args(workflow_name, &intent["args"]),
                            });
                            build_partial_structured_payload(snapshot, &block.content, block_index)
                        };
                        self.emit_intent("workflow_call", intent, is_final, false);
                    }
                }

                BlockType::Helper => {
                    if let Some(helper_name) = block.target_name.as_deref() {
                        let parsed = parse_partial_block_fields(&block.content).map(|fields| {
                            self.unflatten_helper_args(helper_name, ast_to_json_object(&fields))
                        });
                        if is_final && parsed.is_none() {
                            continue;
                        }
                        let args_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
                        let intent = serde_json::json!({
                            "type": helper_name,
                            "args": args_json
                        });
                        let intent = if is_final {
                            intent
                        } else {
                            let snapshot = serde_json::json!({
                                "type": helper_name,
                                "args": self.merge_helper_args(helper_name, &intent["args"]),
                            });
                            build_partial_structured_payload(snapshot, &block.content, block_index)
                        };
                        self.emit_intent("helper_call", intent, is_final, false);
                    }
                }

                BlockType::Out => {
                    let schema_name = block.target_name.as_deref().unwrap_or("Output");
                    let parsed = parse_partial_schema_content(&block.content)
                        .map(|obj_ast| self.unflatten_schema_response(schema_name, ast_to_json(&obj_ast)));
                    if is_final && parsed.is_none() {
                        continue;
                    }
                    let obj_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
                    let intent = serde_json::json!({
                        "type": block.target_name.as_ref().map(|s| s.as_str()).unwrap_or(""),
                        "response": obj_json
                    });
                    let intent = if is_final {
                        intent
                    } else {
                        let snapshot = serde_json::json!({
                            "type": block.target_name.as_ref().map(|s| s.as_str()).unwrap_or(""),
                            "response": self.merge_schema_response(schema_name, &intent["response"]),
                        });
                        build_partial_structured_payload(snapshot, &block.content, block_index)
                    };

                    if terminal_types.contains("response_schema") {
                        terminal_intents
                            .entry("response_schema".to_string())
                            .or_insert_with(Vec::new)
                            .push(intent);
                    } else {
                        self.emit_intent("response_schema", intent, is_final, false);
                    }
                }

                BlockType::Custom(intent_name) => {
                    if let Some(fields) = parse_partial_block_fields(&block.content) {
                        let args_json =
                            self.unflatten_custom_args(intent_name, ast_to_json_object(&fields));
                        let intent = if is_final {
                            args_json
                        } else {
                            let snapshot = self.merge_custom_args(intent_name, &args_json);
                            build_partial_structured_payload(
                                snapshot,
                                &block.content,
                                block_index,
                            )
                        };
                        self.emit_intent(&intent_name, intent, is_final, true);
                    } else {
                        if is_final {
                            let intent = serde_json::json!({ "content": block.content });
                            self.emit_intent(&intent_name, intent, is_final, true);
                            continue;
                        }
                        let intent = build_partial_structured_payload(
                            self.merge_custom_args(intent_name, &Value::Object(Map::new())),
                            &block.content,
                            block_index,
                        );
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
            let partial_key = partial_payload_key(name, &value);
            let payload_json = serde_json::to_string(&value).unwrap_or_default();
            if self
                .last_partial_payloads
                .get(&partial_key)
                .is_some_and(|previous| previous == &payload_json)
            {
                return;
            }
            self.last_partial_payloads.insert(partial_key, payload_json);
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

    fn merge_tool_args(&self, tool_name: &str, args: &Value) -> Value {
        merge_template(
            self.tool_arg_templates
                .get(tool_name)
                .unwrap_or(&Value::Object(Map::new())),
            args,
        )
    }

    fn merge_workflow_args(&self, workflow_name: &str, args: &Value) -> Value {
        merge_template(
            self.workflow_arg_templates
                .get(workflow_name)
                .unwrap_or(&Value::Object(Map::new())),
            args,
        )
    }

    fn merge_helper_args(&self, helper_name: &str, args: &Value) -> Value {
        merge_template(
            self.helper_arg_templates
                .get(helper_name)
                .unwrap_or(&Value::Object(Map::new())),
            args,
        )
    }

    fn merge_custom_args(&self, intent_name: &str, args: &Value) -> Value {
        merge_template(
            self.custom_templates
                .get(intent_name)
                .unwrap_or(&Value::Object(Map::new())),
            args,
        )
    }

    fn merge_schema_response(&self, schema_name: &str, response: &Value) -> Value {
        merge_template(
            self.output_response_templates
                .get(schema_name)
                .or_else(|| self.output_response_templates.get("Output"))
                .unwrap_or(&Value::Object(Map::new())),
            response,
        )
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

fn parse_partial_block_fields(content: &str) -> Option<std::collections::HashMap<String, ASTValue>> {
    if let Some(parsed) = parse_block_fields(content) {
        return Some(parsed);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut best = None;
    for line_count in 1..=lines.len() {
        let candidate = lines[..line_count].join("\n");
        if let Some(parsed) = parse_block_fields(&candidate) {
            best = Some(parsed);
        }
    }

    best
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

fn parse_partial_schema_content(content: &str) -> Option<ASTValue> {
    if let Ok(parsed) = parse_schema_content(content) {
        return Some(parsed);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut best = None;
    for line_count in 1..=lines.len() {
        let candidate = lines[..line_count].join("\n");
        if let Ok(parsed) = parse_schema_content(&candidate) {
            best = Some(parsed);
        }
    }

    best
}

fn is_incomplete_response_text_open(input: &str) -> bool {
    const OPEN_TAG: &str = "<response_text>";
    let trimmed = input.trim_start();
    !trimmed.is_empty()
        && trimmed.len() < OPEN_TAG.len()
        && OPEN_TAG.starts_with(trimmed)
}

fn build_partial_text_payload(text: &str, segment: usize) -> Value {
    serde_json::json!({
        "partial": true,
        "complete": false,
        "mode": "text",
        "segment": segment,
        "snapshot": {
            "text": text
        },
        "raw": text
    })
}

fn build_partial_structured_payload(snapshot: Value, raw: &str, segment: usize) -> Value {
    serde_json::json!({
        "partial": true,
        "complete": false,
        "mode": "structured",
        "segment": segment,
        "snapshot": snapshot,
        "raw": raw,
    })
}

fn partial_payload_key(name: &str, value: &Value) -> String {
    let segment = value
        .get("segment")
        .and_then(Value::as_u64)
        .map(|segment| segment.to_string())
        .unwrap_or_else(|| "default".to_string());
    format!("{name}:{segment}")
}

fn merge_template(template: &Value, actual: &Value) -> Value {
    match (template, actual) {
        (Value::Object(template_obj), Value::Object(actual_obj)) => {
            let mut merged = template_obj.clone();
            for (key, actual_value) in actual_obj {
                let next_value = merged
                    .get(key)
                    .map(|template_value| merge_template(template_value, actual_value))
                    .unwrap_or_else(|| actual_value.clone());
                merged.insert(key.clone(), next_value);
            }
            Value::Object(merged)
        }
        (Value::Array(_), Value::Array(_)) => actual.clone(),
        (_, _) => actual.clone(),
    }
}

fn build_named_field_template(
    schema_value: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Value {
    Value::Object(build_template_fields(schema_value, types))
}

fn build_helper_input_template(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Value {
    let Some(input) = input_ir else {
        return Value::Object(Map::new());
    };

    if input.get("kind").and_then(|v| v.as_str()) == Some("properties") {
        if let Some(fields) = input.get("fields") {
            return build_named_field_template(fields, types);
        }
    }

    if input.get("kind").and_then(|v| v.as_str()) == Some("direct") {
        if let Some(ty) = input.get("type") {
            if let Some(props) = resolve_template_properties(ty, types) {
                return build_named_field_template(&Value::Object(props), types);
            }
            let mut map = Map::new();
            map.insert("input".to_string(), pending_value());
            return Value::Object(map);
        }
    }

    if let Some(props) = resolve_template_properties(input, types) {
        return build_named_field_template(&Value::Object(props), types);
    }

    if input.as_object().is_some() {
        let filtered: Map<String, Value> = input
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__") && *key != "kind")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        if !filtered.is_empty() {
            return build_named_field_template(&Value::Object(filtered), types);
        }
    }

    Value::Object(Map::new())
}

fn build_output_templates(
    output: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> HashMap<String, Value> {
    let mut templates = HashMap::new();

    if let Some(variants) = output.get("__variants").and_then(|v| v.as_object()) {
        for (schema_name, schema_value) in variants {
            templates.insert(
                schema_name.clone(),
                Value::Object(build_template_fields(schema_value, types)),
            );
        }
        return templates;
    }

    templates.insert(
        "Output".to_string(),
        Value::Object(build_template_fields(output, types)),
    );
    templates
}

fn build_template_fields(
    schema_value: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Map<String, Value> {
    if let Some(obj) = schema_value.as_object() {
        if let Some(properties) = obj.get("properties").and_then(|v| v.as_object()) {
            return properties
                .iter()
                .map(|(key, value)| (key.clone(), build_template_value(value, types)))
                .collect();
        }

        if let Some(type_value) = obj.get("type") {
            if let Some(properties) = resolve_template_properties(type_value, types) {
                return properties
                    .iter()
                    .map(|(key, value)| (key.clone(), build_template_value(value, types)))
                    .collect();
            }
        }

        return obj
            .iter()
            .filter(|(key, _)| !key.starts_with('@') && !key.starts_with("__"))
            .map(|(key, value)| (key.clone(), build_template_value(value, types)))
            .collect();
    }

    Map::new()
}

fn build_template_value(
    def: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Value {
    if let Some(properties) = resolve_template_properties(def, types) {
        let nested = properties
            .iter()
            .map(|(key, value)| (key.clone(), build_template_value(value, types)))
            .collect();
        return Value::Object(nested);
    }

    if is_array_definition(def) {
        return Value::Array(Vec::new());
    }

    pending_value()
}

fn resolve_template_properties(
    def: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Option<Map<String, Value>> {
    if def.get("type").and_then(|v| v.as_str()) == Some("object") {
        return def.get("properties").and_then(|v| v.as_object()).cloned();
    }

    if let Some(type_obj) = def.get("type").and_then(|v| v.as_object()) {
        if type_obj.get("type").and_then(|v| v.as_str()) == Some("object") {
            return type_obj
                .get("properties")
                .and_then(|v| v.as_object())
                .cloned();
        }

        if type_obj.get("type").and_then(|v| v.as_str()) == Some("typeRef") {
            let ref_name = type_obj.get("name").and_then(|v| v.as_str())?;
            let custom_type = types?.get(ref_name)?;
            let props_value = serde_json::to_value(&custom_type.properties).ok()?;
            return props_value.as_object().cloned();
        }
    }

    if def.get("type").and_then(|v| v.as_str()) == Some("typeRef") {
        let ref_name = def.get("name").and_then(|v| v.as_str())?;
        let custom_type = types?.get(ref_name)?;
        let props_value = serde_json::to_value(&custom_type.properties).ok()?;
        return props_value.as_object().cloned();
    }

    None
}

fn is_array_definition(def: &Value) -> bool {
    if def.get("type").and_then(|v| v.as_str()) == Some("array") {
        return true;
    }

    def.get("type")
        .and_then(|v| v.as_object())
        .and_then(|inner| inner.get("type"))
        .and_then(Value::as_str)
        == Some("array")
}

fn pending_value() -> Value {
    serde_json::json!({ "$state": "pending" })
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
