use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(content.into()),
        }
    }

    pub fn user_parts(parts: Vec<Value>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Parts(parts),
        }
    }

    pub fn model(content: impl Into<String>) -> Self {
        Self {
            role: Role::Model,
            content: MessageContent::Text(content.into()),
        }
    }

    pub fn tool_result(content: impl Into<String>) -> Self {
        Self {
            role: Role::ToolResult,
            content: MessageContent::Text(content.into()),
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
    #[serde(default, rename = "inputParts", skip_serializing_if = "Option::is_none")]
    pub input_parts: Option<Vec<Value>>,
    /// The raw model response
    pub model_response: String,
}

impl Turn {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            input_parts: None,
            model_response: String::new(),
        }
    }

    pub fn with_parts(input: impl Into<String>, parts: Vec<Value>) -> Self {
        Self {
            input: input.into(),
            input_parts: Some(parts),
            model_response: String::new(),
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

    /// Get the current (most recent) turn mutably
    pub fn current_turn_mut(&mut self) -> Option<&mut Turn> {
        self.turns.last_mut()
    }

    /// Set the model response on the current turn
    pub fn set_model_response(&mut self, response: impl Into<String>) {
        if let Some(turn) = self.turns.last_mut() {
            turn.model_response = response.into();
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

    /// Return the turn index before which the latest binding block should be
    /// rendered. Internal result turns do not advance the cursor; only external
    /// user inputs do.
    pub fn binding_cursor_turn_index(&self) -> Option<usize> {
        self.turns
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, turn)| (!is_runtime_result_turn(&turn.input)).then_some(index))
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

fn is_runtime_result_turn(input: &str) -> bool {
    input.trim_start().starts_with("[result]")
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
