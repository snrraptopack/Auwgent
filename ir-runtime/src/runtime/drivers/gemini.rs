use crate::runtime::drivers::{
    FinishReason, ModelDriver, ModelEvent, ModelEventStream, ModelMetadata, TokenUsage,
};
use crate::runtime::session::{Message, MessageContent, Role};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use serde_json::json;

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

    async fn generate_content_once(
        &self,
        model: &str,
        body: Value,
    ) -> Result<ModelEventStream, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, self.api_key
        );

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

        let json_val: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

        println!(
            "RAW GEMINI UNARY RESPONSE: {}",
            serde_json::to_string_pretty(&json_val).unwrap()
        );

        Ok(Box::pin(futures_util::stream::iter(
            gemini_response_events(&json_val).into_iter().map(Ok),
        )))
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
        // ── Build request body from messages ──────────────────────────────
        let mut body = json!({});
        let body_obj = body.as_object_mut().expect("body is always an object");

        // Extract system instruction (first System message, if any)
        // turn this on later
        // once removed the sys_msg to test of the gemini-2.5-flash-image model still had
        // no response...
        if let Some(sys_msg) = messages.iter().find(|m| m.role == Role::System) {
            body_obj.insert(
                "system_instruction".to_string(),
                json!({ "parts": [{ "text": sys_msg.content.text() }] }),
            );
        }

        // Build contents array from non-system messages.
        let contents = gemini_contents(messages);

        body_obj.insert("contents".to_string(), json!(contents));

        // ── Native tools / structured output from config ──────────────────
        let native_tools = config.as_ref().and_then(|cfg| {
            cfg.get("auwgent_native_tools")
                .and_then(|v| v.as_array().cloned())
        });
        let native_output_schema = config
            .as_ref()
            .and_then(|cfg| cfg.get("auwgent_native_output_schema").cloned());

        if let Some(tools) = native_tools {
            body_obj.insert("tools".to_string(), json!(tools));
        }

        // ── Generation config ─────────────────────────────────────────────
        if let Some(cfg) = config {
            if let Some(cfg_object) = cfg.as_object() {
                let mut gen_config = serde_json::Map::new();

                for (key, value) in cfg_object {
                    if matches!(
                        key.as_str(),
                        "model"
                            | "contents"
                            | "systemInstruction"
                            | "tools"
                            | "toolConfig"
                            | "auwgent_native_tools"
                            | "auwgent_native_output_schema"
                    ) {
                        continue;
                    }
                    gen_config.insert(key.clone(), value.clone());
                }

                if let Some(fmt) = native_output_schema {
                    gen_config.insert("responseFormat".to_string(), fmt);
                }

                if !gen_config.is_empty() {
                    body_obj.insert("generationConfig".to_string(), Value::Object(gen_config));
                }
            }
        }
        // ── Send request ──────────────────────────────────────────────────
        if uses_non_streaming_generate_content(model) {
            return self.generate_content_once(model, body).await;
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model, self.api_key
        );

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
                            .map_err(|e| format!("Gemini stream contained invalid UTF-8: {}", e))?;
                        let trimmed = line.trim_end_matches(['\r', '\n']).trim_start();

                        if let Some(data) = trimmed.strip_prefix("data: ")
                            && let Ok(json_val) = serde_json::from_str::<Value>(data)
                        {
                            if let Some(error) = json_val["error"].get("message") {
                                return Err(format!("Gemini streaming error: {}", error));
                            }
                            result_events.extend(gemini_response_events(&json_val));
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
                } else if let Some(file_uri) = part
                    .get("url")
                    .or_else(|| part.get("ref"))
                    .and_then(Value::as_str)
                {
                    out.push(file_data_part(part, file_uri));
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

fn gemini_contents(messages: &[Message]) -> Vec<Value> {
    let mut contents = Vec::new();
    let mut last_role: Option<&'static str> = None;

    for message in messages.iter().filter(|m| m.role != Role::System) {
        let (role, parts) = match message.role {
            Role::User => ("user", gemini_parts(message)),
            Role::Model => {
                let mut parts = Vec::new();
                // Include text if present
                let text = model_text_or_ack(&message.content);
                if !text.trim().is_empty() && text != "Acknowledged." {
                    parts.push(json!({ "text": text }));
                }
                // Include function calls if present
                if let Some(ref tcs) = message.tool_calls {
                    for tc in tcs {
                        if let Some(func) = tc.get("function") {
                            let name = func["name"].as_str().unwrap_or("");
                            let args = func
                                .get("arguments")
                                .and_then(|a| a.as_str())
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                .unwrap_or_else(|| {
                                    func.get("arguments").cloned().unwrap_or_else(|| json!({}))
                                });
                            parts.push(json!({
                                "functionCall": {
                                    "name": name,
                                    "args": args
                                }
                            }));
                        }
                    }
                }
                if parts.is_empty() {
                    parts.push(json!({ "text": "Acknowledged." }));
                }
                ("model", parts)
            }
            Role::ToolResult => {
                // Native mode tool results have a tool_call_id or name set.
                // Block mode tool results use the legacy "tool_result" hardcoded name.
                let (name, response) = if message.tool_call_id.is_some() || message.name.is_some() {
                    let name = message
                        .name
                        .as_deref()
                        .or_else(|| message.tool_call_id.as_deref())
                        .unwrap_or("tool_result");
                    let response = serde_json::from_str::<Value>(&message.content.text())
                        .unwrap_or_else(|_| json!({ "output": message.content.text() }));
                    (name, response)
                } else {
                    ("tool_result", json!({ "output": message.content.text() }))
                };
                (
                    "user",
                    vec![json!({
                        "functionResponse": {
                            "name": name,
                            "response": response
                        }
                    })],
                )
            }
            Role::System => unreachable!(),
        };

        if role == "user" && last_role == Some("user") {
            contents.push(json!({
                "role": "model",
                "parts": [{ "text": "Acknowledged." }]
            }));
        }

        contents.push(json!({
            "role": role,
            "parts": parts
        }));
        last_role = Some(role);
    }

    contents
}

fn model_text_or_ack(content: &MessageContent) -> String {
    let text = content.text();
    if text.trim().is_empty() {
        "Acknowledged.".to_string()
    } else {
        text
    }
}

fn uses_non_streaming_generate_content(model: &str) -> bool {
    let normalized = model.to_ascii_lowercase();
    normalized.contains("-image") || normalized.contains("image-preview")
}

fn gemini_response_events(json_val: &Value) -> Vec<ModelEvent> {
    let mut events = Vec::new();
    let finish_reason = json_val["candidates"]
        .get(0)
        .and_then(|candidate| candidate["finishReason"].as_str())
        .map(gemini_finish_reason);

    if let Some(candidate) = json_val["candidates"].get(0) {
        // Text parts
        for text in candidate_text_parts(candidate) {
            events.push(ModelEvent::ContentChunk(text));
        }

        // Function call parts
        for fc in candidate_function_call_parts(candidate) {
            events.push(ModelEvent::NativeToolCall {
                id: fc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                provider_name: fc["name"].as_str().unwrap_or("").to_string(),
                arguments: fc.get("args").cloned().unwrap_or_else(|| json!({})),
            });
        }
    }

    for text in step_text_parts(json_val) {
        events.push(ModelEvent::ContentChunk(text));
    }

    if let Some(usage) = json_val.get("usageMetadata") {
        events.push(ModelEvent::Metadata(ModelMetadata {
            usage: gemini_usage(usage),
            finish_reason,
        }));
    } else if let Some(reason) = finish_reason {
        events.push(ModelEvent::FinishReason(reason));
    } else if let Some(error) = json_val["error"].get("message") {
        events.push(ModelEvent::ContentChunk(format!(
            "(error: {})",
            error.as_str().unwrap_or("Gemini response error")
        )));
    }

    events
}

fn step_text_parts(json_val: &Value) -> Vec<String> {
    json_val["steps"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .filter_map(|step| {
                    let step_type = step["type"].as_str();
                    if matches!(step_type, Some("text" | "output_text" | "message")) {
                        step["text"].as_str().map(ToString::to_string)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn gemini_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "STOP" => FinishReason::Stop,
        "MAX_TOKENS" => FinishReason::Length,
        "SAFETY" | "BLOCKLIST" => FinishReason::ContentFilter,
        "OTHER" => FinishReason::Other(reason.to_string()),
        _ => FinishReason::Other(reason.to_string()),
    }
}

fn gemini_usage(usage: &Value) -> TokenUsage {
    TokenUsage {
        prompt_tokens: usage["promptTokenCount"].as_u64().unwrap_or(0) as u32,
        completion_tokens: usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
        total_tokens: usage["totalTokenCount"].as_u64().unwrap_or(0) as u32,
        reasoning_tokens: usage["thoughtsTokenCount"].as_u64().unwrap_or(0) as u32,
        cached_tokens: usage["cachedTokenCount"].as_u64().unwrap_or(0) as u32,
    }
}

fn candidate_text_parts(candidate: &Value) -> Vec<String> {
    candidate["content"]["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part["text"].as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn candidate_function_call_parts(candidate: &Value) -> Vec<Value> {
    candidate["content"]["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("functionCall").cloned())
                .collect()
        })
        .unwrap_or_default()
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
            general_purpose::STANDARD.encode(data.as_bytes())
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
                "data": general_purpose::STANDARD.encode(&bytes)
            }
        }));
    }
    None
}

