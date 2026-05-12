use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// ═══════════════════════════════════════════════════════════════════════════
// NATIVE TURN TYPES — provider-native tool calling state
// ═══════════════════════════════════════════════════════════════════════════

/// A recorded native tool/function call within an assistant turn.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NativeToolCallRecord {
    pub id: Option<String>,
    pub provider_name: String,
    pub canonical_name: String,
    pub action_kind: String,
    pub arguments: Value,
}

/// A recorded native tool/function result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NativeToolResult {
    pub call_id: Option<String>,
    pub provider_name: String,
    pub canonical_name: String,
    pub action_kind: String,
    pub arguments: Value,
    pub result: Value,
}

/// The assistant's response in native mode.
///
/// Stores text content, tool calls, and structured output separately
/// from the raw text response so provider message reconstruction is accurate.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NativeAssistantTurn {
    pub text_content: Option<String>,
    pub tool_calls: Vec<NativeToolCallRecord>,
    pub structured_output: Option<Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// MESSAGE TYPES — used by drivers and session history
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Model,
    /// Tool results being fed back to the model
    ToolResult,
}

/// A single message in the conversation history.
/// This is the unit passed to `ModelDriver::stream_generate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    /// OpenAI-style tool calls on assistant messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    /// OpenAI-style tool call ID on tool result messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Function/provider name for tool result messages (used by Gemini native mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<Value>),
}

impl MessageContent {
    pub fn text(&self) -> String {
        match self {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => display_input_parts(parts),
        }
    }

    pub fn parts(&self) -> Option<&[Value]> {
        match self {
            MessageContent::Parts(parts) => Some(parts),
            MessageContent::Text(_) => None,
        }
    }
}

impl PartialEq<&str> for MessageContent {
    fn eq(&self, other: &&str) -> bool {
        self.text() == *other
    }
}

