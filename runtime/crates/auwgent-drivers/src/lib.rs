pub mod gemini;
pub mod openai;

use async_trait::async_trait;
use auwgent_runtime_core::{Message, ModelDriverBounds, ModelEventStream};
use serde_json::Value;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ModelDriver: ModelDriverBounds {
    /// Send a conversation to the LLM and return a stream of text chunks.
    ///
    /// `messages` contains the full conversation history including system prompt,
    /// user messages, model responses, and tool results.
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
        headers: Option<Value>,
        api_key: Option<String>,
    ) -> Result<ModelEventStream, String>;

    /// Generate an embedding for the given text.
    async fn embed(
        &self,
        model: &str,
        text: &str,
        config: Option<Value>,
        headers: Option<Value>,
        api_key: Option<String>,
    ) -> Result<Vec<f32>, String>;

    /// Generate embeddings for a batch of texts.
    async fn embed_batch(
        &self,
        model: &str,
        texts: &[String],
        config: Option<Value>,
        headers: Option<Value>,
        api_key: Option<String>,
    ) -> Result<Vec<Vec<f32>>, String>;
}
