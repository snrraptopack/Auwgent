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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<crate::runtime::drivers::ModelEvent, String>> + Send>>, String> {
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };

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
            "stream_options": { "include_usage": true }
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

                let mut result_events = Vec::new();
                while let Some(index) = buffer.find('\n') {
                    let line = buffer.drain(..=index).collect::<String>();
                    let trimmed = line.trim();

                    if trimmed == "data: [DONE]" {
                        continue;
                    }

                    if let Some(data) = trimmed.strip_prefix("data: ") {
                        if let Ok(json_val) = serde_json::from_str::<Value>(data) {
                            if let Some(choices) = json_val.get("choices").and_then(|v| v.as_array()) {
                                if !choices.is_empty() {
                                    if let Some(content) = choices[0].get("delta").and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                                        result_events.push(crate::runtime::drivers::ModelEvent::ContentChunk(content.to_string()));
                                    }
                                    
                                    if let Some(finish_reason_str) = choices[0].get("finish_reason").and_then(|f| f.as_str()) {
                                        let finish_reason = match finish_reason_str {
                                            "stop" => crate::runtime::drivers::FinishReason::Stop,
                                            "length" => crate::runtime::drivers::FinishReason::Length,
                                            "tool_calls" => crate::runtime::drivers::FinishReason::ToolCalls,
                                            "content_filter" => crate::runtime::drivers::FinishReason::ContentFilter,
                                            _ => crate::runtime::drivers::FinishReason::Other(finish_reason_str.to_string()),
                                        };
                                        result_events.push(crate::runtime::drivers::ModelEvent::FinishReason(finish_reason));
                                    }
                                }
                            }
                            
                            // Extract usage if available
                            if let Some(usage) = json_val.get("usage") {
                                let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                
                                result_events.push(crate::runtime::drivers::ModelEvent::Usage(crate::runtime::drivers::TokenUsage {
                                    prompt_tokens,
                                    completion_tokens,
                                    total_tokens,
                                }));
                            }
                        }
                    }
                }
                Ok(result_events)
            }
            Err(e) => Err(format!("Stream error: {}", e)),
        })
        .flat_map(|res| match res {
            Ok(events) => futures_util::stream::iter(events.into_iter().map(Ok)).left_stream(),
            Err(e) => futures_util::stream::iter(vec![Err(e)]).right_stream(),
        });

        Ok(Box::pin(stream))
    }

    async fn embed(
        &self,
        model: &str,
        text: &str,
        config: Option<Value>,
    ) -> Result<Vec<f32>, String> {
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/embeddings") {
            base.to_string()
        } else {
            format!("{}/embeddings", base)
        };

        let mut body = json!({
            "model": model,
            "input": text,
        });

        // Merge optional config directly into the body
        if let Some(cfg) = config.as_ref().and_then(|c| c.as_object()) {
            if let Some(body_obj) = body.as_object_mut() {
                for (k, v) in cfg {
                    body_obj.insert(k.clone(), v.clone());
                }
            }
        }

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send embedding request to OpenAI: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "OpenAI Embedding API error ({}): {}",
                status, error_text
            ));
        }

        let json_val: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        let embedding_val = &json_val["data"][0]["embedding"];
        let values = embedding_val.as_array().ok_or_else(|| {
            format!(
                "Missing 'data[0].embedding' field in response: {}",
                json_val.to_string()
            )
        })?;

        Ok(values
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect())
    }

    async fn embed_batch(
        &self,
        model: &str,
        texts: &[String],
        config: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String> {
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/embeddings") {
            base.to_string()
        } else {
            format!("{}/embeddings", base)
        };

        let mut body = json!({
            "model": model,
            "input": texts,
        });

        // Merge optional config directly into the body
        if let Some(cfg) = config.as_ref().and_then(|c| c.as_object()) {
            if let Some(body_obj) = body.as_object_mut() {
                for (k, v) in cfg {
                    body_obj.insert(k.clone(), v.clone());
                }
            }
        }

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send embedding request to OpenAI: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "OpenAI Embedding API error ({}): {}",
                status, error_text
            ));
        }

        let json_val: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        let data = json_val["data"]
            .as_array()
            .ok_or_else(|| format!("Missing 'data' field in response: {}", json_val.to_string()))?;

        let mut results = Vec::new();
        for item in data {
            let embedding = item["embedding"]
                .as_array()
                .ok_or_else(|| "Missing 'embedding' in data item".to_string())?;
            let vec: Vec<f32> = embedding
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            results.push(vec);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_construction() {
        let driver1 = OpenAIDriver::new(
            "key".to_string(),
            Some("https://api.groq.com/openai/v1/chat/completions".to_string()),
        );
        let base1 = driver1.base_url.trim_end_matches('/');
        let url1 = if base1.ends_with("/chat/completions") {
            base1.to_string()
        } else {
            format!("{}/chat/completions", base1)
        };
        assert_eq!(url1, "https://api.groq.com/openai/v1/chat/completions");

        let driver2 = OpenAIDriver::new(
            "key".to_string(),
            Some("https://api.openai.com/v1".to_string()),
        );
        let base2 = driver2.base_url.trim_end_matches('/');
        let url2 = if base2.ends_with("/chat/completions") {
            base2.to_string()
        } else {
            format!("{}/chat/completions", base2)
        };
        assert_eq!(url2, "https://api.openai.com/v1/chat/completions");

        let driver3 = OpenAIDriver::new("key".to_string(), None);
        let base3 = driver3.base_url.trim_end_matches('/');
        let url3 = if base3.ends_with("/chat/completions") {
            base3.to_string()
        } else {
            format!("{}/chat/completions", base3)
        };
        assert_eq!(url3, "https://api.openai.com/v1/chat/completions");
    }
}