fn file_data_part(part: &Value, file_uri: &str) -> Value {
    let mime = part
        .get("mimeType")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    json!({
        "file_data": {
            "mime_type": mime,
            "file_uri": file_uri
        }
    })
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

    #[test]
    fn maps_ref_to_gemini_file_uri() {
        let message = Message::user_parts(vec![json!({
            "type": "file",
            "ref": "files/report_pdf",
            "mimeType": "application/pdf"
        })]);

        assert_eq!(
            gemini_parts(&message),
            vec![json!({
                "file_data": {
                    "mime_type": "application/pdf",
                    "file_uri": "files/report_pdf"
                }
            })]
        );
    }

    #[test]
    fn image_output_models_use_generate_content_response_path() {
        assert!(uses_non_streaming_generate_content(
            "gemini-2.5-flash-image"
        ));
        assert!(uses_non_streaming_generate_content(
            "gemini-3-pro-image-preview"
        ));
        assert!(!uses_non_streaming_generate_content(
            "gemini-3-flash-preview"
        ));
        assert!(!uses_non_streaming_generate_content("gemini-2.5-flash"));
    }

    #[test]
    fn wraps_generate_content_response_as_model_events() {
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "[response_text]ok[/response_text]" }
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 4,
                "totalTokenCount": 14,
                "thoughtsTokenCount": 2,
                "cachedTokenCount": 1
            }
        });

        let events = gemini_response_events(&response);
        assert!(matches!(
            &events[0],
            ModelEvent::ContentChunk(text) if text == "[response_text]ok[/response_text]"
        ));
        assert!(matches!(
            &events[1],
            ModelEvent::Metadata(meta)
                if meta.finish_reason == Some(FinishReason::Stop)
                    && meta.usage.prompt_tokens == 10
                    && meta.usage.completion_tokens == 4
                    && meta.usage.reasoning_tokens == 2
                    && meta.usage.cached_tokens == 1
        ));
    }

    #[test]
    fn emits_metadata_without_candidate_chunk() {
        let response = json!({
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 0,
                "totalTokenCount": 12,
                "thoughtsTokenCount": 2
            }
        });

        let events = gemini_response_events(&response);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            ModelEvent::Metadata(meta)
                if meta.finish_reason.is_none()
                    && meta.usage.prompt_tokens == 10
                    && meta.usage.reasoning_tokens == 2
        ));
    }

    #[test]
    fn extracts_text_from_steps_response_shape() {
        let response = json!({
            "steps": [
                { "type": "thinking", "thought": "internal reasoning" },
                { "type": "text", "text": "[response_text]ok[/response_text]" }
            ],
            "usageMetadata": {
                "promptTokenCount": 1,
                "candidatesTokenCount": 1,
                "totalTokenCount": 2
            }
        });

        let events = gemini_response_events(&response);
        assert!(matches!(
            &events[0],
            ModelEvent::ContentChunk(text) if text == "[response_text]ok[/response_text]"
        ));
        assert!(matches!(&events[1], ModelEvent::Metadata(_)));
    }

    #[test]
    fn inserts_ack_between_consecutive_gemini_user_messages() {
        let messages = vec![
            Message::system("system"),
            Message::user("[binding]\n@@name is \"Ada\"\n[/binding]"),
            Message::user_parts(vec![
                json!({ "type": "text", "text": "What is this?" }),
                json!({
                    "type": "image",
                    "data": "aW1hZ2U=",
                    "encoding": "base64",
                    "mimeType": "image/png"
                }),
            ]),
        ];

        assert_eq!(
            gemini_contents(&messages),
            vec![
                json!({
                    "role": "user",
                    "parts": [{ "text": "[binding]\n@@name is \"Ada\"\n[/binding]" }]
                }),
                json!({
                    "role": "model",
                    "parts": [{ "text": "Acknowledged." }]
                }),
                json!({
                    "role": "user",
                    "parts": [
                        { "text": "What is this?" },
                        {
                            "inline_data": {
                                "mime_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }
                    ]
                })
            ]
        );
    }

    #[test]
    fn extracts_text_from_all_gemini_candidate_parts() {
        let candidate = json!({
            "content": {
                "parts": [
                    {
                        "inline_data": {
                            "mime_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    },
                    { "text": "This image shows " },
                    { "text": "a processor die." }
                ]
            }
        });

        assert_eq!(
            candidate_text_parts(&candidate),
            vec![
                "This image shows ".to_string(),
                "a processor die.".to_string()
            ]
        );
    }

    #[test]
    fn extracts_function_call_parts_from_candidate() {
        let candidate = json!({
            "content": {
                "parts": [
                    { "text": "I'll search for that." },
                    {
                        "functionCall": {
                            "name": "tool_search",
                            "args": { "query": "hello" },
                            "id": "fc_123"
                        }
                    }
                ]
            }
        });

        let calls = candidate_function_call_parts(&candidate);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"], "tool_search");
        assert_eq!(calls[0]["args"], json!({ "query": "hello" }));
        assert_eq!(calls[0]["id"], "fc_123");
    }

    #[test]
    fn gemini_response_events_emits_native_tool_calls() {
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "I'll search for that." },
                        {
                            "functionCall": {
                                "name": "tool_search",
                                "args": { "query": "hello" },
                                "id": "fc_123"
                            }
                        }
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 4,
                "totalTokenCount": 14
            }
        });

        let events = gemini_response_events(&response);

        // First event: text chunk
        assert!(matches!(
            &events[0],
            ModelEvent::ContentChunk(text) if text == "I'll search for that."
        ));

        // Second event: native tool call
        assert!(matches!(
            &events[1],
            ModelEvent::NativeToolCall { id, provider_name, arguments }
                if id.as_deref() == Some("fc_123")
                && provider_name == "tool_search"
                && arguments == &json!({ "query": "hello" })
        ));

        // Third event: metadata
        assert!(matches!(
            &events[2],
            ModelEvent::Metadata(meta)
                if meta.finish_reason == Some(FinishReason::Stop)
        ));
    }
}
