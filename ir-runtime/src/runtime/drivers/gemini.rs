use crate::runtime::drivers::{ModelDriver, ModelEventStream};
use crate::runtime::session::{Message, Role};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};

pub struct GeminiDriver {
    client: Client,
    api_key: String,
}

impl GeminiDriver {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelDriver for GeminiDriver {
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
    ) -> Result<ModelEventStream, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, self.api_key
        );

        // ── Build request body from messages ──────────────────────────────
        let mut body = json!({});
        let body_obj = body.as_object_mut().expect("body is always an object");

        // Extract system instruction (first System message, if any)
        if let Some(sys_msg) = messages.iter().find(|m| m.role == Role::System) {
            body_obj.insert(
                "system_instruction".to_string(),
                json!({ "parts": [{ "text": sys_msg.content.text() }] }),
            );
        }

        // Build contents array from non-system messages
        // Gemini expects alternating user/model roles
        let contents: Vec<Value> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| {
                match m.role {
                    Role::User => json!({
                        "role": "user",
                        "parts": gemini_parts(m)
                    }),
                    Role::Model => json!({
                        "role": "model",
                        "parts": [{ "text": m.content.text() }]
                    }),
                    Role::ToolResult => json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": "tool_result",
                                "response": { "output": m.content.text() }
                            }
                        }]
                    }),
                    Role::System => unreachable!(), // filtered above
                }
            })
            .collect();

        body_obj.insert("contents".to_string(), json!(contents));

        // ── Generation config ─────────────────────────────────────────────
        if let Some(cfg) = config {
            let mut gen_config = serde_json::Map::new();
            for key in &[
                "temperature",
                "topP",
                "topK",
                "stopSequences",
                "thinkingConfig",
                "maxOutputTokens",
                "responseMimeType",
            ] {
                if let Some(val) = cfg.get(*key) {
                    gen_config.insert(key.to_string(), val.clone());
                }
            }
            if !gen_config.is_empty() {
                body_obj.insert("generationConfig".to_string(), Value::Object(gen_config));
            }
        }

        // ── Send request ──────────────────────────────────────────────────
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Gemini: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Gemini API error ({}): {}", status, error_text));
        }

        // ── SSE stream parsing ────────────────────────────────────────────
        let mut buffer = String::new();
        let stream = response
            .bytes_stream()
            .map(move |item| match item {
                Ok(bytes) => {
                    let chunk = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&chunk);

                    let mut result_events = Vec::new();
                    while let Some(index) = buffer.find('\n') {
                        let line = buffer.drain(..=index).collect::<String>();
                        let trimmed = line.trim();

                        if let Some(data) = trimmed.strip_prefix("data: ")
                            && let Ok(json_val) = serde_json::from_str::<Value>(data)
                        {
                            if let Some(candidate) = json_val["candidates"].get(0) {
                                if let Some(t) = candidate["content"]["parts"][0]["text"].as_str() {
                                    result_events.push(
                                        crate::runtime::drivers::ModelEvent::ContentChunk(
                                            t.to_string(),
                                        ),
                                    );
                                }

                                if let Some(finish_reason_str) = candidate["finishReason"].as_str()
                                {
                                    let finish_reason = match finish_reason_str {
                                        "STOP" => Some(crate::runtime::drivers::FinishReason::Stop),
                                        "MAX_TOKENS" => {
                                            Some(crate::runtime::drivers::FinishReason::Length)
                                        }
                                        "SAFETY" | "BLOCKLIST" => Some(
                                            crate::runtime::drivers::FinishReason::ContentFilter,
                                        ),
                                        "OTHER" => {
                                            Some(crate::runtime::drivers::FinishReason::Other(
                                                finish_reason_str.to_string(),
                                            ))
                                        }
                                        _ => Some(crate::runtime::drivers::FinishReason::Other(
                                            finish_reason_str.to_string(),
                                        )),
                                    };

                                    // Normally usage data is in the same JSON object as STOP
                                    if let Some(usage) = json_val.get("usageMetadata") {
                                        let prompt_tokens =
                                            usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
                                        let completion_tokens =
                                            usage["candidatesTokenCount"].as_u64().unwrap_or(0)
                                                as u32;
                                        let total_tokens =
                                            usage["totalTokenCount"].as_u64().unwrap_or(0) as u32;
                                        let cached_tokens =
                                            usage["cachedTokenCount"].as_u64().unwrap_or(0) as u32;
                                        let reasoning_tokens =
                                            usage["thoughtsTokenCount"].as_u64().unwrap_or(0)
                                                as u32;

                                        result_events.push(
                                            crate::runtime::drivers::ModelEvent::Metadata(
                                                crate::runtime::drivers::ModelMetadata {
                                                    usage: crate::runtime::drivers::TokenUsage {
                                                        prompt_tokens,
                                                        completion_tokens,
                                                        total_tokens,
                                                        reasoning_tokens,
                                                        cached_tokens,
                                                    },
                                                    finish_reason,
                                                },
                                            ),
                                        );
                                    }
                                }
                            } else if let Some(error) = json_val["error"].get("message") {
                                return Err(format!("Gemini streaming error: {}", error));
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
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            model, self.api_key
        );

        let mut body = json!({
            "model": format!("models/{}", model),
            "content": { "parts": [{ "text": text }] }
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
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send embedding request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "Gemini Embedding API error ({}): {}",
                status, error_text
            ));
        }

        let json_val: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        let embedding = &json_val["embedding"]["values"];
        let values = embedding
            .as_array()
            .ok_or_else(|| format!("Missing 'embedding.values' field in response: {}", json_val))?;

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
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:batchEmbedContents?key={}",
            model, self.api_key
        );

        let requests: Vec<Value> = texts
            .iter()
            .map(|t| {
                let mut req = json!({
                    "model": format!("models/{}", model),
                    "content": { "parts": [{ "text": t }] }
                });

                // Merge optional config into each request in the batch
                if let Some(cfg) = config.as_ref().and_then(|c| c.as_object())
                    && let Some(req_obj) = req.as_object_mut()
                {
                    for (k, v) in cfg {
                        req_obj.insert(k.clone(), v.clone());
                    }
                }
                req
            })
            .collect();

        let body = json!({ "requests": requests });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send embedding request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "Gemini Embedding API error ({}): {}",
                status, error_text
            ));
        }

        let json_val: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        let embeddings = json_val["embeddings"]
            .as_array()
            .ok_or_else(|| format!("Missing 'embeddings' field in response: {}", json_val))?;

        let mut results = Vec::new();
        for emb in embeddings {
            let values = emb["values"]
                .as_array()
                .ok_or_else(|| "Missing 'values' in embedding".to_string())?;
            let vec: Vec<f32> = values
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            results.push(vec);
        }

        Ok(results)
    }
}

