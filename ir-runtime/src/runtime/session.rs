use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunStep {
    /// System or User instructions starting the turn
    Prompt {
        content: String,
    },
    /// What the model actually said (raw)
    ModelResponse {
        content: String,
    },
    /// High-level parsed output (if schema or text intent was hit)
    ModelOutput {
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_yaml: Option<String>,
    },
    /// A tool or workflow call and its result
    IntentAction {
        name: String,
        args: Value,
        result: Option<Value>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    pub steps: Vec<RunStep>,
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_step(&mut self, step: RunStep) {
        self.steps.push(step);
    }

    pub fn clear(&mut self) {
        self.steps.clear();
    }
}
