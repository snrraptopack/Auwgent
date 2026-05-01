/// Block-based orchestrator for [] protocol #edit
/// Handles both predefined intents (tool_call, response_text, etc.) and custom intents
use crate::flat_args::{
    alias_map_from_specs, flatten_helper_input_specs, flatten_named_field_specs,
    flatten_output_specs, unflatten_object,
};
use crate::types::{ComponentDefinition, TypeDefinition};
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
    component_alias_maps: HashMap<String, HashMap<String, Vec<String>>>,
    component_templates: HashMap<String, Value>,
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
            component_alias_maps: HashMap::new(),
            component_templates: HashMap::new(),
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

    pub fn register_component_shape(
        &mut self,
        component: &ComponentDefinition,
        types: Option<&HashMap<String, TypeDefinition>>,
    ) {
        let (aliases, template) = build_component_shape(component, types);
        if !aliases.is_empty() {
            self.component_alias_maps
                .insert(component.name.clone(), aliases);
        }
        self.component_templates
            .insert(component.name.clone(), template);
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
            let sanitized = strip_protocol_fragments(&self.buffer);
            let trimmed = sanitized.trim();
            if !trimmed.is_empty()
                && self.intent_keys.contains("response_text")
                && !is_incomplete_response_text_open(trimmed)
                && !is_incomplete_protocol_header(trimmed)
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
        let component_instances = self.collect_component_instances(&blocks, is_final);

        for (block_index, block) in blocks.iter().enumerate() {
            match &block.block_type {
                BlockType::Chat => {
                    let sanitized = strip_protocol_fragments(&block.content);
                    if !sanitized.is_empty()
                        && (is_final || !is_incomplete_protocol_header(sanitized.trim()))
                    {
                        let intent = if is_final {
                            serde_json::json!({ "text": sanitized })
                        } else {
                            build_partial_text_payload(&sanitized, block_index)
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
                        if !is_final && block.content.trim().is_empty() {
                            continue;
                        }
                        let parsed = parse_partial_block_fields(&block.content).map(|fields| {
                            self.unflatten_tool_args(tool_name, ast_to_json_object(&fields))
                        });
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
                        if !is_final && block.content.trim().is_empty() {
                            continue;
                        }
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
                        if !is_final && block.content.trim().is_empty() {
                            continue;
                        }
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

                BlockType::Component => {
                    if let (Some(component_name), Some(instance_id)) =
                        (block.target_name.as_deref(), block.instance_id.as_deref())
                    {
                        let parsed = parse_partial_block_fields(&block.content).map(|fields| {
                            self.unflatten_component_args(
                                component_name,
                                ast_to_json_object(&fields),
                            )
                        });
                        if is_final && parsed.is_none() {
                            continue;
                        }
                        let fields_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
                        let intent = build_component_intent(
                            component_name,
                            instance_id,
                            fields_json.clone(),
                        );
                        let intent = if is_final {
                            intent
                        } else {
                            let snapshot = build_component_intent(
                                component_name,
                                instance_id,
                                self.merge_component_args(component_name, &fields_json),
                            );
                            build_partial_structured_payload(snapshot, &block.content, block_index)
                        };
                        self.emit_intent("component", intent, is_final, false);
                    }
                }

                BlockType::Out => {
                    let schema_name = block.target_name.as_deref().unwrap_or("Output");
                    if !is_final && block.content.trim().is_empty() {
                        continue;
                    }
                    let parsed = parse_partial_schema_content(&block.content).map(|obj_ast| {
                        self.unflatten_schema_response(schema_name, ast_to_json(&obj_ast))
                    });
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

                BlockType::RenderComponent => {
                    let parsed = parse_partial_block_fields(&block.content)
                        .map(|fields| ast_to_json_object(&fields))
                        .unwrap_or_else(|| Value::Object(Map::new()));
                    let intent = build_render_component_intent(&parsed, &component_instances);
                    let intent = if is_final {
                        intent
                    } else {
                        build_partial_structured_payload(intent, &block.content, block_index)
                    };
                    self.emit_intent("render_component", intent, is_final, true);
                }

                BlockType::Custom(intent_name) => {
                    if let Some(fields) = parse_partial_block_fields(&block.content) {
                        let args_json =
                            self.unflatten_custom_args(intent_name, ast_to_json_object(&fields));
                        let intent = if is_final {
                            args_json
                        } else {
                            let snapshot = self.merge_custom_args(intent_name, &args_json);
                            build_partial_structured_payload(snapshot, &block.content, block_index)
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

    fn unflatten_component_args(&self, component_name: &str, args: Value) -> Value {
        self.component_alias_maps
            .get(component_name)
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

    fn merge_component_args(&self, component_name: &str, args: &Value) -> Value {
        merge_template(
            self.component_templates
                .get(component_name)
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

    fn collect_component_instances(
        &self,
        blocks: &[function_parser::Block],
        is_final: bool,
    ) -> HashMap<String, Value> {
        let mut instances = HashMap::new();

        for block in blocks {
            if block.block_type != BlockType::Component {
                continue;
            }

            let (Some(component_name), Some(instance_id)) =
                (block.target_name.as_deref(), block.instance_id.as_deref())
            else {
                continue;
            };

            let parsed = parse_partial_block_fields(&block.content).map(|fields| {
                self.unflatten_component_args(component_name, ast_to_json_object(&fields))
            });

            if is_final && parsed.is_none() {
                continue;
            }

            let fields_json = parsed.unwrap_or_else(|| Value::Object(Map::new()));
            let intent = if is_final {
                build_component_intent(component_name, instance_id, fields_json)
            } else {
                build_component_intent(
                    component_name,
                    instance_id,
                    self.merge_component_args(component_name, &fields_json),
                )
            };

            instances.insert(instance_id.to_string(), intent);
        }

        instances
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

fn parse_partial_block_fields(
    content: &str,
) -> Option<std::collections::HashMap<String, ASTValue>> {
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
    const OPEN_TAG: &str = "[response_text]";
    let trimmed = input.trim_start();
    !trimmed.is_empty() && trimmed.len() < OPEN_TAG.len() && OPEN_TAG.starts_with(trimmed)
}

fn is_incomplete_protocol_header(input: &str) -> bool {
    const HEADER_PREFIXES: &[&str] = &[
        "[response_text]",
        "[/response_text]",
        "[tool_call:]",
        "[/tool_call]",
        "[workflow_call:]",
        "[/workflow]",
        "[helper_call:]",
        "[/helper]",
        "[component:]",
        "[/component]",
        "[render_component]",
        "[/render_component]",
        "[schema:]",
        "[/schema]",
        "[custom:]",
        "[/custom]",
        "[result]",
        "[/result]",
        "[error]",
        "[/error]",
    ];

    let trimmed = input.trim_start();
    !trimmed.is_empty()
        && HEADER_PREFIXES
            .iter()
            .any(|prefix| prefix.starts_with(trimmed) || trimmed.starts_with(prefix))
}

fn strip_protocol_fragments(input: &str) -> String {
    const FRAGMENTS: &[&str] = &[
        "[/render_component",
        "[render_component",
        "[/response_text",
        "[response_text",
        "[/workflow_call",
        "[workflow_call",
        "[/helper_call",
        "[helper_call",
        "[/component",
        "[component",
        "[/tool_call",
        "[tool_call",
        "[/workflow",
        "[workflow",
        "[/response",
        "[/helper",
        "[helper",
        "[/schema",
        "[schema",
        "[/result",
        "[result",
        "[/custom",
        "[custom",
        "[/error",
        "[error",
        "[/tool",
        "[tool",
    ];

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while cursor < input.len() {
        let rest = &input[cursor..];
        if let Some(fragment) = FRAGMENTS
            .iter()
            .find(|fragment| rest.starts_with(**fragment))
        {
            cursor += protocol_fragment_skip_len(rest, fragment);
            continue;
        }

        if let Some(ch) = rest.chars().next() {
            out.push(ch);
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }

    out
}

fn protocol_fragment_skip_len(rest: &str, fragment: &str) -> usize {
    let mut consumed = fragment.len();
    let tail = &rest[fragment.len()..];

    // Bare prefixes such as `[tool_callAwaiting...` should only strip the prefix
    // so ordinary text that follows without a real header separator is preserved.
    let Some(first) = tail.chars().next() else {
        return consumed;
    };

    let is_header_tail = matches!(first, ':' | ']' | '\n' | '\r' | ' ' | '\t');
    if !is_header_tail {
        return consumed;
    }

    for ch in tail.chars() {
        if ch == '[' {
            break;
        }

        consumed += ch.len_utf8();

        if matches!(ch, ']' | '\n' | '\r') {
            break;
        }
    }

    consumed
}

fn build_partial_text_payload(text: &str, segment: usize) -> Value {
    serde_json::json!({
        "partial": true,
        "complete": false,
        "mode": "text",
        "segment": segment,
        "text": text,
        "raw": text
    })
}

fn build_partial_structured_payload(snapshot: Value, raw: &str, segment: usize) -> Value {
    let mut payload = match snapshot {
        Value::Object(map) => Value::Object(map),
        other => {
            let mut map = Map::new();
            map.insert("value".to_string(), other);
            Value::Object(map)
        }
    };

    if let Value::Object(ref mut map) = payload {
        map.insert("partial".to_string(), Value::Bool(true));
        map.insert("complete".to_string(), Value::Bool(false));
        map.insert("mode".to_string(), Value::String("structured".to_string()));
        map.insert(
            "segment".to_string(),
            Value::Number(serde_json::Number::from(segment as u64)),
        );
        map.insert("raw".to_string(), Value::String(raw.to_string()));
    }

    payload
}

fn build_component_intent(component_name: &str, instance_id: &str, fields: Value) -> Value {
    let mut intent = Map::new();
    intent.insert(
        "type".to_string(),
        Value::String(component_name.to_string()),
    );
    intent.insert("c_id".to_string(), Value::String(instance_id.to_string()));

    if let Value::Object(fields_obj) = fields {
        if let Some(props) = fields_obj.get("props") {
            intent.insert("props".to_string(), props.clone());
        }
        if let Some(action) = fields_obj.get("action") {
            intent.insert(
                "action".to_string(),
                normalize_component_action_value(action),
            );
        }
        if let Some(children) = fields_obj.get("children") {
            intent.insert("children".to_string(), children.clone());
        }
    }

    Value::Object(intent)
}

fn build_render_component_intent(
    render_fields: &Value,
    component_instances: &HashMap<String, Value>,
) -> Value {
    let mut intent = Map::new();

    if let Some(root) = render_fields.get("root").and_then(Value::as_str) {
        intent.insert("root".to_string(), Value::String(root.to_string()));
        if let Some(tree) =
            resolve_component_instance(root, component_instances, &mut HashSet::new())
        {
            intent.insert("tree".to_string(), tree);
        }
    }

    if let Some(roots) = render_fields.get("roots").and_then(Value::as_array) {
        intent.insert("roots".to_string(), Value::Array(roots.clone()));
        let mut trees = Vec::new();
        for root in roots.iter().filter_map(Value::as_str) {
            if let Some(tree) =
                resolve_component_instance(root, component_instances, &mut HashSet::new())
            {
                trees.push(tree);
            }
        }
        if !trees.is_empty() {
            intent.insert("trees".to_string(), Value::Array(trees));
        }
    }

    if !component_instances.is_empty() {
        let registry: Map<String, Value> = component_instances
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        intent.insert("components".to_string(), Value::Object(registry));
    }

    if intent.is_empty() {
        return render_fields.clone();
    }

    Value::Object(intent)
}

fn normalize_component_action_value(action: &Value) -> Value {
    let Some(action_obj) = action.as_object() else {
        return action.clone();
    };

    let mut normalized = Map::new();
    for (event_name, event_value) in action_obj {
        if let Some(call_obj) = event_value.as_object()
            && call_obj.get("__kind").and_then(Value::as_str) == Some("call")
        {
            let mut target = Map::new();
            if let Some(name) = call_obj.get("name") {
                target.insert("name".to_string(), name.clone());
            }
            if let Some(args) = call_obj.get("args") {
                target.insert("args".to_string(), args.clone());
            }
            normalized.insert(event_name.clone(), Value::Object(target));
            continue;
        }

        if let Some(name) = event_value.as_str() {
            normalized.insert(event_name.clone(), serde_json::json!({ "name": name }));
            continue;
        }

        normalized.insert(event_name.clone(), event_value.clone());
    }

    Value::Object(normalized)
}

fn resolve_component_instance(
    instance_id: &str,
    component_instances: &HashMap<String, Value>,
    seen: &mut HashSet<String>,
) -> Option<Value> {
    let component = component_instances.get(instance_id)?.as_object()?.clone();

    if !seen.insert(instance_id.to_string()) {
        return None;
    }

    let mut resolved = component;
    if let Some(children) = resolved.get("children").and_then(Value::as_array) {
        let mut resolved_children = Vec::new();
        for child_id in children.iter().filter_map(Value::as_str) {
            if let Some(child) = resolve_component_instance(child_id, component_instances, seen) {
                resolved_children.push(child);
            }
        }
        resolved.insert("children".to_string(), Value::Array(resolved_children));
    }

    seen.remove(instance_id);
    Some(Value::Object(resolved))
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
    if actual.is_null() {
        return template.clone();
    }

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

fn build_component_shape(
    component: &ComponentDefinition,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> (HashMap<String, Vec<String>>, Value) {
    let mut aliases = HashMap::new();
    let mut template = Map::new();

    let prop_specs = flatten_named_field_specs(&component.props.0, types);
    if !prop_specs.is_empty() {
        for spec in prop_specs {
            aliases.insert(
                spec.alias.clone(),
                std::iter::once("props".to_string())
                    .chain(spec.path.into_iter())
                    .collect(),
            );
        }
        template.insert(
            "props".to_string(),
            build_named_field_template(&component.props.0, types),
        );
    }

    if let Some(actions) = &component.action {
        let mut action_template = Map::new();
        for (event_name, _allowed_actions) in actions {
            aliases.insert(
                format!("action_{event_name}"),
                vec!["action".to_string(), event_name.clone()],
            );
            action_template.insert(event_name.clone(), pending_value());
        }
        template.insert("action".to_string(), Value::Object(action_template));
    }

    if component.children.is_some() {
        template.insert("children".to_string(), Value::Array(Vec::new()));
    }

    (aliases, Value::Object(template))
}

fn build_helper_input_template(
    input_ir: Option<&Value>,
    types: Option<&HashMap<String, TypeDefinition>>,
) -> Value {
    let Some(input) = input_ir else {
        return default_text_input_template();
    };

    if input.is_null() {
        return default_text_input_template();
    }

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

fn default_text_input_template() -> Value {
    let mut map = Map::new();
    map.insert("input".to_string(), pending_value());
    Value::Object(map)
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

fn build_template_value(def: &Value, types: Option<&HashMap<String, TypeDefinition>>) -> Value {
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
            let num = if n.is_finite() && n.fract() == 0.0 {
                if *n >= 0.0 && *n <= u64::MAX as f64 {
                    Some(serde_json::Number::from(*n as u64))
                } else if *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Some(serde_json::Number::from(*n as i64))
                } else {
                    serde_json::Number::from_f64(*n)
                }
            } else {
                serde_json::Number::from_f64(*n)
            };

            if let Some(num) = num {
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
        ASTValue::Call { name, args } => {
            let mut args_map = Map::new();
            for (key, value) in args {
                args_map.insert(key.clone(), ast_to_json(value));
            }
            serde_json::json!({
                "__kind": "call",
                "name": name,
                "args": Value::Object(args_map),
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_orchestrator() -> BlockOrchestrator {
        let mut orch = BlockOrchestrator::new();
        orch.register_intent("tool_call");
        orch.register_intent("workflow_call");
        orch.register_intent("response_schema");
        orch.register_intent("response_text");
        orch.register_intent("helper_call");
        orch
    }

    #[test]
    fn test_response_text_and_schema_both_emitted() {
        let mut orch = setup_orchestrator();

        // Register output shape for schema parsing
        let output = serde_json::json!({
            "name": { "type": "string" },
            "age": { "type": "number" },
            "country": { "type": "string" },
            "is_student": { "type": "boolean" }
        });
        orch.register_output_shape(&output, None);

        // Collect emitted intents
        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        // Simulate the model response arriving as a single chunk
        let model_output = " \n[response_text]\nHiroshi is a 21-year-old student from Japan.\n[/response_text]\n[schema: Output ]\nname: Hiroshi\nage: 21\ncountry: Japan\nis_student: true\n[/schema]";

        orch.write(model_output);

        // During streaming, no intents should be emitted (only partials)
        let emitted_during_streaming = emitted.lock().unwrap().clone();
        assert!(
            emitted_during_streaming.is_empty(),
            "No intents should be emitted during streaming, got: {:?}",
            emitted_during_streaming
        );

        // Finalize
        orch.end();

        // After finalization, both response_text and response_schema should be emitted
        let final_emitted = emitted.lock().unwrap().clone();
        let intent_names: Vec<&str> = final_emitted
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();

        assert!(
            intent_names.contains(&"response_text"),
            "Expected response_text intent to be emitted. Got: {:?}",
            intent_names
        );
        assert!(
            intent_names.contains(&"response_schema"),
            "Expected response_schema intent to be emitted. Got: {:?}",
            intent_names
        );

        // Verify response_text content
        let text_intent = final_emitted
            .iter()
            .find(|(n, _)| n == "response_text")
            .unwrap();
        assert_eq!(
            text_intent.1["text"].as_str().unwrap(),
            "Hiroshi is a 21-year-old student from Japan."
        );

        // Verify response_schema content
        let schema_intent = final_emitted
            .iter()
            .find(|(n, _)| n == "response_schema")
            .unwrap();
        assert_eq!(schema_intent.1["type"].as_str().unwrap(), "Output");
        assert!(
            schema_intent.1["response"].is_object(),
            "response_schema should have a response object"
        );
    }

    #[test]
    fn test_response_text_only() {
        let mut orch = setup_orchestrator();

        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[response_text]\nHello world!\n[/response_text]");
        orch.end();

        let final_emitted = emitted.lock().unwrap().clone();
        assert_eq!(final_emitted.len(), 1);
        assert_eq!(final_emitted[0].0, "response_text");
        assert_eq!(final_emitted[0].1["text"].as_str().unwrap(), "Hello world!");
    }

    #[test]
    fn incomplete_tool_header_is_not_emitted_as_response_text() {
        let mut orch = setup_orchestrator();

        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[tool_call: get_details");

        let emitted_during_streaming = emitted.lock().unwrap().clone();
        assert!(
            emitted_during_streaming.is_empty(),
            "Incomplete tool headers should not leak into response_text partials: {:?}",
            emitted_during_streaming
        );
    }

    #[test]
    fn bare_incomplete_protocol_prefix_is_not_emitted_as_response_text() {
        let mut orch = setup_orchestrator();

        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[tool_call");
        orch.write("[schema");

        let emitted_during_streaming = emitted.lock().unwrap().clone();
        assert!(
            emitted_during_streaming.is_empty(),
            "Bare incomplete protocol prefixes should not leak into response_text partials: {:?}",
            emitted_during_streaming
        );
    }

    #[test]
    fn ultra_short_protocol_prefixes_are_not_emitted_as_response_text() {
        let mut orch = setup_orchestrator();

        let partials = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let partials_for_handler = Arc::clone(&partials);
        orch.on_intent_partial(Arc::new(move |name, value| {
            partials_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[");
        orch.write("s");

        let partials = partials.lock().unwrap().clone();
        assert!(
            partials.is_empty(),
            "Ultra-short protocol prefixes like `[` or `[s` should not leak as response_text partials: {:?}",
            partials
        );
    }

    #[test]
    fn mixed_protocol_fragments_are_stripped_from_partial_response_text() {
        let mut orch = setup_orchestrator();

        let partials = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let partials_for_handler = Arc::clone(&partials);
        orch.on_intent_partial(Arc::new(move |name, value| {
            partials_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[tool_call[tool_callAwaiting location result...[tool_call[schema");

        let partials = partials.lock().unwrap().clone();
        assert_eq!(partials.len(), 1);
        assert_eq!(partials[0].0, "response_text");
        assert_eq!(
            partials[0].1["text"].as_str().unwrap(),
            "Awaiting location result..."
        );
    }

    #[test]
    fn malformed_named_tool_headers_do_not_leak_header_tail_into_response_text() {
        let mut orch = setup_orchestrator();

        let partials = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let partials_for_handler = Arc::clone(&partials);
        orch.on_intent_partial(Arc::new(move |name, value| {
            partials_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[tool_call: get_details][tool_call: get_location]");

        let partials = partials.lock().unwrap().clone();
        assert!(
            partials.is_empty(),
            "Malformed named tool headers should not leak as response_text partials: {:?}",
            partials
        );
    }

    #[test]
    fn malformed_schema_header_does_not_leak_header_tail_into_response_text() {
        let mut orch = setup_orchestrator();

        let partials = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let partials_for_handler = Arc::clone(&partials);
        orch.on_intent_partial(Arc::new(move |name, value| {
            partials_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[schema: Output");

        let partials = partials.lock().unwrap().clone();
        assert!(
            partials.is_empty(),
            "Malformed schema header tails should not leak as response_text partials: {:?}",
            partials
        );
    }

    #[test]
    fn test_schema_only() {
        let mut orch = setup_orchestrator();

        let output = serde_json::json!({
            "name": { "type": "string" },
            "age": { "type": "number" }
        });
        orch.register_output_shape(&output, None);

        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        orch.write("[schema: Output]\nname: John\nage: 30\n[/schema]");
        orch.end();

        let final_emitted = emitted.lock().unwrap().clone();
        assert_eq!(final_emitted.len(), 1);
        assert_eq!(final_emitted[0].0, "response_schema");
        assert_eq!(final_emitted[0].1["type"].as_str().unwrap(), "Output");
    }

    #[test]
    fn test_incremental_streaming_then_finalize() {
        let mut orch = setup_orchestrator();

        let output = serde_json::json!({
            "name": { "type": "string" }
        });
        orch.register_output_shape(&output, None);

        let emitted = Arc::new(std::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let emitted_for_handler = Arc::clone(&emitted);
        orch.on_intent_ready(Arc::new(move |name, value| {
            emitted_for_handler.lock().unwrap().push((name, value));
        }));

        // Simulate incremental token streaming
        orch.write("[response_text]\nHello");
        orch.write(" world!\n[/response_text]\n[schema:");
        orch.write(" Output]\nname: Alice\n[/schema]");

        // Nothing emitted yet during streaming
        assert!(emitted.lock().unwrap().is_empty());

        // Finalize
        orch.end();

        let final_emitted = emitted.lock().unwrap().clone();
        let intent_names: Vec<&str> = final_emitted
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();

        assert!(
            intent_names.contains(&"response_text"),
            "Expected response_text, got: {:?}",
            intent_names
        );
        assert!(
            intent_names.contains(&"response_schema"),
            "Expected response_schema, got: {:?}",
            intent_names
        );
    }
}
