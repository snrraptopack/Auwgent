use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

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

fn display_input_parts(parts: &[Value]) -> String {
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

// ═══════════════════════════════════════════════════════════════════════════
// DRIVER BASE TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: u32,
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub usage: TokenUsage,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone)]
pub enum ModelEvent {
    ContentChunk(String),
    Usage(TokenUsage),
    FinishReason(FinishReason),
    Metadata(ModelMetadata),

    /// A completed native tool/function call from the provider.
    NativeToolCall {
        id: Option<String>,
        provider_name: String,
        arguments: Value,
    },

    /// Provider-native structured output matching the agent's output schema.
    NativeStructuredOutput(Value),
}

#[cfg(not(target_arch = "wasm32"))]
pub type ModelEventStream = std::pin::Pin<
    Box<dyn futures_util::Stream<Item = Result<ModelEvent, String>> + Send>,
>;

#[cfg(target_arch = "wasm32")]
pub type ModelEventStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<ModelEvent, String>>>>;

#[cfg(not(target_arch = "wasm32"))]
pub trait ModelDriverBounds: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> ModelDriverBounds for T {}

#[cfg(target_arch = "wasm32")]
pub trait ModelDriverBounds {}
#[cfg(target_arch = "wasm32")]
impl<T> ModelDriverBounds for T {}

// ═══════════════════════════════════════════════════════════════════════════
// RUNTIME METADATA
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: u32,
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnMetadata {
    pub turn_index: usize,
    pub usage: TokenUsage,
    pub finish_reason: Option<FinishReason>,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunMetadata {
    pub aggregate: AggregateUsage,
    pub turns: Vec<TurnMetadata>,
}

// ═══════════════════════════════════════════════════════════════════════════
// INTENT CONTROL
// ═══════════════════════════════════════════════════════════════════════════

/// Control returned by an intent handler to override default behavior.
#[derive(Debug, Clone)]
pub enum IntentControl {
    /// Skip this intent - don't execute the tool/workflow
    Skip,
    /// Use this result instead of executing the tool
    Override { result: Value },
}

// ═══════════════════════════════════════════════════════════════════════════
// CALLBACK TYPE ALIASES
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
pub type EngineFuture<'a, T> = futures_util::future::BoxFuture<'a, T>;
#[cfg(target_arch = "wasm32")]
pub type EngineFuture<'a, T> = futures_util::future::LocalBoxFuture<'a, T>;

#[cfg(not(target_arch = "wasm32"))]
pub type ToolImplementation = Arc<
    dyn Fn(Value) -> EngineFuture<'static, Result<Value, String>> + Send + Sync,
>;

#[cfg(target_arch = "wasm32")]
pub type ToolImplementation = Arc<
    dyn Fn(Value) -> EngineFuture<'static, Result<Value, String>> + Send + Sync,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type IntentCallback =
    Arc<dyn Fn(String, Value, String) -> Option<IntentControl> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type IntentCallback =
    Arc<dyn Fn(String, Value, String) -> Option<IntentControl> + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
pub type AsyncIntentCallback = Arc<
    dyn Fn(String, Value, String) -> EngineFuture<'static, Option<IntentControl>> + Send + Sync,
>;

#[cfg(target_arch = "wasm32")]
pub type AsyncIntentCallback = Arc<
    dyn Fn(String, Value, String) -> EngineFuture<'static, Option<IntentControl>> + Send + Sync,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type PartialIntentCallback = Arc<dyn Fn(String, Value, String) + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type PartialIntentCallback = Arc<dyn Fn(String, Value, String) + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
pub type AsyncSessionPreloadCallback = Arc<
    dyn Fn(String, String) -> EngineFuture<'static, Option<String>> + Send + Sync,
>;

#[cfg(target_arch = "wasm32")]
pub type AsyncSessionPreloadCallback = Arc<
    dyn Fn(String, String) -> EngineFuture<'static, Option<String>> + Send + Sync,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type SessionSaveCallback =
    Arc<dyn Fn(String, String) -> EngineFuture<'static, ()> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type SessionSaveCallback =
    Arc<dyn Fn(String, String) -> EngineFuture<'static, ()> + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
pub type AsyncMiddlewareEventCallback =
    Arc<dyn Fn(String) -> EngineFuture<'static, Option<String>> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type AsyncMiddlewareEventCallback =
    Arc<dyn Fn(String) -> EngineFuture<'static, Option<String>> + Send + Sync>;
