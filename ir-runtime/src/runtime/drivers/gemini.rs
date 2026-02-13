use crate::runtime::drivers::ModelDriver;
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
    async fn stream_generate_content(
        &self,
        model: &str,
        prompt: &str,
        system_instruction: Option<&str>,
        config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
        // Construct URL with dynamic model
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, self.api_key
        );

        let mut body = json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }]
        });

        if let Some(si) = system_instruction {
            body.as_object_mut().unwrap().insert(
                "system_instruction".to_string(),
                json!({
                    "parts": [{ "text": si }]
                }),
            );
        }

        if let Some(cfg) = config {
            let mut gen_config = serde_json::Map::new();
            if let Some(temp) = cfg.get("temperature") {
                gen_config.insert("temperature".to_string(), temp.clone());
            }
            if let Some(top_p) = cfg.get("topP") {
                gen_config.insert("topP".to_string(), top_p.clone());
            }
            if let Some(top_k) = cfg.get("topK") {
                gen_config.insert("topK".to_string(), top_k.clone());
            }
            if let Some(stop) = cfg.get("stopSequences") {
                gen_config.insert("stopSequences".to_string(), stop.clone());
            }
            if let Some(think) = cfg.get("thinkingConfig") {
                gen_config.insert("thinkingConfig".to_string(), think.clone());
            }

            if !gen_config.is_empty() {
                body.as_object_mut()
                    .unwrap()
                    .insert("generationConfig".to_string(), Value::Object(gen_config));
            }
        }

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

        // --- Robust SSE Stream Handling ---
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
}
