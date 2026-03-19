use super::types::*;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// TYPE COERCION
// ═══════════════════════════════════════════════════════════════════════════

/// Coerce a string value to appropriate type
pub fn coerce_value(value: &str, quoted: bool) -> IRValue {
    // Quoted strings stay as strings
    if quoted {
        return IRValue::String(value.to_string());
    }

    // Trim for comparison
    let trimmed = value.trim();

    // Null - but NOT empty string
    if trimmed.eq_ignore_ascii_case("null") || trimmed == "~" {
        return IRValue::Null;
    }

    // Empty string
    if trimmed.is_empty() && value.is_empty() {
        return IRValue::String("".to_string());
    }

    // Boolean
    if trimmed.eq_ignore_ascii_case("true") {
        return IRValue::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return IRValue::Boolean(false);
    }
    
    // YAML 1.1 Boolean variants
    if trimmed.eq_ignore_ascii_case("yes") || trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("on") {
        return IRValue::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("no") || trimmed.eq_ignore_ascii_case("n") || trimmed.eq_ignore_ascii_case("off") {
        return IRValue::Boolean(false);
    }

    // Number (integer)
    if let Ok(num) = trimmed.parse::<i64>() {
        return IRValue::Number(num as f64);
    }

    // Number (float/scientific)
    if let Ok(num) = trimmed.parse::<f64>() {
        return IRValue::Number(num);
    }

    // Inline JSON array
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if val.is_array() {
                return json_to_ir(val);
            }
        }
    }

    // Inline object
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Try JSON first
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if val.is_object() {
                return json_to_ir(val);
            }
        }

        // Try YAML-style flow object
        if let Some(map) = parse_yaml_flow_object(trimmed) {
            return IRValue::Object(map);
        }
    }

    // Default to string
    IRValue::String(value.to_string())
}

fn json_to_ir(val: serde_json::Value) -> IRValue {
    match val {
        serde_json::Value::Null => IRValue::Null,
        serde_json::Value::Bool(b) => IRValue::Boolean(b),
        serde_json::Value::Number(n) => IRValue::Number(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => IRValue::String(s),
        serde_json::Value::Array(a) => IRValue::Array(a.into_iter().map(json_to_ir).collect()),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, v) in o {
                map.insert(k, json_to_ir(v));
            }
            IRValue::Object(map)
        }
    }
}

/// Parse YAML-style flow object: { key: value, key2: "quoted" }
pub fn parse_yaml_flow_object(input: &str) -> Option<HashMap<String, IRValue>> {
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();

    // Check braces
    if chars.len() < 2 || chars[0] != '{' || chars[chars.len() - 1] != '}' {
        return None;
    }

    let mut result = HashMap::new();
    i += 1; // Skip {

    while i < chars.len() - 1 {
        // Skip whitespace
        while i < chars.len() - 1 && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() - 1 {
            break;
        }

        // Parse key
        let mut key = String::new();
        let c = chars[i];
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            while i < chars.len() - 1 && chars[i] != quote {
                key.push(chars[i]);
                i += 1;
            }
            if i < chars.len() - 1 {
                i += 1;
            }
        } else {
            while i < chars.len() - 1
                && !chars[i].is_whitespace()
                && chars[i] != ':'
                && chars[i] != ','
                && chars[i] != '}'
            {
                key.push(chars[i]);
                i += 1;
            }
        }

        if key.is_empty() {
            break;
        }

        // Skip to colon
        while i < chars.len() - 1 && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() - 1 || chars[i] != ':' {
            return None;
        }
        i += 1;
        while i < chars.len() - 1 && chars[i].is_whitespace() {
            i += 1;
        }

        // Parse value
        let value = if i < chars.len() - 1 {
            let val_char = chars[i];
            if val_char == '"' || val_char == '\'' {
                let quote = val_char;
                i += 1;
                let mut str_val = String::new();
                while i < chars.len() - 1 && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < chars.len() - 1 {
                        i += 1;
                        str_val.push(chars[i]);
                    } else {
                        str_val.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() - 1 {
                    i += 1;
                }
                IRValue::String(str_val)
            } else if val_char == '{' {
                let mut depth = 1;
                let mut nested = String::from("{");
                i += 1;
                while i < chars.len() - 1 && depth > 0 {
                    if chars[i] == '{' {
                        depth += 1;
                    }
                    if chars[i] == '}' {
                        depth -= 1;
                    }
                    nested.push(chars[i]);
                    i += 1;
                }
                if let Some(map) = parse_yaml_flow_object(&nested) {
                    IRValue::Object(map)
                } else {
                    return None;
                }
            } else if val_char == '[' {
                let mut depth = 1;
                let mut arr_str = String::from("[");
                i += 1;
                while i < chars.len() - 1 && depth > 0 {
                    if chars[i] == '[' {
                        depth += 1;
                    }
                    if chars[i] == ']' {
                        depth -= 1;
                    }
                    arr_str.push(chars[i]);
                    i += 1;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&arr_str) {
                    json_to_ir(val)
                } else {
                    IRValue::String(arr_str)
                }
            } else {
                let mut raw = String::new();
                while i < chars.len() - 1 && chars[i] != ',' && chars[i] != '}' {
                    raw.push(chars[i]);
                    i += 1;
                }
                coerce_value(&raw, false)
            }
        } else {
            IRValue::Null
        };

        result.insert(key, value);
        while i < chars.len() - 1 && chars[i].is_whitespace() {
            i += 1;
        }
        if i < chars.len() - 1 && chars[i] == ',' {
            i += 1;
        }
    }

    Some(result)
}

