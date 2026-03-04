use super::parser::Parser;
use super::types::*;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

/// Handler for intent events
pub type IntentHandler = Arc<dyn Fn(String, Value) + Send + Sync>;

pub struct Orchestrator {
    parser: Parser,
    ir_builder: IRBuilder,
    /// Registered intent keys
    intent_keys: HashSet<String>,
    /// Callback for finished intents
    intent_handler: Option<IntentHandler>,
    /// Callback for partial updates
    partial_handler: Option<IntentHandler>,
    /// Set of (line, col) for intents already emitted as "ready"
    emitted_identities: HashSet<(usize, usize)>,
}

impl Orchestrator {
    pub fn new(options: Option<ParserOptions>) -> Self {
        let parser = Parser::new(options);

        Self {
            parser,
            ir_builder: IRBuilder::new(),
            intent_keys: HashSet::new(),
            intent_handler: None,
            partial_handler: None,
            emitted_identities: HashSet::new(),
        }
    }

    /// Register a key as an intent
    pub fn register_intent(&mut self, key: &str) {
        self.intent_keys.insert(key.to_string());
    }

    /// Set handler for finished intents
    pub fn on_intent_ready(&mut self, handler: IntentHandler) {
        self.intent_handler = Some(handler);
    }

    /// Set handler for partial intent updates
    pub fn on_intent_partial(&mut self, handler: IntentHandler) {
        self.partial_handler = Some(handler);
    }

    /// Write chunk of input
    pub fn write(&mut self, chunk: &str) {
        self.parser.write(chunk);
        self.check_intents(false);
    }

    /// Peek at current state
    pub fn peek(&mut self) -> Value {
        self.parser.peek();
        self.check_intents(false);

        let res = self.parser.peek();
        if let Some(ast) = res.ast {
            let build = self.ir_builder.build(Some(&ast));
            build.value.into_json()
        } else {
            Value::Null
        }
    }

    /// End parsing
    pub fn end(&mut self) -> Value {
        let res = self.parser.end();
        self.check_intents(true);

        if let Some(ast) = res.ast {
            let build = self.ir_builder.build(Some(&ast));
            build.value.into_json()
        } else {
            Value::Null
        }
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.parser.reset();
        self.emitted_identities.clear();
    }

