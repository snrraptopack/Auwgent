use serde::{Deserialize, Serialize};

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
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn model(content: impl Into<String>) -> Self {
        Self {
            role: Role::Model,
            content: content.into(),
        }
    }

    pub fn tool_result(content: impl Into<String>) -> Self {
        Self {
            role: Role::ToolResult,
            content: content.into(),
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
    /// The raw model response
    pub model_response: String,
}

impl Turn {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            model_response: String::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SESSION STATE — the temporal conversation state
// ═══════════════════════════════════════════════════════════════════════════

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
        let mut messages = Vec::new();

        // System prompt
        if let Some(ref prompt) = self.system_prompt {
            messages.push(Message::system(prompt.clone()));
        }

        // Each turn contributes user + model messages
        for turn in &self.turns {
            // The input for this turn
            messages.push(Message::user(turn.input.clone()));

            // The model response
            if !turn.model_response.is_empty() {
                messages.push(Message::model(turn.model_response.clone()));
            }
        }

        messages
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
    }
}
