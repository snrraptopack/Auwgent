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
    /// Parser options (stored for glue heuristic configuration)
    options: ParserOptions,
}

impl Orchestrator {
    pub fn new(options: Option<ParserOptions>) -> Self {
        let opts = options.unwrap_or(ParserOptions {
            indent_size: None,
            allow_tabs: None,
            preserve_comments: None,
            strict: None,
            intent_schema: None,
            intent_key: None,
            middleware: None,
            enable_glue_heuristic: Some(true),
        });
        let parser = Parser::new(Some(opts.clone()));

        Self {
            parser,
            ir_builder: IRBuilder::new(),
            intent_keys: HashSet::new(),
            intent_handler: None,
            partial_handler: None,
            emitted_identities: HashSet::new(),
            options: opts,
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

    /// Configure the glue heuristic
    /// When enabled (default), orphaned keys that aren't registered intents will be merged into the previous intent.
    /// This helps handle LLM output that doesn't use pipe blocks for multiline content.
    /// When disabled, orphaned keys are ignored.
    pub fn set_glue_heuristic(&mut self, enabled: bool) {
        self.options.enable_glue_heuristic = Some(enabled);
    }

    /// Write chunk of input
    pub fn write(&mut self, chunk: &str) {
        // If we see triple backticks at the very beginning of the first chunk
        // or we are in a state where we might want to skip noise, we could.
        // But the current YAML parser is already somewhat robust to noise
        // if it doesn't look like YAML keys.
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
        if let Some(ast) = &res.ast {
            self.flush_remaining_intents(ast);
            let build = self.ir_builder.build(Some(ast));
            build.value.into_json()
        } else {
            Value::Null
        }
    }

    /// Emit any registered intents that were never fired during streaming
    fn flush_remaining_intents(&mut self, ast: &ASTNode) {
        match ast {
            ASTNode::Mapping(root_mapping) => {
                self.process_mapping_intents(root_mapping, true);
            }
            ASTNode::Sequence(root_sequence) => {
                for item in &root_sequence.items {
                    if let ASTNode::Mapping(mapping) = item {
                        self.process_mapping_intents(mapping, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn process_mapping_intents(&mut self, mapping: &MappingNode, is_final: bool) {
        let entries = &mapping.entries;
        for (idx, entry) in entries.iter().enumerate() {
            if !self.intent_keys.contains(&entry.key) {
                // ── HEURISTIC: Skip unknown root keys ──
                continue;
            }

            let identity = (entry.line, entry.column);
            if self.emitted_identities.contains(&identity) {
                continue; // already fired during streaming, skip
            }

            // During streaming, never emit the last entry as it might be incomplete.
            // When is_final is true (from flush_remaining_intents), we emit everything left.
            if !is_final && idx == entries.len() - 1 {
                continue;
            }

            self.emitted_identities.insert(identity);
            let mut build_result = self.ir_builder.build(Some(&entry.value));

            // ── HEURISTIC: "Glue" subsequent unmapped keys into this intent ──
            // If the next few entries in the mapping are NOT registered intents,
            // they are likely part of a broken multiline scalar (e.g. from an LLM that didn't use |).
            // We append them to the first string field we find in the intent.
            // This can be disabled via set_glue_heuristic(false).
            let glue_enabled = self.options.enable_glue_heuristic.unwrap_or(true);
            if glue_enabled {
                let intent_indent = entry.column;
                let mut next_idx = idx + 1;
                while next_idx < entries.len() {
                    let next_entry = &entries[next_idx];
                    if self.intent_keys.contains(&next_entry.key) {
                        break;
                    }
                    
                    // Improved heuristic: only merge keys at same indent level that immediately follow
                    // Don't merge if indent level is different (likely a nested structure)
                    if next_entry.column != intent_indent {
                        break;
                    }
                    
                    // This is an "orphaned" key/value. Glue it!
                    let orphaned_key = &next_entry.key;
                    
                    // Extract value, avoiding serialization of empty objects
                    let orphaned_val_str = match &next_entry.value {
                        ASTNode::Scalar(scalar) => scalar.value.clone(),
                        ASTNode::Empty(_) => String::new(),
                        ASTNode::Mapping(map) if map.entries.is_empty() => String::new(),
                        ASTNode::Sequence(seq) if seq.items.is_empty() => String::new(),
                        _ => {
                            // For complex values, build and serialize
                            let orphaned_val_res = self.ir_builder.build(Some(&next_entry.value));
                            match orphaned_val_res.value {
                                IRValue::String(s) => s,
                                IRValue::Null => String::new(),
                                IRValue::Object(map) if map.is_empty() => String::new(),
                                IRValue::Array(arr) if arr.is_empty() => String::new(),
                                other => serde_json::to_string(&other.into_json()).unwrap_or_default(),
                            }
                        }
                    };

                    // Detect if this looks like content (numbered list, bullet point) vs a real key
                    let looks_like_content = Self::is_content_pattern(orphaned_key);
                    
                    let glue = if looks_like_content {
                        // Content patterns: preserve as-is with minimal formatting
                        if orphaned_val_str.is_empty() {
                            format!("\n{}", orphaned_key)
                        } else {
                            format!("\n{}: {}", orphaned_key, orphaned_val_str)
                        }
                    } else {
                        // Traditional key-value: add extra spacing
                        if orphaned_val_str.is_empty() {
                            format!("\n\n{}", orphaned_key)
                        } else {
                            format!("\n\n{}: {}", orphaned_key, orphaned_val_str)
                        }
                    };

                    // Find a string field to append to
                    match build_result.value {
                        IRValue::Object(ref mut map) => {
                            let mut appended = false;
                            for target_key in &["text", "questions", "response", "prompts", "explain"] {
                                if let Some(IRValue::String(s)) = map.get_mut(*target_key) {
                                    s.push_str(&glue);
                                    appended = true;
                                    break;
                                }
                            }
                            if !appended {
                                // Fallback: first available string field
                                for (_, val) in map.iter_mut() {
                                    if let IRValue::String(s) = val {
                                        s.push_str(&glue);
                                        break;
                                    }
                                }
                            }
                        }
                        IRValue::String(ref mut s) => {
                            s.push_str(&glue);
                        }
                        _ => {}
                    }

                    self.emitted_identities.insert((next_entry.line, next_entry.column));
                    next_idx += 1;
                }
            }

            // Don't fire partial for final intents - only fire the ready handler
            // The partial handler is for streaming updates only
            if let Some(handler) = &self.intent_handler {
                let mut json_val = build_result.value.into_json();
                self.process_final_intent_value(&entry.key, &mut json_val, entry);
                handler(entry.key.clone(), json_val);
            }
        }
    }

    fn process_final_intent_value(&self, key: &str, json_val: &mut Value, entry: &MappingEntry) {
        // ── Fix multi-line text merging ──
        if key == "response_text" {
            if let Value::Object(map) = json_val {
                let main_key = "text";
                let extra_keys: Vec<(String, Value)> = map
                    .iter()
                    .filter(|(k, _)| k.as_str() != main_key)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                if !extra_keys.is_empty() {
                    let mut continuation = String::new();
                    for (k, v) in &extra_keys {
                        let val_str = match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            Value::Null => "null".to_string(),
                            other => serde_json::to_string(other).unwrap_or_default(),
                        };
                        continuation.push_str(&format!("\n{}: {}", k, val_str));
                    }

                    if let Some(Value::String(text)) = map.get_mut(main_key) {
                        text.push_str(&continuation);
                    }

                    for (k, _) in &extra_keys {
                        map.remove(k);
                    }
                }
            }
        }

        // ── Inject _raw for middleware logging/audit ──
        if let Value::Object(map) = json_val {
            let yaml_body = serde_yaml::to_string(&Value::Object(map.clone())).unwrap_or_default();
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
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.parser.reset();
        self.emitted_identities.clear();
    }

    /// Detect if a key looks like content (numbered list, bullet point) rather than a YAML key
    fn is_content_pattern(key: &str) -> bool {
        let trimmed = key.trim_start();
        
        // Numbered lists: "1.", "2)", "1.1", etc.
        if let Some(first_char) = trimmed.chars().next() {
            if first_char.is_numeric() {
                // Check if followed by . or ) or more numbers
                let after_num: String = trimmed.chars().skip_while(|c| c.is_numeric()).collect();
                if after_num.starts_with('.') || after_num.starts_with(')') {
                    return true;
                }
            }
        }
        
        // Roman numerals: "i.", "ii)", "I.", "IV.", etc.
        let roman_pattern = trimmed.chars().take_while(|c| {
            matches!(c.to_ascii_uppercase(), 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M')
        }).collect::<String>();
        if !roman_pattern.is_empty() {
            let after_roman = &trimmed[roman_pattern.len()..];
            if after_roman.starts_with('.') || after_roman.starts_with(')') {
                return true;
            }
        }
        
        // Bullet points: "- ", "* ", "• "
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• ") {
            return true;
        }
        
        // Markdown bold/italic at start: "**text", "*text", "__text"
        if trimmed.starts_with("**") || trimmed.starts_with("__") {
            return true;
        }
        
        false
    }

    /// Check for ready and partial intents
    fn check_intents(&mut self, _final_pass: bool) {
        // ── 1. Finished Intents (from root mapping or sequence) ──
        // Only emit entries that are CONFIRMED sealed by a successor entry.
        // The last entry is ALWAYS deferred to flush_remaining_intents() which
        // uses the finalized AST — never guess if it's complete mid-stream.
        if let Some(ast) = self.parser.peek().ast {
            match ast {
                ASTNode::Mapping(root_mapping) => {
                    self.process_mapping_intents(&root_mapping, false);
                }
                ASTNode::Sequence(root_sequence) => {
                    for item in &root_sequence.items {
                        if let ASTNode::Mapping(mapping) = item {
                            self.process_mapping_intents(mapping, false);
                        }
                    }
                }
                _ => {}
            }
        }

        // ── 2. Partial Intents (from stack) ──
        // Shows live typing progress to the UI while the intent is still being built.
        let stack = self.parser.stack();
        let partial_token = self.parser.get_partial_token();

        // ── BUG FIX: Emit partial for the top frame if it's an intent ──
        // The original logic only checked parents (i-1), missing the very field being typed.
        if let Some(top_frame) = stack.last() {
            if let Some(key) = &top_frame.pending_key {
                if self.intent_keys.contains(key) && !partial_token.is_empty() {
                    if let Some(handler) = &self.partial_handler {
                        handler(key.clone(), Value::String(partial_token.clone()));
                    }
                }
            }
        }

        for i in 1..stack.len() {
            let parent = &stack[i - 1];
            if let Some(key) = &parent.pending_key {
                if self.intent_keys.contains(key) {
                    if let Some(handler) = &self.partial_handler {
                        let mut node = stack[i].node.to_ast_node();

                        // If this is the active leaf and we have a partial token from the
                        // tokenizer, inject it so the UI sees the character being typed
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
    let mut blocks = Vec::new();
    let mut current_pos = 0;

    // We use a case-insensitive search for code fences to be more robust
    let lower_input = input.to_lowercase();

    while let Some(start_idx) = lower_input[current_pos..].find("```") {
        let abs_start = current_pos + start_idx;
        let rest = &input[abs_start + 3..];

        // Skip language tag if any
        let line_end = rest.find('\n').unwrap_or(rest.len());
        let tag = rest[..line_end].trim().to_lowercase();
        let content_start_offset = if tag == "yaml" || tag == "yml" || tag.is_empty() {
            line_end + 1
        } else {
            0 // Not a yaml block? Or just no newline after ```
        };

        let content_start = abs_start + 3 + content_start_offset;
        if content_start >= input.len() {
            break;
        }

        if let Some(end_idx) = lower_input[content_start..].find("```") {
            let abs_end = content_start + end_idx;
            blocks.push(input[content_start..abs_end].trim());
            current_pos = abs_end + 3;
        } else {
            // Unclosed block
            blocks.push(input[content_start..].trim());
            break;
        }
    }

    if blocks.is_empty() {
        // Fallback: if no fences, try to find the first line that looks like a YAML key:
        // Especially useful for models that output noise before the YAML
        let lines: Vec<&str> = input.lines().collect();
        let mut first_key_line = None;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.contains(':') && !trimmed.starts_with('#') {
                // Heuristic: line starts with a word-like thing followed by a colon
                let part = trimmed.split(':').next().unwrap_or("");
                if !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    first_key_line = Some(i);
                    break;
                }
            }
        }

        if let Some(i) = first_key_line {
            return lines[i..].join("\n").trim().to_string();
        }

        return input.trim().to_string();
    }

    blocks.join("\n\n")
}

use super::builder::IRBuilder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_yaml_user_failing_case() {
        let input = "```yaml\nresponse_schema:\n  response: |\n    Ghana became the first sub-Saharan African country to gain independence from colonial rule on 6 March 1957.\n\n    A short story:  \n    In the coastal town of Cape Coast, 27-year-old teacher Akosua Mensah woke before dawn on 5 March 1957. She pressed her best cotton dress, the one printed with the new red-gold-green flag, and walked with her father to the beach where fishermen were already singing freedom songs. All night, people poured into Accra’s Old Polo Grounds, carrying lanterns and homemade drums. When midnight struck, the Union Jack was lowered and the Ghanaian flag rose for the first time; the crowd roared so loudly that Akosua felt the sound in her ribs. Dr. Kwame Nkrumah stepped to the microphone and declared, “The African is capable of managing his own affairs,” and in that moment fishermen, market women, clerks, and schoolchildren believed it. Akosua’s father lifted her onto his shoulders so she could see history; she later told every class she taught that freedom feels like salt wind and drumbeats and the taste of tears that are half joy, half disbelief.\n  score: 0.95\n```";
        let cleaned = extract_yaml(input);
        assert!(!cleaned.contains("```yaml"));
        assert!(!cleaned.contains("```"));
        assert!(cleaned.starts_with("response_schema:"));
    }

    #[test]
    fn test_orchestrator_sequence_root() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "- response_schema:\n    response: \"Hello\"\n    score: 0.95";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(final_intents.len(), 1);
        assert_eq!(final_intents[0].0, "response_schema");
    }

    #[test]
    fn test_orchestrator_user_intermittent_failing_case() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "response_schema:\n  response: |\n    Ghana gained its independence from British colonial rule on March 6, 1957. \n    The story of that historic day is one of hope, determination, and the collective dream of a people yearning for freedom.\n\n    In the early 1950s, a charismatic leader named Dr. Kwame Nkrumah rose to prominence. He founded the Convention People's Party (CPP) and championed the slogan “Self‑government now!” Nkrumah traveled across the Gold Coast (the name of Ghana then) rallying crowds, singing songs of liberation, and demanding an end to colonial domination.\n\n    The British administration, feeling the pressure of a growing nationalist movement, tried to negotiate a gradual transition. However, Nkrumah and his supporters were steadfast: they wanted full sovereignty without delay. Massive peaceful protests, strikes, and a wave of civil disobedience swept the towns and villages. The most famous was the “Positive Action” campaign of 1950, where students, workers, and farmers united in a non‑violent push for independence.\n\n    By 1956, the British government recognized that the tide could not be turned back. A constitutional conference in London led to an agreement that the Gold Coast would become an independent nation. The night before the declaration, the streets of Accra glowed with lanterns, drums, and the rhythmic chants of “Ghana! Ghana!” Families gathered, children waved small flags, and elders recited prayers for the new nation’s future.\n\n    On the morning of March 6, 1957, a jubilant crowd assembled at the Black Star Square (now Independence Square). Dr. Nkrumah, dressed in a ceremonial red robe, raised the newly designed Ghanaian flag—a red, gold, and green banner with a solitary black star at its center. As the anthem “God Bless Our Homeland Ghana” resonated, the crowd sang in unison, tears of joy streaming down faces. The moment marked not just political independence but the birth of a hopeful vision: a united, prosperous Africa led by a free Ghana.\n\n    That day, Ghana’s independence inspired many other African nations to pursue their own liberation, earning Ghana the nickname “the beacon of Africa.” Today, every March 6th, Ghanaians celebrate with parades, cultural performances, and reflections on the journey from colonial rule to self‑determination—reminding the world that the spirit of freedom, once ignited, can illuminate an entire continent.\n  score: 1.0\n";

        // Use small chunks to stress test streaming boundary robustness
        for chunk in input.as_bytes().chunks(5) {
            orchestrator.write(&String::from_utf8_lossy(chunk));
        }

        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(final_intents.len(), 1, "Intent should have been emitted");
        assert_eq!(final_intents[0].0, "response_schema");

        let val = &final_intents[0].1;
        println!("Value: {}", serde_json::to_string_pretty(val).unwrap());
        assert!(val.get("response").is_some());
        assert_eq!(val.get("score").and_then(|v| v.as_f64()), Some(1.0));
    }

    #[test]
    fn test_orchestrator_noise_before_yaml() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "Here is the response:\n\nresponse_schema:\n  response: |\n    This is the data.\n  score: 1.0";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(
            final_intents.len(),
            1,
            "Intent should have been emitted even with noise before it"
        );
        assert_eq!(final_intents[0].0, "response_schema");
    }

    #[test]
    fn test_orchestrator_noise_with_hyphen() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "The model says - let's see:\n\nresponse_schema:\n  response: |\n    This is the data.\n  score: 1.0";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(
            final_intents.len(),
            1,
            "Intent should have been emitted even with noisy hyphens before it"
        );
        assert_eq!(final_intents[0].0, "response_schema");
    }

    #[test]
    fn test_orchestrator_noise_with_hyphen_space() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "- Here is the response:\n\nresponse_schema:\n  response: |\n    This is the data.\n  score: 1.0";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(
            final_intents.len(),
            1,
            "Intent should have been emitted even with noisy root sequence before it"
        );
        assert_eq!(final_intents[0].0, "response_schema");
    }

    #[test]
    fn test_orchestrator_noise_with_hyphen_space_at_same_indent() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "- some noise\n- response_schema:\n    response: |\n      This is the data.\n    score: 1.0";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(
            final_intents.len(),
            1,
            "Intent should have been emitted even with noisy items in same root sequence"
        );
        assert_eq!(final_intents[0].0, "response_schema");
    }

    #[test]
    fn test_orchestrator_noise_with_key_value() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("response_schema");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "noise_key: noise_value\nresponse_schema:\n  response: |\n    This is the data.\n  score: 1.0";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(
            final_intents.len(),
            1,
            "Intent should have been emitted even with noisy keys at root"
        );
        assert_eq!(final_intents[0].0, "response_schema");
    }


    #[test]
    fn test_extract_yaml_mixed_noise() {
        let input = "Sure, I can help with that!\n\n```yaml\nintent1: value1\n```\nAnd here is more noise:\nintent2: value2\n\n```yaml\nintent3: value3\n```";
        let cleaned = extract_yaml(input);
        // Should join multiple yaml blocks
        assert!(cleaned.contains("intent1: value1"));
        assert!(cleaned.contains("intent3: value3"));
        // Since intent2 is between fences, it might be skipped if we only take fences.
        // But extract_yaml takes ALL fences and joins them.
        assert!(!cleaned.contains("intent2: value2"));
    }

    #[test]
    fn test_extract_yaml_no_fences_noise_before() {
        let input = "I am a helpful assistant.\n\nresponse_schema:\n  score: 1.0";
        let cleaned = extract_yaml(input);
        assert_eq!(cleaned, "response_schema:\n  score: 1.0");
    }

    #[test]
    fn test_orchestrator_sequence_root_multiple() {
        let mut orchestrator = Orchestrator::new(None);
        orchestrator.register_intent("intent1");
        orchestrator.register_intent("intent2");

        let intents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intents_clone = intents.clone();
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            intents_clone.lock().unwrap().push((name, value));
        }));

        let input = "- intent1: value1\n- intent2: value2";
        orchestrator.write(input);
        orchestrator.end();

        let final_intents = intents.lock().unwrap();
        assert_eq!(final_intents.len(), 2);
        assert_eq!(final_intents[0].0, "intent1");
        assert_eq!(final_intents[1].0, "intent2");
    }
}
