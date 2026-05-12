use async_trait::async_trait;
use futures_util::stream;
use ir_runtime::AgentIR;
use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::{ModelDriver, ModelEvent, ModelEventStream};
use ir_runtime::runtime::session::{Message, MessageContent, SessionState};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct CaptureDriver {
    messages: Arc<Mutex<Vec<Message>>>,
}

#[async_trait]
impl ModelDriver for CaptureDriver {
    async fn stream_generate(
        &self,
        _model: &str,
        messages: &[Message],
        _config: Option<Value>,
    ) -> Result<ModelEventStream, String> {
        *self.messages.lock().unwrap() = messages.to_vec();
        Ok(Box::pin(stream::iter(vec![Ok(ModelEvent::ContentChunk(
            "[response_text]ok[/response_text]".to_string(),
        ))])))
    }

    async fn embed(
        &self,
        _model: &str,
        _text: &str,
        _config: Option<Value>,
    ) -> Result<Vec<f32>, String> {
        Ok(Vec::new())
    }

    async fn embed_batch(
        &self,
        _model: &str,
        texts: &[String],
        _config: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|_| Vec::new()).collect())
    }
}

fn build_ir() -> AgentIR {
    serde_json::from_value(json!({
        "name": "ImageAgent",
        "modelConfig": [
            {
                "defaultConfig": {
                    "model": {
                        "type": "gemini",
                        "modelName": "gemini-test",
                        "config": null
                    },
                    "prompt": { "type": "literal", "value": "Answer with response_text." }
                },
                "namedConfig": []
            }
        ],
        "input": "image",
        "output": null,
        "context": null,
        "tools": [],
        "workflows": [],
        "helpers": [],
        "components": [],
        "tests": []
    }))
    .expect("valid ir")
}

#[tokio::test]
async fn engine_run_persists_input_parts_for_array_input() {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let engine = AuwgentEngine::new(build_ir());
    engine.register_driver(
        "gemini",
        Arc::new(CaptureDriver {
            messages: Arc::clone(&messages),
        }),
    );

    let input = json!([
        { "type": "text", "text": "what is in this image?" },
        {
            "type": "image",
            "url": "https://example.com/image.png",
            "mimeType": "image/png"
        }
    ]);

    engine.run(Some(input.clone()), None).await.unwrap();

    let session: SessionState =
        serde_json::from_str(&engine.export_session().unwrap()).expect("valid session");
    assert_eq!(
        session.turns[0].input,
        "what is in this image?\n[image: https://example.com/image.png]"
    );
    assert_eq!(session.turns[0].input_parts, input.as_array().cloned());

    let captured = messages.lock().unwrap();
    assert!(captured.iter().any(|message| {
        matches!(
            &message.content,
            MessageContent::Parts(parts) if Some(parts) == input.as_array()
        )
    }));
}

#[tokio::test]
async fn llm_start_middleware_does_not_drop_input_parts() {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let engine = AuwgentEngine::new(build_ir());
    engine.register_driver(
        "gemini",
        Arc::new(CaptureDriver {
            messages: Arc::clone(&messages),
        }),
    );
    engine.on_middleware_event(Arc::new(|event_json| {
        Box::pin(async move {
            let event: Value = serde_json::from_str(&event_json).expect("event json");
            if event.get("type").and_then(Value::as_str) == Some("llm_start") {
                let prompt = event
                    .get("prompt")
                    .cloned()
                    .unwrap_or(Value::String(String::new()));
                return Some(json!({ "prompt": prompt }).to_string());
            }
            if event.get("type").and_then(Value::as_str) == Some("run_start") {
                return Some(json!({ "session": event["session"].clone() }).to_string());
            }
            None
        })
    }));

    let input = json!([
        { "type": "text", "text": "what is in this image?" },
        {
            "type": "image",
            "url": "https://example.com/image.png",
            "mimeType": "image/png"
        }
    ]);

    engine.run(Some(input.clone()), None).await.unwrap();

    let session: SessionState =
        serde_json::from_str(&engine.export_session().unwrap()).expect("valid session");
    assert_eq!(session.turns[0].input_parts, input.as_array().cloned());
}
