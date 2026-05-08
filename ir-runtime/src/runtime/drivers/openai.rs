use crate::runtime::drivers::{ModelDriver, ModelEventStream};
use crate::runtime::session::{Message, Role};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};

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

    fn merge_request_config(body: &mut Value, cfg: &Value) {
        let Some(cfg_obj) = cfg.as_object() else {
            return;
        };
        let Some(body_obj) = body.as_object_mut() else {
            return;
        };

        for (key, value) in cfg_obj {
            if matches!(
                key.as_str(),
                "model" | "messages" | "stream" | "stream_options"
            ) {
                continue;
            }
            body_obj.insert(key.clone(), value.clone());
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelDriver for OpenAIDriver {
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
    ) -> Result<ModelEventStream, String> {
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
                    "content": m.content.text()
                }),
                Role::User => json!({
                    "role": "user",
                    "content": openai_content(m)
                }),
                Role::Model => json!({
                    "role": "assistant",
                    "content": m.content.text()
                }),
                Role::ToolResult => json!({
                    "role": "user",
                    "content": m.content.text()
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
        if let Some(cfg) = config.as_ref() {
            Self::merge_request_config(&mut body, cfg);
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
        let mut buffer = Vec::<u8>::new();
        let stream = response
            .bytes_stream()
            .map(move |item| match item {
                Ok(bytes) => {
                    buffer.extend_from_slice(&bytes);

                    let mut result_events = Vec::new();
                    while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
                        let line_bytes = buffer.drain(..=index).collect::<Vec<u8>>();
                        let line = String::from_utf8(line_bytes)
                            .map_err(|e| format!("OpenAI stream contained invalid UTF-8: {}", e))?;
                        let trimmed = line.trim_end_matches(['\r', '\n']).trim_start();

                        if trimmed == "data: [DONE]" {
                            continue;
                        }

                        if let Some(data) = trimmed.strip_prefix("data: ")
                            && let Ok(json_val) = serde_json::from_str::<Value>(data)
                        {
                            if let Some(choices) =
                                json_val.get("choices").and_then(|v| v.as_array())
                                && !choices.is_empty()
                            {
                                if let Some(content) = choices[0]
                                    .get("delta")
                                    .and_then(|d| d.get("content"))
                                    .and_then(|c| c.as_str())
                                {
                                    result_events.push(
                                        crate::runtime::drivers::ModelEvent::ContentChunk(
                                            content.to_string(),
                                        ),
                                    );
                                }

                                if let Some(finish_reason_str) =
                                    choices[0].get("finish_reason").and_then(|f| f.as_str())
                                {
                                    let finish_reason = match finish_reason_str {
                                        "stop" => crate::runtime::drivers::FinishReason::Stop,
                                        "length" => crate::runtime::drivers::FinishReason::Length,
                                        "tool_calls" => {
                                            crate::runtime::drivers::FinishReason::ToolCalls
                                        }
                                        "content_filter" => {
                                            crate::runtime::drivers::FinishReason::ContentFilter
                                        }
                                        _ => crate::runtime::drivers::FinishReason::Other(
                                            finish_reason_str.to_string(),
                                        ),
                                    };
                                    result_events.push(
                                        crate::runtime::drivers::ModelEvent::FinishReason(
                                            finish_reason,
                                        ),
                                    );
                                }
                            }
                            // Extract usage if available
                            if let Some(usage) = json_val.get("usage") {
                                let prompt_tokens = usage
                                    .get("prompt_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let completion_tokens = usage
                                    .get("completion_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let total_tokens = usage
                                    .get("total_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let prompt_tokens_details = usage
                                    .get("prompt_tokens_details")
                                    .or_else(|| usage.get("input_tokens_details"))
                                    .and_then(|v| v.as_object());
                                let completion_tokens_details = usage
                                    .get("completion_tokens_details")
                                    .and_then(|v| v.as_object());

                                let cached_tokens = prompt_tokens_details
                                    .and_then(|details| details.get("cached_tokens"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let reasoning_tokens = completion_tokens_details
                                    .and_then(|details| details.get("reasoning_tokens"))
                                    .or_else(|| {
                                        prompt_tokens_details
                                            .and_then(|details| details.get("reasoning_tokens"))
                                    })
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;

                                result_events.push(crate::runtime::drivers::ModelEvent::Usage(
                                    crate::runtime::drivers::TokenUsage {
                                        prompt_tokens,
                                        completion_tokens,
                                        total_tokens,
                                        reasoning_tokens,
                                        cached_tokens,
                                    },
                                ));
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
        if let Some(cfg) = config.as_ref().and_then(|c| c.as_object())
            && let Some(body_obj) = body.as_object_mut()
        {
            for (k, v) in cfg {
                body_obj.insert(k.clone(), v.clone());
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
                json_val
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
        if let Some(cfg) = config.as_ref().and_then(|c| c.as_object())
            && let Some(body_obj) = body.as_object_mut()
        {
            for (k, v) in cfg {
                body_obj.insert(k.clone(), v.clone());
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
            .ok_or_else(|| format!("Missing 'data' field in response: {}", json_val))?;

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

fn openai_content(message: &Message) -> Value {
    let Some(parts) = message.content.parts() else {
        return json!(message.content.text());
    };

    let mut content = Vec::new();
    for part in parts {
        let Some(part_type) = part.get("type").and_then(Value::as_str) else {
            continue;
        };
        match part_type {
            "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    content.push(json!({ "type": "text", "text": text }));
                }
            }
            "image" => {
                if let Some(url) = media_url(part) {
                    let mut image_url = json!({ "url": url });
                    if let Some(detail) = part.get("detail").and_then(Value::as_str)
                        && let Some(obj) = image_url.as_object_mut()
                    {
                        obj.insert("detail".to_string(), json!(detail));
                    }
                    content.push(json!({ "type": "image_url", "image_url": image_url }));
                } else {
                    content.push(json!({
                        "type": "text",
                        "text": format!("[image: {}]", media_label(part))
                    }));
                }
            }
            "file" | "audio" | "video" => {
                content.push(json!({
                    "type": "text",
                    "text": format!("[{}: {}]", part_type, media_label(part))
                }));
            }
            _ => {}
        }
    }

    if content.is_empty() {
        json!(message.content.text())
    } else {
        Value::Array(content)
    }
}

fn media_url(part: &Value) -> Option<String> {
    if let Some(url) = part.get("url").and_then(Value::as_str) {
        return Some(url.to_string());
    }
    if let Some(data) = part.get("data").and_then(Value::as_str) {
        let mime = part
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        if matches!(
            part.get("encoding").and_then(Value::as_str),
            Some("utf8" | "utf-8")
        ) {
            return Some(format!(
                "data:{mime};base64,{}",
                general_purpose::STANDARD.encode(data.as_bytes())
            ));
        }
        return Some(format!("data:{mime};base64,{data}"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = part.get("path").and_then(Value::as_str)
        && let Ok(bytes) = std::fs::read(path)
    {
        let mime = part
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        return Some(format!(
            "data:{mime};base64,{}",
            general_purpose::STANDARD.encode(&bytes)
        ));
    }
    None
}

fn media_label(part: &Value) -> String {
    for key in ["name", "path", "url", "ref", "mimeType"] {
        if let Some(value) = part.get(key).and_then(Value::as_str)
            && !value.is_empty()
        {
            return value.to_string();
        }
    }
    if part.get("data").is_some() {
        "inline data".to_string()
    } else {
        "attached".to_string()
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

    #[test]
    fn merges_arbitrary_request_config_fields() {
        let mut body = json!({
            "model": "demo",
            "messages": [],
            "stream": true,
            "stream_options": { "include_usage": true }
        });
        let cfg = json!({
            "temperature": 0.1,
            "somefield": { "another": "value" },
            "stream": false
        });

        OpenAIDriver::merge_request_config(&mut body, &cfg);

        assert_eq!(body["temperature"], json!(0.1));
        assert_eq!(body["somefield"], json!({ "another": "value" }));
        assert_eq!(body["stream"], json!(true));
    }

    #[test]
    fn reads_cached_and_reasoning_tokens_from_openai_usage_shape() {
        let usage = json!({
            "prompt_tokens": 2006,
            "completion_tokens": 300,
            "total_tokens": 2306,
            "prompt_tokens_details": {
                "cached_tokens": 1920
            },
            "completion_tokens_details": {
                "reasoning_tokens": 17
            }
        });

        let prompt_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let completion_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let total_tokens = usage
            .get("total_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let prompt_tokens_details = usage
            .get("prompt_tokens_details")
            .or_else(|| usage.get("input_tokens_details"))
            .and_then(|v| v.as_object());
        let completion_tokens_details = usage
            .get("completion_tokens_details")
            .and_then(|v| v.as_object());

        let cached_tokens = prompt_tokens_details
            .and_then(|details| details.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let reasoning_tokens = completion_tokens_details
            .and_then(|details| details.get("reasoning_tokens"))
            .or_else(|| prompt_tokens_details.and_then(|details| details.get("reasoning_tokens")))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let token_usage = crate::runtime::drivers::TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            reasoning_tokens,
            cached_tokens,
        };

        assert_eq!(token_usage.cached_tokens, 1920);
        assert_eq!(token_usage.reasoning_tokens, 17);
    }

    #[test]
    fn maps_structured_image_parts_to_openai_content_array() {
        let message = Message::user_parts(vec![
            json!({ "type": "text", "text": "What is this?" }),
            json!({
                "type": "image",
                "data": "aW1hZ2U=",
                "encoding": "base64",
                "mimeType": "image/png",
                "detail": "auto"
            }),
        ]);

        let content = openai_content(&message);

        assert_eq!(
            content,
            json!([
                { "type": "text", "text": "What is this?" },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": "data:image/png;base64,aW1hZ2U=",
                        "detail": "auto"
                    }
                }
            ])
        );
    }

    #[test]
    fn encodes_utf8_inline_data_before_openai_submission() {
        let part = json!({
            "type": "image",
            "data": "hello",
            "encoding": "utf8",
            "mimeType": "text/plain"
        });

        assert_eq!(
            media_url(&part),
            Some("data:text/plain;base64,aGVsbG8=".to_string())
        );
    }
}