fn gemini_parts(message: &Message) -> Vec<Value> {
    let Some(parts) = message.content.parts() else {
        return vec![json!({ "text": message.content.text() })];
    };

    let mut out = Vec::new();
    for part in parts {
        let Some(part_type) = part.get("type").and_then(Value::as_str) else {
            continue;
        };
        match part_type {
            "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    out.push(json!({ "text": text }));
                }
            }
            "image" | "file" | "audio" | "video" => {
                if let Some(inline) = inline_data_part(part) {
                    out.push(inline);
                } else if let Some(url) = part.get("url").and_then(Value::as_str) {
                    let mime = part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("application/octet-stream");
                    out.push(json!({
                        "file_data": {
                            "mime_type": mime,
                            "file_uri": url
                        }
                    }));
                } else {
                    out.push(json!({ "text": format!("[{}: {}]", part_type, media_label(part)) }));
                }
            }
            _ => {}
        }
    }

    if out.is_empty() {
        vec![json!({ "text": message.content.text() })]
    } else {
        out
    }
}

fn inline_data_part(part: &Value) -> Option<Value> {
    let mime = part
        .get("mimeType")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    if let Some(data) = part.get("data").and_then(Value::as_str) {
        let data = if matches!(
            part.get("encoding").and_then(Value::as_str),
            Some("utf8" | "utf-8")
        ) {
            encode_base64(data.as_bytes())
        } else {
            data.to_string()
        };
        return Some(json!({
            "inline_data": {
                "mime_type": mime,
                "data": data
            }
        }));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = part.get("path").and_then(Value::as_str)
        && let Ok(bytes) = std::fs::read(path)
    {
        return Some(json!({
            "inline_data": {
                "mime_type": mime,
                "data": encode_base64(&bytes)
            }
        }));
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

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_structured_media_parts_to_gemini_parts() {
        let message = Message::user_parts(vec![
            json!({ "type": "text", "text": "What is this?" }),
            json!({
                "type": "image",
                "data": "aW1hZ2U=",
                "encoding": "base64",
                "mimeType": "image/png"
            }),
        ]);

        assert_eq!(
            gemini_parts(&message),
            vec![
                json!({ "text": "What is this?" }),
                json!({
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": "aW1hZ2U="
                    }
                }),
            ]
        );
    }

    #[test]
    fn encodes_utf8_inline_data_before_gemini_submission() {
        let part = json!({
            "type": "file",
            "data": "hello",
            "encoding": "utf8",
            "mimeType": "text/plain"
        });

        assert_eq!(
            inline_data_part(&part),
            Some(json!({
                "inline_data": {
                    "mime_type": "text/plain",
                    "data": "aGVsbG8="
                }
            }))
        );
    }
}
