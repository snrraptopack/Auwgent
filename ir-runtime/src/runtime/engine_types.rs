use crate::runtime::drivers::{FinishReason, TokenUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
use futures_util::future::BoxFuture as RuntimeBoxFuture;
#[cfg(target_arch = "wasm32")]
use futures_util::future::LocalBoxFuture as RuntimeBoxFuture;
use std::sync::Arc as RuntimeCallback;

pub type EngineFuture<'a, T> = RuntimeBoxFuture<'a, T>;

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

#[cfg(not(target_arch = "wasm32"))]
pub type ToolImplementation =
    RuntimeCallback<dyn Fn(Value) -> RuntimeBoxFuture<'static, Result<Value, String>> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type ToolImplementation =
    RuntimeCallback<dyn Fn(Value) -> RuntimeBoxFuture<'static, Result<Value, String>> + Send + Sync>;

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
#[cfg(not(target_arch = "wasm32"))]
pub type IntentCallback =
    RuntimeCallback<dyn Fn(String, Value, String) -> Option<IntentControl> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type IntentCallback =
    RuntimeCallback<dyn Fn(String, Value, String) -> Option<IntentControl> + Send + Sync>;

/// Async intent callback for handlers that need to await.
/// Receives (intent_name, intent_value, agent_name).
#[cfg(not(target_arch = "wasm32"))]
pub type AsyncIntentCallback = RuntimeCallback<
    dyn Fn(String, Value, String) -> RuntimeBoxFuture<'static, Option<IntentControl>>
        + Send
        + Sync,
>;

#[cfg(target_arch = "wasm32")]
pub type AsyncIntentCallback = RuntimeCallback<
    dyn Fn(String, Value, String) -> RuntimeBoxFuture<'static, Option<IntentControl>>
        + Send
        + Sync,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type PartialIntentCallback =
    RuntimeCallback<dyn Fn(String, Value, String) + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type PartialIntentCallback =
    RuntimeCallback<dyn Fn(String, Value, String) + Send + Sync>;

/// Async callback for preloading a helper's session history before it runs.
/// Receives `(helper_name, empty_session_json)`. Returns an optional `SessionState` JSON string.
#[cfg(not(target_arch = "wasm32"))]
pub type AsyncSessionPreloadCallback = RuntimeCallback<
    dyn Fn(String, String) -> RuntimeBoxFuture<'static, Option<String>> + Send + Sync,
>;

#[cfg(target_arch = "wasm32")]
pub type AsyncSessionPreloadCallback =
    RuntimeCallback<dyn Fn(String, String) -> RuntimeBoxFuture<'static, Option<String>> + Send + Sync>;

/// Async callback for saving a helper's session history after it completes.
/// Receives `(helper_name, completed_session_json)`.
#[cfg(not(target_arch = "wasm32"))]
pub type SessionSaveCallback =
    RuntimeCallback<dyn Fn(String, String) -> RuntimeBoxFuture<'static, ()> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type SessionSaveCallback =
    RuntimeCallback<dyn Fn(String, String) -> RuntimeBoxFuture<'static, ()> + Send + Sync>;

/// Generic middleware event callback.
/// Receives an event JSON payload and may return an optional response JSON payload.
#[cfg(not(target_arch = "wasm32"))]
pub type AsyncMiddlewareEventCallback =
    RuntimeCallback<dyn Fn(String) -> RuntimeBoxFuture<'static, Option<String>> + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub type AsyncMiddlewareEventCallback =
    RuntimeCallback<dyn Fn(String) -> RuntimeBoxFuture<'static, Option<String>> + Send + Sync>;
