use crate::runtime::drivers::ModelDriver;
use crate::runtime::session::{Message, Role};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use std::pin::Pin;

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

#[async_trait]
impl ModelDriver for GeminiDriver {
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
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
                json!({ "parts": [{ "text": sys_msg.content }] }),
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
                        "parts": [{ "text": m.content }]
                    }),
                    Role::Model => json!({
                        "role": "model",
                        "parts": [{ "text": m.content }]
                    }),
                    Role::ToolResult => json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": "tool_result",
                                "response": { "output": m.content }
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
        let stream = response.bytes_stream().map(move |item| match item {
            Ok(bytes) => {
                let chunk = String::from_utf8_lossy(&bytes);
                buffer.push_str(&chunk);

                let mut result_text = String::new();
                while let Some(index) = buffer.find('\n') {
                    let line = buffer.drain(..=index).collect::<String>();
                    let trimmed = line.trim();

                    if let Some(data) = trimmed.strip_prefix("data: ") {
                        if let Ok(json_val) = serde_json::from_str::<Value>(data) {
                            if let Some(t) =
                                json_val["candidates"][0]["content"]["parts"][0]["text"].as_str()
                            {
                                result_text.push_str(t);
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

    async fn embed(&self, model: &str, text: &str, config: Option<Value>) -> Result<Vec<f32>, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            model, self.api_key
        );

        let mut body = json!({
            "model": format!("models/{}", model),
            "content": { "parts": [{ "text": text }] }
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
        let values = embedding.as_array().ok_or_else(|| {
            format!(
                "Missing 'embedding.values' field in response: {}",
                json_val.to_string()
            )
        })?;

        Ok(values
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect())
    }

    async fn embed_batch(&self, model: &str, texts: &[String], config: Option<Value>) -> Result<Vec<Vec<f32>>, String> {
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
                if let Some(cfg) = config.as_ref().and_then(|c| c.as_object()) {
                    if let Some(req_obj) = req.as_object_mut() {
                        for (k, v) in cfg {
                            req_obj.insert(k.clone(), v.clone());
                        }
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

        let embeddings = json_val["embeddings"].as_array().ok_or_else(|| {
            format!(
                "Missing 'embeddings' field in response: {}",
                json_val.to_string()
            )
        })?;

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
