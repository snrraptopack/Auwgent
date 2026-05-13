use async_trait::async_trait;
use futures_util::stream;
use ir_runtime::AgentIR;
use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::{ModelDriver, ModelEvent, ModelEventStream};
use ir_runtime::runtime::session::Message;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct EventDriver {
    events: Vec<ModelEvent>,
    captured_config: Option<Arc<Mutex<Option<Value>>>>,
}

#[async_trait]
impl ModelDriver for EventDriver {
    async fn stream_generate(
        &self,
        _model: &str,
        _messages: &[Message],
        config: Option<Value>,
        _headers: Option<Value>,
        _api_key: Option<String>,
    ) -> Result<ModelEventStream, String> {
        if let Some(captured) = &self.captured_config {
            *captured.lock().unwrap() = config;
        }

        Ok(Box::pin(stream::iter(
            self.events.clone().into_iter().map(Ok),
        )))
    }

    async fn embed(
        &self,
        _model: &str,
        _text: &str,
        _config: Option<Value>,
        _headers: Option<Value>,
        _api_key: Option<String>,
    ) -> Result<Vec<f32>, String> {
        Ok(Vec::new())
    }

    async fn embed_batch(
        &self,
        _model: &str,
        texts: &[String],
        _config: Option<Value>,
        _headers: Option<Value>,
        _api_key: Option<String>,
    ) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|_| Vec::new()).collect())
    }
}

fn native_ir(output: Option<Value>) -> AgentIR {
    native_ir_with_provider("gemini", output)
}

fn native_ir_with_provider(provider: &str, output: Option<Value>) -> AgentIR {
    serde_json::from_value(json!({
        "name": "NativeTerminalAgent",
        "modelConfig": [{
            "defaultConfig": {
                "model": {
                    "type": provider,
                    "modelName": "gemini-test",
                    "config": null
                },
                "prompt": { "type": "literal", "value": "Answer directly." },
                "toolProtocol": "native"
            },
            "namedConfig": []
        }],
        "input": null,
        "output": output,
        "context": null,
        "tools": [],
        "workflows": [],
        "helpers": [],
        "components": [],
        "tests": []
    }))
    .expect("valid native ir")
}

fn capture_intent_middleware(engine: &AuwgentEngine) -> Arc<Mutex<Vec<(String, Value)>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_handler = Arc::clone(&seen);
    engine.on_middleware_event(Arc::new(move |event_json| {
        let seen = Arc::clone(&seen_for_handler);
        Box::pin(async move {
            let event: Value = serde_json::from_str(&event_json).expect("event json");
            if event.get("type").and_then(Value::as_str) == Some("intent") {
                let name = event["name"].as_str().unwrap_or("").to_string();
                let value = event["value"].clone();
                seen.lock().unwrap().push((name, value));
            }
            None
        })
    }));
    seen
}

#[tokio::test]
async fn native_structured_output_uses_intent_middleware_path() {
    let engine = AuwgentEngine::new(native_ir(Some(json!({
        "status": { "type": "string", "optional": false }
    }))));
    engine.register_driver(
        "gemini",
        Arc::new(EventDriver {
            events: vec![ModelEvent::NativeStructuredOutput(
                json!({ "status": "ok" }),
            )],
            captured_config: None,
        }),
    );
    let seen = capture_intent_middleware(&engine);

    engine.run(Some(json!("go")), None).await.unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|(name, value)| {
        name == "response_schema"
            && value
                == &json!({
                    "type": "Output",
                    "response": { "status": "ok" }
                })
    }));
}

#[tokio::test]
async fn native_text_fallback_uses_intent_middleware_path() {
    let engine = AuwgentEngine::new(native_ir(None));
    engine.register_driver(
        "gemini",
        Arc::new(EventDriver {
            events: vec![ModelEvent::ContentChunk("plain native text".to_string())],
            captured_config: None,
        }),
    );
    let seen = capture_intent_middleware(&engine);

    engine.run(Some(json!("go")), None).await.unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|(name, value)| {
        name == "response_text" && value == &json!({ "text": "plain native text" })
    }));
}

#[tokio::test]
async fn native_groq_receives_openai_compatible_output_schema() {
    let captured_config = Arc::new(Mutex::new(None));
    let engine = AuwgentEngine::new(native_ir_with_provider(
        "groq",
        Some(json!({
            "user_name": { "type": "string", "optional": false },
            "age": { "type": "number", "optional": false }
        })),
    ));
    engine.register_driver(
        "groq",
        Arc::new(EventDriver {
            events: vec![ModelEvent::NativeStructuredOutput(json!({
                "user_name": "Theo",
                "age": 22
            }))],
            captured_config: Some(Arc::clone(&captured_config)),
        }),
    );

    engine.run(Some(json!("go")), None).await.unwrap();

    let config = captured_config
        .lock()
        .unwrap()
        .clone()
        .expect("native config");
    assert_eq!(
        config["auwgent_native_output_schema"]["type"],
        json!("json_schema")
    );
    assert_eq!(
        config["auwgent_native_output_schema"]["json_schema"]["schema"]["properties"]["user_name"]
            ["type"],
        json!("string")
    );
}

#[tokio::test]
async fn native_json_text_fallback_uses_block_compatible_schema_shape() {
    let engine = AuwgentEngine::new(native_ir(Some(json!({
        "user_name": { "type": "string", "optional": false },
        "age": { "type": "number", "optional": false }
    }))));
    engine.register_driver(
        "gemini",
        Arc::new(EventDriver {
            events: vec![ModelEvent::ContentChunk(
                "{\"user_name\":\"Theo\",\"age\":22}".to_string(),
            )],
            captured_config: None,
        }),
    );
    let seen = capture_intent_middleware(&engine);

    engine.run(Some(json!("go")), None).await.unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|(name, value)| {
        name == "response_schema"
            && value
                == &json!({
                    "type": "Output",
                    "response": {
                        "user_name": "Theo",
                        "age": 22
                    }
                })
    }));
}
