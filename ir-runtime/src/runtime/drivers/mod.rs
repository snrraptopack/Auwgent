use async_trait::async_trait;
use futures_util::Stream;
use serde_json::Value;
use std::pin::Pin;

use crate::runtime::session::Message;

pub mod gemini;

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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String>;
}