impl PartialEq<String> for MessageContent {
    fn eq(&self, other: &String) -> bool {
        self.text() == *other
    }
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user_parts(parts: Vec<Value>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Parts(parts),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn model(content: impl Into<String>) -> Self {
        Self {
            role: Role::Model,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn model_with_tool_calls(content: impl Into<String>, tool_calls: Vec<Value>) -> Self {
        Self {
            role: Role::Model,
            content: MessageContent::Text(content.into()),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result(content: impl Into<String>) -> Self {
        Self {
            role: Role::ToolResult,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result_with_id(content: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            role: Role::ToolResult,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: Some(id.into()),
            name: None,
        }
    }

    pub fn tool_result_native(
        content: impl Into<String>,
        id: Option<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::ToolResult,
            content: MessageContent::Text(content.into()),
            tool_calls: None,
            tool_call_id: id,
            name: Some(name.into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TURN — groups a single model interaction in the agentic loop
// ═══════════════════════════════════════════════════════════════════════════

/// A single turn in the agentic loop. One turn = one model call + response.
/// Tool/workflow results are tracked via the onIntent callback, not stored here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// The user/tool-result input that triggered this turn
    pub input: String,
    /// Structured multimodal input parts used to rebuild provider messages.
    #[serde(
        default,
        rename = "inputParts",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_parts: Option<Vec<Value>>,
    /// The raw model response (block mode) or assistant text (native mode)
    pub model_response: String,
    /// Protocol used for this turn: "block" or "native"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// Native assistant turn data (tool calls, structured output)
    #[serde(
        default,
        rename = "nativeAssistantTurn",
        skip_serializing_if = "Option::is_none"
    )]
    pub native_assistant_turn: Option<NativeAssistantTurn>,
    /// Native tool results for this turn
    #[serde(
        default,
        rename = "nativeToolResults",
        skip_serializing_if = "Option::is_none"
    )]
    pub native_tool_results: Option<Vec<NativeToolResult>>,
}

impl Turn {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            input_parts: None,
            model_response: String::new(),
            protocol: None,
            native_assistant_turn: None,
            native_tool_results: None,
        }
    }

    pub fn with_parts(input: impl Into<String>, parts: Vec<Value>) -> Self {
        Self {
            input: input.into(),
            input_parts: Some(parts),
            model_response: String::new(),
            protocol: None,
            native_assistant_turn: None,
            native_tool_results: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SESSION STATE — the temporal conversation state
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingCursor {
    pub turn_index: Option<usize>,
    pub role: String,
    pub input: Option<String>,
}

/// Temporal conversation state for the engine. This provides in-engine
/// memory that can be:
/// - Exported to JSON for the host runtime to persist (e.g. to a DB via hooks)
/// - Imported from JSON to restore a session
/// - Used to build message history for multi-turn conversations
///
/// The host runtime is responsible for long-term persistence;
/// this is the temporal working memory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// The system prompt for this session
    pub system_prompt: Option<String>,
    /// Ordered list of turns in this session
    pub turns: Vec<Turn>,
    /// The current execution stack (agent names)
    pub stack: Vec<String>,
    /// The initial input that started this session (for structured scope)
    pub initial_input: Option<Value>,
    /// Runtime-rendered binding cursor preview. Bindings are not stored as turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_cursor: Option<BindingCursor>,
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = Some(prompt.into());
    }

    /// Start a new turn with the given input
    pub fn start_turn(&mut self, input: impl Into<String>) {
        self.turns.push(Turn::new(input));
    }

    pub fn start_turn_parts(&mut self, input: impl Into<String>, parts: Vec<Value>) {
        self.turns.push(Turn::with_parts(input, parts));
    }

    /// Set the input on the current turn
    pub fn set_input(&mut self, input: impl Into<String>) {
        if let Some(turn) = self.turns.last_mut() {
            turn.input = input.into();
            turn.input_parts = None;
        }
    }

    /// Set the display input on the current turn without discarding any
    /// structured multimodal parts already attached to that turn.
    pub fn set_display_input(&mut self, input: impl Into<String>) {
        if let Some(turn) = self.turns.last_mut() {
            turn.input = input.into();
        }
    }

    /// Get the current (most recent) turn mutably
    pub fn current_turn_mut(&mut self) -> Option<&mut Turn> {
        self.turns.last_mut()
    }

    /// Pop the last turn if it has no input and no model response.
    /// Used by forceStart to clean up failed turn state.
    pub fn pop_last_turn_if_empty(&mut self) {
        if let Some(turn) = self.turns.last() {
            if turn.input.is_empty() && turn.model_response.is_empty() {
                self.turns.pop();
            }
        }
    }

    /// Set the model response on the current turn
    pub fn set_model_response(&mut self, response: impl Into<String>) {
        if let Some(turn) = self.turns.last_mut() {
            turn.model_response = response.into();
        }
    }

    /// Mark the current turn as using a specific protocol.
    /// Internal — called by the runtime loop based on IR `toolProtocol`.
    pub(crate) fn set_turn_protocol(&mut self, protocol: impl Into<String>) {
        if let Some(turn) = self.turns.last_mut() {
            turn.protocol = Some(protocol.into());
        }
    }

    /// Set native assistant turn data (tool calls, structured output) on the current turn.
    /// Internal — called by the runtime loop during native mode streaming.
    pub(crate) fn set_native_assistant_turn(&mut self, nat: NativeAssistantTurn) {
        if let Some(turn) = self.turns.last_mut() {
            turn.native_assistant_turn = Some(nat);
        }
    }

    /// Append a native tool result to the current turn.
    /// Internal — called by the runtime loop after native tool execution.
    pub(crate) fn append_native_tool_result(&mut self, result: NativeToolResult) {
        if let Some(turn) = self.turns.last_mut() {
            turn.native_tool_results
                .get_or_insert_with(Vec::new)
                .push(result);
        }
    }

    /// Build the full message history for the model driver.
    /// This reconstructs the conversation from all turns.
    pub fn to_messages(&self) -> Vec<Message> {
        self.to_messages_with_bindings(None)
    }

    /// Build message history with a runtime-managed binding block.
    ///
    /// Binding messages are render-time artifacts: they are sent to the model
    /// but are not stored in `turns`, so exported sessions remain clean and can
    /// reconstruct the cursor position from normal turn history.
    pub fn to_messages_with_bindings(&self, bindings: Option<String>) -> Vec<Message> {
        let mut messages = Vec::new();
        let binding_cursor = bindings
            .as_ref()
            .filter(|block| !block.trim().is_empty())
            .and_then(|_| self.binding_cursor_turn_index());

        // System prompt
        if let Some(ref prompt) = self.system_prompt {
            messages.push(Message::system(prompt.clone()));
        }

        // Each turn contributes user + model messages
        for (index, turn) in self.turns.iter().enumerate() {
            if binding_cursor == Some(index)
                && let Some(block) = bindings.as_ref()
            {
                messages.push(Message::user(block.clone()));
            }

            // The input for this turn
            if let Some(parts) = turn.input_parts.clone() {
                messages.push(Message::user_parts(parts));
            } else {
                messages.push(Message::user(turn.input.clone()));
            }

            // The model response
            if !turn.model_response.is_empty() {
                messages.push(Message::model(turn.model_response.clone()));
            } else {
                // Models like Gemini fail on consecutive User messages.
                // If the model response was empty, we inject a placeholder
                // to maintain alternating roles.
                messages.push(Message::model("(no response)"));
            }
        }

        messages
    }

    /// Build the full message history for the OpenAI driver in native mode.
    ///
    /// Reconstructs the conversation with OpenAI-style `tool_calls` on assistant
    /// messages and `role: "tool"` / `tool_call_id` on tool result messages.
    /// This enables proper round-trip native tool calling with OpenAI models.
    ///
    /// Each turn becomes:
    ///   - user message (input)
    ///   - assistant message (with optional tool_calls)
    ///   - tool result message(s) (if any)
    pub fn to_messages_native_openai(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // System prompt
        if let Some(ref prompt) = self.system_prompt {
            messages.push(Message::system(prompt.clone()));
        }

        for (turn_idx, turn) in self.turns.iter().enumerate() {
            // User input
            if let Some(parts) = turn.input_parts.clone() {
                messages.push(Message::user_parts(parts));
            } else {
                messages.push(Message::user(turn.input.clone()));
            }

            // Assistant message: may have tool_calls
            let nat = turn.native_assistant_turn.as_ref();
            let text = nat
                .and_then(|n| n.text_content.clone())
                .or_else(|| {
                    if !turn.model_response.is_empty() {
                        Some(turn.model_response.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let tool_calls: Option<Vec<Value>> = nat.and_then(|n| {
                if n.tool_calls.is_empty() {
                    return None;
                }
                Some(
                    n.tool_calls
                        .iter()
                        .enumerate()
                        .map(|(call_idx, tc)| {
                            let id = tc
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("call_{turn_idx}_{call_idx}"));
                            let mut obj = Map::new();
                            obj.insert("id".to_string(), Value::String(id));
                            obj.insert("type".to_string(), Value::String("function".to_string()));
                            let mut func = Map::new();
                            func.insert(
                                "name".to_string(),
                                Value::String(tc.provider_name.clone()),
                            );
                            func.insert(
                                "arguments".to_string(),
                                serde_json::to_string(&tc.arguments)
                                    .map(Value::String)
                                    .unwrap_or(Value::Null),
                            );
                            obj.insert("function".to_string(), Value::Object(func));
                            Value::Object(obj)
                        })
                        .collect(),
                )
            });

            messages.push(Message {
                role: Role::Model,
                content: MessageContent::Text(text),
                tool_calls,
                tool_call_id: None,
                name: None,
            });

            // Tool result messages
            if let Some(ref results) = turn.native_tool_results {
                for result in results {
                    messages.push(Message {
                        role: Role::ToolResult,
                        content: MessageContent::Text(
                            serde_json::to_string(&result.result).unwrap_or_default(),
                        ),
                        tool_calls: None,
                        tool_call_id: result.call_id.clone(),
                        name: Some(result.provider_name.clone()),
                    });
                }
            }
        }

        messages
    }

    /// Return the turn index before which the latest binding block should be
    /// rendered.
    ///
    /// Bindings are live runtime context, so they must sit immediately before
    /// the input that will drive the next model call. When the current input is
    /// an internal `[result]` turn, the binding belongs before that result so
    /// updated tool/context values override any older binding block for the
    /// continuation. For a normal user turn, this is naturally before that user
    /// input.
    pub fn binding_cursor_turn_index(&self) -> Option<usize> {
        self.turns.len().checked_sub(1)
    }

    /// Export session to JSON string for the host to persist
    pub fn export(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import session from JSON string (host restoring a saved session)
    pub fn import(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Clear all turns
    pub fn clear(&mut self) {
        self.turns.clear();
        self.stack.clear();
    }
}

pub fn display_input_value(input: &Value) -> String {
    match input {
        Value::String(text) => text.clone(),
        Value::Array(parts) if parts.iter().all(is_input_part) => display_input_parts(parts),
        value => serde_json::to_string(value).unwrap_or_else(|_| String::new()),
    }
}

pub fn input_parts_value(input: &Value) -> Option<Vec<Value>> {
    match input {
        Value::Array(parts) if parts.iter().all(is_input_part) => Some(parts.clone()),
        _ => None,
    }
}

pub fn display_input_parts(parts: &[Value]) -> String {
    let mut lines = Vec::new();
    for part in parts {
        let Some(part_type) = part.get("type").and_then(Value::as_str) else {
            continue;
        };
        match part_type {
            "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str)
                    && !text.is_empty()
                {
                    lines.push(text.to_string());
                }
            }
            "image" | "file" | "audio" | "video" => {
                lines.push(format!("[{}: {}]", part_type, media_part_label(part)));
            }
            _ => {}
        }
    }
    lines.join("\n")
}

fn is_input_part(value: &Value) -> bool {
    matches!(
        value.get("type").and_then(Value::as_str),
        Some("text" | "image" | "file" | "audio" | "video")
    )
}

fn media_part_label(part: &Value) -> String {
    for key in ["name", "path", "url", "ref", "mimeType"] {
        if let Some(value) = part.get(key).and_then(Value::as_str)
            && !value.is_empty()
        {
            return value.to_string();
        }
    }
    if part.get("data").is_some() {
        return "inline data".to_string();
    }
    "attached".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════════════
    // Native session state — round-trip message reconstruction
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn native_empty_session_has_system_prompt() {
        let mut session = SessionState::new();
        session.set_system_prompt("You are a helpful assistant.");
        let msgs = session.to_messages_native_openai();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, Role::System);
        assert_eq!(msgs[0].content.text(), "You are a helpful assistant.");
    }

    #[test]
    fn native_single_user_turn_reconstruction() {
        let mut session = SessionState::new();
        session.set_system_prompt("Sys");
        session.start_turn("Hello");
        session.set_model_response("Hi there");

        let msgs = session.to_messages_native_openai();
        assert_eq!(msgs.len(), 3); // system, user, assistant
        assert_eq!(msgs[1].role, Role::User);
        assert_eq!(msgs[1].content.text(), "Hello");
        assert_eq!(msgs[2].role, Role::Model);
        assert_eq!(msgs[2].content.text(), "Hi there");
        assert!(msgs[2].tool_calls.is_none());
    }

    #[test]
    fn native_turn_with_tool_calls_reconstruction() {
        let mut session = SessionState::new();
        session.set_system_prompt("Sys");
        session.start_turn("Search for cats");
        session.set_model_response("I'll search for cats.");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: Some("I'll search for cats.".to_string()),
            tool_calls: vec![NativeToolCallRecord {
                id: Some("call_abc123".to_string()),
                provider_name: "tool_search".to_string(),
                canonical_name: "search".to_string(),
                action_kind: "tool".to_string(),
                arguments: serde_json::json!({"query": "cats"}),
            }],
            structured_output: None,
        });
        session.append_native_tool_result(NativeToolResult {
            call_id: Some("call_abc123".to_string()),
            provider_name: "tool_search".to_string(),
            canonical_name: "search".to_string(),
            action_kind: "tool".to_string(),
            arguments: serde_json::json!({"query": "cats"}),
            result: serde_json::json!({"results": ["cat1", "cat2"]}),
        });

        let msgs = session.to_messages_native_openai();
        assert_eq!(msgs.len(), 4); // system, user, assistant(tool_calls), tool

        // Assistant message has tool_calls
        let assistant = &msgs[2];
        assert_eq!(assistant.role, Role::Model);
        assert!(assistant.tool_calls.is_some());
        let tcs = assistant.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0]["id"], "call_abc123");
        assert_eq!(tcs[0]["type"], "function");
        assert_eq!(tcs[0]["function"]["name"], "tool_search");
        assert_eq!(tcs[0]["function"]["arguments"], "{\"query\":\"cats\"}");

        // Tool result message
        let tool = &msgs[3];
        assert_eq!(tool.role, Role::ToolResult);
        assert_eq!(tool.tool_call_id, Some("call_abc123".to_string()));
    }

    #[test]
    fn native_multiple_tool_calls_ordered() {
        let mut session = SessionState::new();
        session.start_turn("Multi");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: None,
            tool_calls: vec![
                NativeToolCallRecord {
                    id: Some("c1".to_string()),
                    provider_name: "tool_a".to_string(),
                    canonical_name: "a".to_string(),
                    action_kind: "tool".to_string(),
                    arguments: Value::Null,
                },
                NativeToolCallRecord {
                    id: Some("c2".to_string()),
                    provider_name: "tool_b".to_string(),
                    canonical_name: "b".to_string(),
                    action_kind: "tool".to_string(),
                    arguments: Value::Null,
                },
            ],
            structured_output: None,
        });

        let msgs = session.to_messages_native_openai();
        let assistant = &msgs[1];
        let tcs = assistant.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 2);
        assert_eq!(tcs[0]["id"], "c1");
        assert_eq!(tcs[1]["id"], "c2");
    }

    #[test]
    fn native_tool_call_id_fallback_when_none() {
        let mut session = SessionState::new();
        session.start_turn("X");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: None,
            tool_calls: vec![NativeToolCallRecord {
                id: None,
                provider_name: "tool_foo".to_string(),
                canonical_name: "foo".to_string(),
                action_kind: "tool".to_string(),
                arguments: Value::Null,
            }],
            structured_output: None,
        });

        let msgs = session.to_messages_native_openai();
        let assistant = &msgs[1];
        let tcs = assistant.tool_calls.as_ref().unwrap();
        // Fallback uses turn_idx_call_idx pattern
        assert_eq!(tcs[0]["id"], "call_0_0");
    }

    #[test]
    fn native_session_import_export_roundtrip() {
        let mut session = SessionState::new();
        session.set_system_prompt("Sys");
        session.start_turn("Hello");
        session.set_model_response("Hi");
        session.set_turn_protocol("native");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: Some("Hi".to_string()),
            tool_calls: vec![NativeToolCallRecord {
                id: Some("c1".to_string()),
                provider_name: "tool_x".to_string(),
                canonical_name: "x".to_string(),
                action_kind: "tool".to_string(),
                arguments: serde_json::json!({"k": "v"}),
            }],
            structured_output: None,
        });
        session.append_native_tool_result(NativeToolResult {
            call_id: Some("c1".to_string()),
            provider_name: "tool_x".to_string(),
            canonical_name: "x".to_string(),
            action_kind: "tool".to_string(),
            arguments: serde_json::json!({"k": "v"}),
            result: serde_json::json!({"ok": true}),
        });

        let exported = session.export().unwrap();
        let imported = SessionState::import(&exported).unwrap();

        assert_eq!(imported.system_prompt, session.system_prompt);
        assert_eq!(imported.turns.len(), 1);
        let turn = &imported.turns[0];
        assert_eq!(turn.protocol, Some("native".to_string()));
        assert!(turn.native_assistant_turn.is_some());
        let nat = turn.native_assistant_turn.as_ref().unwrap();
        assert_eq!(nat.tool_calls.len(), 1);
        assert_eq!(nat.tool_calls[0].provider_name, "tool_x");
        assert!(turn.native_tool_results.is_some());
        assert_eq!(turn.native_tool_results.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn native_multimodal_input_parts_preserved() {
        let mut session = SessionState::new();
        let parts = vec![
            serde_json::json!({"type": "text", "text": "Look at this"}),
            serde_json::json!({"type": "image", "url": "http://example.com/img.png"}),
        ];
        session.start_turn_parts("Look at this", parts.clone());
        session.set_model_response("I see it.");

        let msgs = session.to_messages_native_openai();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::User);
        match &msgs[0].content {
            MessageContent::Parts(p) => assert_eq!(p.len(), 2),
            _ => panic!("expected parts"),
        }
    }

    #[test]
    fn native_structured_output_turn() {
        let mut session = SessionState::new();
        session.start_turn("Give me JSON");
        session.set_model_response("");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: None,
            tool_calls: vec![],
            structured_output: Some(serde_json::json!({"answer": 42})),
        });

        let msgs = session.to_messages_native_openai();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1].content.text(), "");
        assert!(msgs[1].tool_calls.is_none());
    }

    #[test]
    fn native_and_block_mode_messages_are_independent() {
        let mut session = SessionState::new();
        session.set_system_prompt("Sys");
        session.start_turn("Hello");
        session.set_model_response("Hi");

        // Block mode messages (default)
        let block_msgs = session.to_messages_with_bindings(None);
        assert_eq!(block_msgs.len(), 3);
        assert!(block_msgs[2].tool_calls.is_none());

        // Native mode messages
        let native_msgs = session.to_messages_native_openai();
        assert_eq!(native_msgs.len(), 3);
        assert!(native_msgs[2].tool_calls.is_none());
    }

    #[test]
    fn turn_protocol_field_set_and_get() {
        let mut session = SessionState::new();
        session.start_turn("A");
        session.set_turn_protocol("native");
        assert_eq!(session.turns[0].protocol, Some("native".to_string()));

        session.start_turn("B");
        session.set_turn_protocol("block");
        assert_eq!(session.turns[1].protocol, Some("block".to_string()));
    }

    #[test]
    fn native_tool_result_without_call_id() {
        let mut session = SessionState::new();
        session.start_turn("X");
        session.set_native_assistant_turn(NativeAssistantTurn {
            text_content: None,
            tool_calls: vec![NativeToolCallRecord {
                id: None,
                provider_name: "tool_y".to_string(),
                canonical_name: "y".to_string(),
                action_kind: "tool".to_string(),
                arguments: Value::Null,
            }],
            structured_output: None,
        });
        session.append_native_tool_result(NativeToolResult {
            call_id: None,
            provider_name: "tool_y".to_string(),
            canonical_name: "y".to_string(),
            action_kind: "tool".to_string(),
            arguments: Value::Null,
            result: serde_json::json!("done"),
        });

        let msgs = session.to_messages_native_openai();
        let tool_msg = &msgs[2];
        assert_eq!(tool_msg.role, Role::ToolResult);
        assert_eq!(tool_msg.tool_call_id, None);
        assert_eq!(tool_msg.content.text(), "\"done\"");
    }

    #[test]
    fn message_constructors_set_tool_fields_correctly() {
        let m1 = Message::model("hi");
        assert!(m1.tool_calls.is_none());
        assert!(m1.tool_call_id.is_none());

        let tc = vec![serde_json::json!({"id": "c1"})];
        let m2 = Message::model_with_tool_calls("hi", tc.clone());
        assert_eq!(m2.tool_calls, Some(tc));
        assert!(m2.tool_call_id.is_none());

        let m3 = Message::tool_result_with_id("result", "c1");
        assert!(m3.tool_calls.is_none());
        assert_eq!(m3.tool_call_id, Some("c1".to_string()));
    }
}
