use crate::runtime::drivers::ModelDriver;
use crate::runtime::session::{Message, Role};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAIDriver {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIDriver {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        }
    }
}

#[async_trait]
impl ModelDriver for OpenAIDriver {
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        // ── Build messages array ──────────────────────────────────────────
        let openai_messages: Vec<Value> = messages
            .iter()
            .map(|m| match m.role {
                Role::System => json!({
                    "role": "system",
                    "content": m.content
                }),
                Role::User => json!({
                    "role": "user",
                    "content": m.content
                }),
                Role::Model => json!({
                    "role": "assistant",
                    "content": m.content
                }),
                Role::ToolResult => json!({
                    "role": "user",
                    "content": m.content
                }),
            })
            .collect();

        // ── Build request body ────────────────────────────────────────────
        let mut body = json!({
            "model": model,
            "messages": openai_messages,
            "stream": true,
        });

        // Apply generation config params if provided
        if let Some(cfg) = config {
            let body_obj = body.as_object_mut().expect("body is always an object");
            for key in &[
                "temperature",
                "top_p",
                "max_tokens",
                "stop",
                "frequency_penalty",
                "presence_penalty",
            ] {
                if let Some(val) = cfg.get(*key) {
                    body_obj.insert(key.to_string(), val.clone());
                }
            }
        }

        // ── Send request ──────────────────────────────────────────────────
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to OpenAI: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("OpenAI API error ({}): {}", status, error_text));
        }

        // ── SSE stream parsing ────────────────────────────────────────────
        let mut buffer = String::new();
        let stream = response.bytes_stream().map(move |item| match item {
            Ok(bytes) => {
                let chunk = String::from_utf8_lossy(&bytes);
                buffer.push_str(&chunk);

                let mut result_text = String::new();
                while let Some(index) = buffer.find('\n') {
                    let line = buffer.drain(..=index).collect::<String>();
                    let trimmed = line.trim();

                    if trimmed == "data: [DONE]" {
                        continue;
                    }

                    if let Some(data) = trimmed.strip_prefix("data: ") {
                        if let Ok(json_val) = serde_json::from_str::<Value>(data) {
                            if let Some(content) =
                                json_val["choices"][0]["delta"]["content"].as_str()
                            {
                                result_text.push_str(content);
                            }
                        }
                    }
                }
                Ok(result_text)
            }
            Err(e) => Err(format!("Stream error: {}", e)),
        });

        Ok(Box::pin(stream))
    }
}
