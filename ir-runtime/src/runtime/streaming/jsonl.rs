use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputPhase {
    Partial,
    Final,
    Lifecycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredOutputEvent {
    pub seq: u64,
    pub ts_ms: u64,
    pub event: String,
    pub phase: StructuredOutputPhase,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl StructuredOutputEvent {
    pub fn intent(seq: u64, agent: String, name: String, payload: Value) -> Self {
        Self {
            seq,
            ts_ms: now_ms(),
            event: "intent".to_string(),
            phase: StructuredOutputPhase::Final,
            agent,
            name: Some(name),
            payload: Some(payload),
            done: None,
            error: None,
        }
    }

    pub fn partial_intent(seq: u64, agent: String, name: String, payload: Value) -> Self {
        Self {
            seq,
            ts_ms: now_ms(),
            event: "partial_intent".to_string(),
            phase: StructuredOutputPhase::Partial,
            agent,
            name: Some(name),
            payload: Some(payload),
            done: None,
            error: None,
        }
    }

    pub fn lifecycle_start(seq: u64, agent: String) -> Self {
        Self {
            seq,
            ts_ms: now_ms(),
            event: "stream_start".to_string(),
            phase: StructuredOutputPhase::Lifecycle,
            agent,
            name: None,
            payload: None,
            done: None,
            error: None,
        }
    }

    pub fn lifecycle_finish(seq: u64, agent: String) -> Self {
        Self {
            seq,
            ts_ms: now_ms(),
            event: "stream_finish".to_string(),
            phase: StructuredOutputPhase::Lifecycle,
            agent,
            name: None,
            payload: None,
            done: Some(true),
            error: None,
        }
    }

    pub fn lifecycle_error(seq: u64, agent: String, message: String) -> Self {
        Self {
            seq,
            ts_ms: now_ms(),
            event: "stream_error".to_string(),
            phase: StructuredOutputPhase::Lifecycle,
            agent,
            name: None,
            payload: None,
            done: Some(true),
            error: Some(message),
        }
    }

    pub fn to_jsonl_line(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }
}

#[derive(Debug, Default)]
pub struct JsonlEventBuffer {
    lines: Vec<String>,
    seq: u64,
}

impl JsonlEventBuffer {
    pub fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    pub fn push_event(&mut self, event: StructuredOutputEvent) {
        if let Some(line) = event.to_jsonl_line() {
            self.lines.push(line);
        }
    }

    pub fn drain_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
