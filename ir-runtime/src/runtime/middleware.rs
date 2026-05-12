use crate::runtime::{
    engine_types::{AsyncMiddlewareEventCallback, IntentControl, TurnMetadata},
    middleware_event::{ErrorPayload, RunCompletePayload},
};
use serde_json::Value;

use crate::runtime::middleware_event::{
    EventContext, IntentPayload, LlmEndPayload, LlmStartPayload, MiddlewareEvent, RunStartPayload,
};

pub async fn fire_middleware_event(
    handler: Option<AsyncMiddlewareEventCallback>,
    event: Value,
) -> Option<Value> {
    let payload = serde_json::to_string(&event).ok()?;
    let response = handler?.clone()(payload).await?;
    serde_json::from_str(&response).ok()
}

pub fn parse_intent_control_response(response: &Value) -> Option<IntentControl> {
    match response {
        Value::Null => None,
        Value::Object(obj) => {
            if obj.get("skip").and_then(Value::as_bool) == Some(true) {
                Some(IntentControl::Skip)
            } else {
                obj.get("result")
                    .cloned()
                    .map(|result| IntentControl::Override { result })
            }
        }
        _ => None,
    }
}

/// Result of parsing an llm_start middleware response.
#[derive(Debug, Clone, Default)]
pub struct LlmStartMiddlewareResult {
    pub prompt: Option<String>,
    pub stack: Option<Vec<String>>,
    pub config: Option<Value>,
    pub provider: Option<String>,
    pub url: Option<String>,
    pub headers: Option<Value>,
}

/// Parse the JSON response from an llm_start middleware event.
///
/// Supports two shapes for backward compatibility:
/// - A plain string: treated as the new prompt.
/// - An object with optional `prompt`, `stack`, `config`, `provider`, `url`, `headers`.
pub fn parse_llm_start_response(response: &Value) -> LlmStartMiddlewareResult {
    let mut result = LlmStartMiddlewareResult::default();

    // Backward compatibility: bare string return means "replace prompt"
    if let Value::String(s) = response {
        result.prompt = Some(s.clone());
        return result;
    }

    let Some(obj) = response.as_object() else {
        return result;
    };
    result.prompt = obj.get("prompt").and_then(Value::as_str).map(ToString::to_string);
    result.stack = obj.get("stack").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter().filter_map(Value::as_str).map(ToString::to_string).collect()
        })
    });
    result.config = obj.get("config").cloned();
    result.provider = obj.get("provider").and_then(Value::as_str).map(ToString::to_string);
    result.url = obj.get("url").and_then(Value::as_str).map(ToString::to_string);
    result.headers = obj.get("headers").cloned();
    result
}

/// Result of parsing an error middleware response.
#[derive(Debug, Clone, Default)]
pub struct ErrorMiddlewareResult {
    pub swallow: bool,
    pub force_start: Option<String>,
}

/// Parse the JSON response from an error middleware event.
pub fn parse_error_response(response: &Value) -> ErrorMiddlewareResult {
    let mut result = ErrorMiddlewareResult::default();
    let Some(obj) = response.as_object() else {
        return result;
    };
    result.swallow = obj.get("swallow").and_then(Value::as_bool) == Some(true);
    result.force_start = obj.get("forceStart").and_then(Value::as_str).map(ToString::to_string);
    result
}

pub async fn apply_intent_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    payload: IntentPayload,
) -> Option<IntentControl> {
    let event = MiddlewareEvent::Intent(payload);
    let event_value = serde_json::to_value(event).unwrap();
    let response = fire_middleware_event(handler, event_value).await?;
    parse_intent_control_response(&response)
}

pub async fn apply_llm_start_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    payload: LlmStartPayload,
) -> Option<Value> {
    let event = MiddlewareEvent::LlmStart(payload);
    let event_value = serde_json::to_value(event).unwrap();
    fire_middleware_event(handler, event_value).await
}

pub async fn notify_llm_end_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    response: &Value,
    context: EventContext,
    turn_metadata: &TurnMetadata,
) {
    let mut response_with_metadata = response.clone();
    if let Some(obj) = response_with_metadata.as_object_mut() {
        obj.insert(
            "metadata".to_string(),
            serde_json::to_value(turn_metadata).unwrap_or(Value::Null),
        );
    } else if let Value::String(s) = response {
        response_with_metadata = serde_json::json!({
            "text": s,
            "metadata": turn_metadata
        });
    } else {
        response_with_metadata = serde_json::json!({
            "value": response,
            "metadata": turn_metadata
        });
    }
    let event = MiddlewareEvent::LlmEnd(LlmEndPayload {
        response: response_with_metadata,
        context: context,
    });
    let event_value = serde_json::to_value(event).unwrap();
    fire_middleware_event(handler, event_value).await;
}

pub async fn apply_run_start_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    payload: RunStartPayload,
) -> Option<Value> {
    let event = MiddlewareEvent::RunStart(payload);
    let event_value = serde_json::to_value(event).unwrap();
    fire_middleware_event(handler, event_value).await
}

pub async fn apply_run_complete_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    payload: RunCompletePayload,
) -> Option<Value> {
    let event = MiddlewareEvent::RunComplete(payload);
    let event_value = serde_json::to_value(event).unwrap();
    fire_middleware_event(handler, event_value).await
}

pub async fn apply_error_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    payload: ErrorPayload,
) -> Option<Value> {
    let event = MiddlewareEvent::Error(payload);
    let event_value = serde_json::to_value(event).unwrap();
    fire_middleware_event(handler, event_value).await
}
