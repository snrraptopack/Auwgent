use async_trait::async_trait;
use futures_util::Stream;
use serde_json::Value;
use std::pin::Pin;

use crate::runtime::session::Message;

pub mod gemini;
pub mod openai;



use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub usage: TokenUsage,
    pub finish_reason: Option<FinishReason>,
}

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

pub enum ModelEvent {
    ContentChunk(String),
    Usage(TokenUsage),
    FinishReason(FinishReason),
    Metadata(ModelMetadata),
}

#[async_trait]
pub trait ModelDriver: Send + Sync {
    /// Send a conversation to the LLM and return a stream of text chunks.
    ///
    /// `messages` contains the full conversation history including system prompt,
    /// user messages, model responses, and tool results.
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ModelEvent, String>> + Send>>, String>;

    /// Generate an embedding for the given text.
    async fn embed(
        &self,
        model: &str,
        text: &str,
        config: Option<Value>,
    ) -> Result<Vec<f32>, String>;

    /// Generate embeddings for a batch of texts.
    async fn embed_batch(
        &self,
        model: &str,
        texts: &[String],
        config: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String>;
}
