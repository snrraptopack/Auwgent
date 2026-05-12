/// Integration tests for middleware provider request interception.
/// Tests that onLLMStart can mutate config, headers, provider, url
/// and that onError can trigger forceStart retries.
use async_trait::async_trait;
use ir_runtime::runtime::drivers::{ModelDriver, ModelEvent, ModelEventStream};
use ir_runtime::runtime::engine::AuwgentEngine;
use ir_runtime::runtime::engine_types::AsyncMiddlewareEventCallback;
use ir_runtime::runtime::session::Message;
use ir_runtime::types::AgentIR;
use serde_json::Value;
use std::sync::{Arc, Mutex};

// ═══════════════════════════════════════════════════════════════════════════
// Mock Driver — records all arguments for inspection
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct RecordedCall {
    config: Option<Value>,
    headers: Option<Value>,
}

struct MockDriver {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    response: Vec<ModelEvent>,
}

impl MockDriver {
    fn new(response: Vec<ModelEvent>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response,
        }
    }

    fn recorded_calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl ModelDriver for MockDriver {
    async fn stream_generate(
        &self,
        _model: &str,
        _messages: &[Message],
        config: Option<Value>,
        headers: Option<Value>,
    ) -> Result<ModelEventStream, String> {
        self.calls.lock().unwrap().push(RecordedCall {
            config: config.clone(),
            headers: headers.clone(),
        });
        let events = self.response.clone();
        Ok(Box::pin(futures_util::stream::iter(
            events.into_iter().map(Ok),
        )))
    }

    async fn embed(
        &self,
        _model: &str,
        _text: &str,
        _config: Option<Value>,
        _headers: Option<Value>,
    ) -> Result<Vec<f32>, String> {
        Ok(vec![])
    }

    async fn embed_batch(
        &self,
        _model: &str,
        _texts: &[String],
        _config: Option<Value>,
        _headers: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String> {
        Ok(vec![])
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn test_ir() -> AgentIR {
    serde_json::from_value(serde_json::json!({
        "name": "TestAgent",
        "modelConfig": [{
            "defaultConfig": {
                "model": { "type": "modelRef", "name": "test" },
                "prompt": { "type": "literal", "value": "You are a test agent." }
            }
        }],
        "tools": [],
        "workflows": [],
        "helpers": [],
        "components": [],
        "types": null,
        "output": null,
        "input": { "type": "string", "optional": false }
    }))
    .expect("valid test IR")
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn middleware_config_override_is_passed_to_driver() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    let driver = Arc::new(MockDriver::new(vec![
        ModelEvent::ContentChunk("[response_text]Hello[/response_text]".to_string()),
    ]));
    engine.register_driver("gemini", driver.clone());

    // Middleware that overrides config.temperature
    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("llm_start")) {
            return Box::pin(async move {
                Some(serde_json::json!({
                    "config": { "temperature": 0.42 }
                }).to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    engine.run(Some(serde_json::json!("hi")), None).await.unwrap();

    let calls = driver.recorded_calls();
    assert_eq!(calls.len(), 1);
    let config = calls[0].config.as_ref().unwrap();
    assert_eq!(config.get("temperature"), Some(&serde_json::json!(0.42)));
}

#[tokio::test]
async fn middleware_headers_are_passed_to_driver() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    let driver = Arc::new(MockDriver::new(vec![
        ModelEvent::ContentChunk("[response_text]Hello[/response_text]".to_string()),
    ]));
    engine.register_driver("gemini", driver.clone());

    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("llm_start")) {
            return Box::pin(async move {
                Some(serde_json::json!({
                    "headers": { "Authorization": "Bearer test-token" }
                }).to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    engine.run(Some(serde_json::json!("hi")), None).await.unwrap();

    let calls = driver.recorded_calls();
    assert_eq!(calls.len(), 1);
    let headers = calls[0].headers.as_ref().unwrap();
    assert_eq!(
        headers.get("Authorization"),
        Some(&serde_json::json!("Bearer test-token"))
    );
}

#[tokio::test]
async fn middleware_provider_override_switches_driver() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    let gemini_driver = Arc::new(MockDriver::new(vec![]));
    let openai_driver = Arc::new(MockDriver::new(vec![
        ModelEvent::ContentChunk("[response_text]Hello[/response_text]".to_string()),
    ]));

    engine.register_driver("gemini", gemini_driver.clone());
    engine.register_driver("openai", openai_driver.clone());

    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("llm_start")) {
            return Box::pin(async move {
                Some(serde_json::json!({
                    "provider": "openai"
                }).to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    engine.run(Some(serde_json::json!("hi")), None).await.unwrap();

    assert_eq!(gemini_driver.recorded_calls().len(), 0);
    assert_eq!(openai_driver.recorded_calls().len(), 1);
}

#[tokio::test]
async fn middleware_prompt_string_return_still_works() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    let driver = Arc::new(MockDriver::new(vec![
        ModelEvent::ContentChunk("[response_text]Hello[/response_text]".to_string()),
    ]));
    engine.register_driver("gemini", driver.clone());

    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("llm_start")) {
            return Box::pin(async move {
                // Old-style string return
                Some(serde_json::json!("modified prompt").to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    engine.run(Some(serde_json::json!("hi")), None).await.unwrap();

    let calls = driver.recorded_calls();
    assert_eq!(calls.len(), 1);
    // The turn input should have been modified
    let session = engine.session();
    let last_turn = session.turns.last().unwrap();
    assert_eq!(last_turn.input, "modified prompt");
}

#[tokio::test]
async fn parse_llm_start_response_extracts_all_fields() {
    use ir_runtime::runtime::middleware::parse_llm_start_response;

    let response = serde_json::json!({
        "prompt": "modified",
        "stack": ["Main"],
        "config": { "temperature": 0.5 },
        "provider": "openai",
        "url": "https://proxy.example.com",
        "headers": { "Authorization": "Bearer token" }
    });

    let parsed = parse_llm_start_response(&response);
    assert_eq!(parsed.prompt, Some("modified".to_string()));
    assert_eq!(parsed.stack, Some(vec!["Main".to_string()]));
    assert_eq!(parsed.config, Some(serde_json::json!({ "temperature": 0.5 })));
    assert_eq!(parsed.provider, Some("openai".to_string()));
    assert_eq!(parsed.url, Some("https://proxy.example.com".to_string()));
    assert_eq!(
        parsed.headers,
        Some(serde_json::json!({ "Authorization": "Bearer token" }))
    );
}

#[tokio::test]
async fn parse_error_response_extracts_swallow_and_force_start() {
    use ir_runtime::runtime::middleware::parse_error_response;

    let swallow = serde_json::json!({ "swallow": true });
    let parsed = parse_error_response(&swallow);
    assert!(parsed.swallow);
    assert_eq!(parsed.force_start, None);

    let force = serde_json::json!({ "forceStart": "llm_start" });
    let parsed = parse_error_response(&force);
    assert!(!parsed.swallow);
    assert_eq!(parsed.force_start, Some("llm_start".to_string()));

    let both = serde_json::json!({ "swallow": true, "forceStart": "run_start" });
    let parsed = parse_error_response(&both);
    assert!(parsed.swallow);
    assert_eq!(parsed.force_start, Some("run_start".to_string()));
}

#[tokio::test]
async fn session_pop_last_turn_if_empty_works() {
    use ir_runtime::runtime::session::SessionState;

    let mut session = SessionState::new();
    session.start_turn("hello");
    session.set_model_response("hi");
    session.start_turn("");

    assert_eq!(session.turns.len(), 2);
    session.pop_last_turn_if_empty();
    assert_eq!(session.turns.len(), 1);

    // Should not pop a turn with content
    session.start_turn("second");
    session.set_model_response("response");
    session.pop_last_turn_if_empty();
    assert_eq!(session.turns.len(), 2);
}

#[tokio::test]
async fn middleware_url_override_is_injected_into_config() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    let driver = Arc::new(MockDriver::new(vec![
        ModelEvent::ContentChunk("[response_text]Hello[/response_text]".to_string()),
    ]));
    engine.register_driver("gemini", driver.clone());

    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("llm_start")) {
            return Box::pin(async move {
                Some(serde_json::json!({
                    "url": "https://proxy.example.com/v1/chat/completions"
                }).to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    engine.run(Some(serde_json::json!("hi")), None).await.unwrap();

    let calls = driver.recorded_calls();
    assert_eq!(calls.len(), 1);
    let config = calls[0].config.as_ref().unwrap();
    assert_eq!(config.get("url"), Some(&serde_json::json!("https://proxy.example.com/v1/chat/completions")));
}

#[tokio::test]
async fn force_start_retry_limit_is_enforced() {
    let ir = test_ir();
    let engine = AuwgentEngine::new(ir);

    // Driver that always fails
    let fail_driver = Arc::new(FailingDriver);
    engine.register_driver("gemini", fail_driver.clone());

    let cb: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let event: Value = serde_json::from_str(&event_json).unwrap();
        if event.get("type") == Some(&serde_json::json!("error")) {
            return Box::pin(async move {
                Some(serde_json::json!({ "forceStart": "llm_start" }).to_string())
            });
        }
        Box::pin(async move { None })
    });
    engine.on_middleware_event(cb);

    let result = engine.run(Some(serde_json::json!("hi")), None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("exceeded max retries"), "Expected retry limit error, got: {}", err);
}

// Driver that always fails
struct FailingDriver;

#[async_trait]
impl ModelDriver for FailingDriver {
    async fn stream_generate(
        &self,
        _model: &str,
        _messages: &[Message],
        _config: Option<Value>,
        _headers: Option<Value>,
    ) -> Result<ModelEventStream, String> {
        Err("always fails".to_string())
    }

    async fn embed(
        &self,
        _model: &str,
        _text: &str,
        _config: Option<Value>,
        _headers: Option<Value>,
    ) -> Result<Vec<f32>, String> {
        Ok(vec![])
    }

    async fn embed_batch(
        &self,
        _model: &str,
        _texts: &[String],
        _config: Option<Value>,
        _headers: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn deep_merge_json_merges_nested_objects() {
    use ir_runtime::runtime::engine::deep_merge_json;

    let a = serde_json::json!({
        "temperature": 0.7,
        "top_p": 0.9,
        "nested": { "a": 1, "b": 2 }
    });
    let b = serde_json::json!({
        "temperature": 0.5,
        "nested": { "b": 3, "c": 4 }
    });

    let merged = deep_merge_json(a, b);
    assert_eq!(merged["temperature"], 0.5);
    assert_eq!(merged["top_p"], 0.9);
    assert_eq!(merged["nested"]["a"], 1);
    assert_eq!(merged["nested"]["b"], 3);
    assert_eq!(merged["nested"]["c"], 4);
}