// ═══════════════════════════════════════════════════════════════════════════
// IR BUILDER CLASS
// ═══════════════════════════════════════════════════════════════════════════

pub struct IRBuilder {
    registry: HashMap<String, IRValue>,
    unresolved_refs: Vec<String>,
    errors: Vec<IRError>,
    current_path: Vec<String>,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
            current_path: Vec::new(),
        }
    }

    /// Build IR from AST
    pub fn build(&mut self, ast: Option<&ASTNode>) -> IRResult {
        self.registry.clear();
        self.unresolved_refs.clear();
        self.errors.clear();
        self.current_path.clear();

        let mut value = if let Some(node) = ast {
            self.transform_node(node)
        } else {
            IRValue::Null
        };

        // Resolve references (inline resolved values)
        if !matches!(value, IRValue::Null) {
            self.resolve_refs(&mut value);
            // Lift ref-only objects
            value = self.lift_ref_only_objects(value);
        }

        IRResult {
            value,
            registry: self.registry.clone(),
            unresolved_refs: self.unresolved_refs.clone(),
            errors: self.errors.clone(),
        }
    }

    /// Transform an AST node to IR
    fn transform_node(&mut self, node: &ASTNode) -> IRValue {
        match node {
            ASTNode::Scalar(n) => self.transform_scalar(n),
            ASTNode::Mapping(n) => self.transform_mapping(n),
            ASTNode::Sequence(n) => self.transform_sequence(n),
            ASTNode::Ref(n) => self.transform_ref(n),
            ASTNode::Empty(n) => self.transform_empty(n),
        }
    }

    fn transform_scalar(&mut self, node: &ScalarNode) -> IRValue {
        coerce_value(&node.value, node.quoted)
    }

    fn transform_mapping(&mut self, node: &MappingNode) -> IRValue {
        let mut obj = HashMap::new();
        let mut node_id: Option<String> = None;

        for entry in &node.entries {
            self.current_path.push(entry.key.clone());

            let val = self.transform_node(&entry.value);
            obj.insert(entry.key.clone(), val.clone());

            // Track id for registry
            if entry.key == "id" {
                if let IRValue::String(s) = val {
                    node_id = Some(s);
                }
            }

            self.current_path.pop();
        }

        let ir_obj = IRValue::Object(obj);

        // Register node if it has an id
        if let Some(id) = node_id {
            self.registry.insert(id, ir_obj.clone());
        }

        ir_obj
    }

    fn transform_sequence(&mut self, node: &SequenceNode) -> IRValue {
        let mut arr = Vec::new();

        for (i, item) in node.items.iter().enumerate() {
            self.current_path.push(format!("[{}]", i));
            arr.push(self.transform_node(item));
            self.current_path.pop();
        }

        IRValue::Array(arr)
    }

    fn transform_ref(&mut self, node: &RefNode) -> IRValue {
        IRValue::Ref(IRRef {
            reference: node.target.clone(),
        })
    }

    fn transform_empty(&mut self, node: &EmptyNode) -> IRValue {
        if node.hint.as_deref() == Some("sequence") {
            IRValue::Array(Vec::new())
        } else {
            IRValue::Object(HashMap::new())
        }
    }

    /// Resolve $ref placeholders in the IR by inlining resolved values
    fn resolve_refs(&mut self, value: &mut IRValue) {
        match value {
            IRValue::Object(map) => {
                let keys: Vec<String> = map.keys().cloned().collect();
                for key in keys {
                    let child = map.get_mut(&key).unwrap();
                    if let IRValue::Ref(r) = child {
                        if let Some(resolved) = self.registry.get(&r.reference) {
                            *child = resolved.clone();
                        } else {
                            self.unresolved_refs.push(r.reference.clone());
                        }
                    } else {
                        self.resolve_refs(child);
                    }
                }
            }
            IRValue::Array(arr) => {
                for i in 0..arr.len() {
                    let child = &mut arr[i];

                    // Auto-resolve string refs in arrays: if string matches registry, inline it
                    if let IRValue::String(s) = child {
                        if let Some(resolved) = self.registry.get(s) {
                            *child = resolved.clone();
                            continue;
                        }
                    }

                    if let IRValue::Ref(r) = child {
                        if let Some(resolved) = self.registry.get(&r.reference) {
                            *child = resolved.clone();
                        } else {
                            self.unresolved_refs.push(r.reference.clone());
                        }
                    } else {
                        self.resolve_refs(child);
                    }
                }
            }
            _ => {}
        }
    }

    /// Lift ref-only objects: replace {ref: val} with val
    fn lift_ref_only_objects(&mut self, value: IRValue) -> IRValue {
        match value {
            IRValue::Object(mut map) => {
                if map.len() == 1 && map.contains_key("ref") {
                    let ref_val = map.remove("ref").unwrap();
                    return self.lift_ref_only_objects(ref_val);
                }

                let mut processed = HashMap::new();
                for (k, v) in map {
                    processed.insert(k, self.lift_ref_only_objects(v));
                }
                IRValue::Object(processed)
            }
            IRValue::Array(arr) => IRValue::Array(
                arr.into_iter()
                    .map(|v| self.lift_ref_only_objects(v))
                    .collect(),
            ),
            _ => value,
        }
    }

    #[allow(dead_code)]
    fn add_error(&mut self, message: String) {
        self.errors.push(IRError {
            message,
            severity: ErrorSeverity::Error,
            path: self.current_path.clone(),
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONVENIENCE FUNCTION
// ═══════════════════════════════════════════════════════════════════════════

pub fn build_ir(ast: Option<&ASTNode>) -> IRResult {
    let mut builder = IRBuilder::new();
    builder.build(ast)
}