    /// Check for ready and partial intents
    fn check_intents(&mut self, _final_pass: bool) {
        // 1. Finished Intents (from root mapping)
        if let Some(root_mapping) = self.parser.get_root_mapping() {
            // Clone entries to avoid borrow issues while potentially calling handlers
            let entries = root_mapping.entries.clone();
            for entry in entries {
                if !self.intent_keys.contains(&entry.key) {
                    continue;
                }

                let identity = (entry.line, entry.column);
                if !self.emitted_identities.contains(&identity) {
                    self.emitted_identities.insert(identity);
                    let build_result = self.ir_builder.build(Some(&entry.value));

                    // Emit a final partial event so UI hits 100% and catches fast chunks
                    if let Some(handler) = &self.partial_handler {
                        handler(entry.key.clone(), build_result.value.clone().into_json());
                    }

                    if let Some(handler) = &self.intent_handler {
                        let mut json_val = build_result.value.into_json();

                        // ── Fix multi-line text merging ──
                        // When the LLM outputs response_text with indented continuation
                        // lines (e.g. "text: Hello:\n    Name: Babyface\n    Age: 22"),
                        // the YAML parser treats the indented lines as sibling keys
                        // instead of multi-line text. Merge extra keys back into `text`.
                        if entry.key == "response_text" || entry.key == "response_schema" {
                            if let Value::Object(ref mut map) = json_val {
                                let main_key = if entry.key == "response_text" {
                                    "text"
                                } else {
                                    "data"
                                };
                                // Collect extra keys (anything that isn't the main key)
                                let extra_keys: Vec<(String, Value)> = map
                                    .iter()
                                    .filter(|(k, _)| k.as_str() != main_key)
                                    .map(|(k, v)| (k.clone(), v.clone()))
                                    .collect();

                                if !extra_keys.is_empty() {
                                    // Build continuation text from extra keys
                                    let mut continuation = String::new();
                                    for (k, v) in &extra_keys {
                                        let val_str = match v {
                                            Value::String(s) => s.clone(),
                                            Value::Number(n) => n.to_string(),
                                            Value::Bool(b) => b.to_string(),
                                            Value::Null => "null".to_string(),
                                            other => {
                                                serde_json::to_string(other).unwrap_or_default()
                                            }
                                        };
                                        continuation.push_str(&format!("\n{}: {}", k, val_str));
                                    }

                                    // Append continuation to main text
                                    if let Some(Value::String(text)) = map.get_mut(main_key) {
                                        text.push_str(&continuation);
                                    }

                                    // Remove the extra keys from the map
                                    for (k, _) in &extra_keys {
                                        map.remove(k);
                                    }
                                }
                            }
                        }

                        // Inject _raw: readable representation for middleware logging/audit
                        if let Value::Object(ref mut map) = json_val {
                            let yaml_body = serde_yaml::to_string(&Value::Object(map.clone()))
                                .unwrap_or_default();
                            let trimmed = yaml_body.trim();
                            let content = trimmed.strip_prefix("---\n").unwrap_or(trimmed);
                            let indented: String = content
                                .lines()
                                .map(|line| format!("  {}", line))
                                .collect::<Vec<_>>()
                                .join("\n");
                            let raw = format!("{}:\n{}", entry.key, indented);
                            map.insert("_raw".to_string(), Value::String(raw));
                        }

                        handler(entry.key.clone(), json_val);
                    }
                }
            }
        }

        // 2. Partial Intents (from stack)
        // We look at the stack to see what's currently being built.
        // If stack[i-1] has a pending_key that matches an intent, then stack[i].node is its partial value.
        let stack = self.parser.stack();
        let partial_token = self.parser.get_partial_token();
        for i in 1..stack.len() {
            let parent = &stack[i - 1];
            if let Some(key) = &parent.pending_key {
                if self.intent_keys.contains(key) {
                    if let Some(handler) = &self.partial_handler {
                        // Clone the node from stack to build partial IR
                        let mut node = stack[i].node.to_ast_node();

                        // If this is the active leaf node and we have a partial token from the tokenizer,
                        // forcibly inject it so the UI sees the live typing
                        if i == stack.len() - 1 && !partial_token.is_empty() {
                            match &mut node {
                                super::types::ASTNode::Mapping(map) => {
                                    if let Some(pk) = &stack[i].pending_key {
                                        map.entries.push(super::types::MappingEntry {
                                            key: pk.clone(),
                                            value: super::types::ASTNode::Scalar(
                                                super::types::ScalarNode {
                                                    kind: "scalar".to_string(),
                                                    value: partial_token.clone(),
                                                    quoted: false,
                                                    line: 0,
                                                    column: 0,
                                                },
                                            ),
                                            line: 0,
                                            column: 0,
                                        });
                                    }
                                }
                                super::types::ASTNode::Sequence(seq) => {
                                    seq.items.push(super::types::ASTNode::Scalar(
                                        super::types::ScalarNode {
                                            kind: "scalar".to_string(),
                                            value: partial_token.clone(),
                                            quoted: false,
                                            line: 0,
                                            column: 0,
                                        },
                                    ));
                                }
                                super::types::ASTNode::Scalar(scalar) => {
                                    scalar.value = partial_token.clone();
                                }
                                _ => {}
                            }
                        }

                        let build_result = self.ir_builder.build(Some(&node));
                        handler(key.clone(), build_result.value.into_json());
                    }
                }
            }
        }
    }
}

/// Helper to extract YAML from LLM output (noisy markdown)
pub fn extract_yaml(input: &str) -> String {
    if !input.contains("```") {
        return input.to_string();
    }

    if let Some(start_idx) = input.find("```") {
        let rest = &input[start_idx + 3..];
        // Skip language tag if any
        let content_start = if rest.starts_with("yaml") {
            rest[4..].find('\n').map(|i| 4 + i + 1).unwrap_or(4)
        } else if rest.starts_with("yml") {
            rest[3..].find('\n').map(|i| 3 + i + 1).unwrap_or(3)
        } else {
            rest.find('\n').map(|i| i + 1).unwrap_or(0)
        };

        let actual_content = &rest[content_start..];
        if let Some(end_idx) = actual_content.find("```") {
            return actual_content[..end_idx].trim().to_string();
        }
        return actual_content.trim().to_string();
    }

    input.to_string()
}

use super::builder::IRBuilder;
