use crate::runtime::engine_types::{AsyncMiddlewareEventCallback, IntentControl, TurnMetadata};
use serde_json::Value;

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

pub async fn apply_intent_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    event: Value,
) -> Option<IntentControl> {
    let response = fire_middleware_event(handler, event).await?;
    parse_intent_control_response(&response)
}

pub async fn apply_llm_start_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    event: Value,
) -> Option<Value> {
    fire_middleware_event(handler, event).await
}

pub async fn notify_llm_end_middleware(
    handler: Option<AsyncMiddlewareEventCallback>,
    response: &Value,
    context: Value,
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

    let _ = fire_middleware_event(
        handler,
        serde_json::json!({
            "type": "llm_end",
            "response": response_with_metadata,
            "context": context,
        }),
    )
    .await;
}
