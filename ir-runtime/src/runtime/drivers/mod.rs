use async_trait::async_trait;
use futures_util::Stream;
use serde_json::Value;
use std::pin::Pin;

pub mod gemini;

#[async_trait]
pub trait ModelDriver: Send + Sync {
    /// Send a request to the LLM and return a stream of text chunks or structured intents.
    async fn stream_generate_content(
        &self,
        model: &str,
        prompt: &str,
        system_instruction: Option<&str>,
        config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String>;
}
