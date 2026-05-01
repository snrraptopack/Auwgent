use crate::runtime::drivers::{FinishReason, TokenUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

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

pub type ToolImplementation = Arc<
    dyn Fn(Value) -> futures_util::future::BoxFuture<'static, Result<Value, String>> + Send + Sync,
>;

/// Control returned by an intent handler to override default behavior.
#[derive(Debug, Clone)]
pub enum IntentControl {
    /// Skip this intent - don't execute the tool/workflow
    Skip,
    /// Use this result instead of executing the tool
    Override { result: Value },
}

/// Intent callback for standard synchronous handlers.
/// Receives (intent_name, intent_value, agent_name).
pub type IntentCallback = Arc<dyn Fn(String, Value, String) -> Option<IntentControl> + Send + Sync>;

/// Async intent callback for handlers that need to await.
/// Receives (intent_name, intent_value, agent_name).
pub type AsyncIntentCallback = Arc<
    dyn Fn(String, Value, String) -> futures_util::future::BoxFuture<'static, Option<IntentControl>>
        + Send
        + Sync,
>;

/// Async callback for preloading a helper's session history before it runs.
/// Receives `(helper_name, empty_session_json)`. Returns an optional `SessionState` JSON string.
pub type AsyncSessionPreloadCallback = Arc<
    dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, Option<String>>
        + Send
        + Sync,
>;

/// Async callback for saving a helper's session history after it completes.
/// Receives `(helper_name, completed_session_json)`.
pub type SessionSaveCallback =
    Arc<dyn Fn(String, String) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync>;

/// Generic middleware event callback.
/// Receives an event JSON payload and may return an optional response JSON payload.
pub type AsyncMiddlewareEventCallback =
    Arc<dyn Fn(String) -> futures_util::future::BoxFuture<'static, Option<String>> + Send + Sync>;
