// Tool execution.
// Keep direct tool invocation, tool error shaping, and tool middleware/error
// handoff here. Do not mix workflow or helper orchestration into this file.
use super::*;
use auwgent_middleware;

impl AuwgentEngine {
    pub(super) async fn execute_tool(&self, call: &Value) -> AuwgentResult<(String, Value, Value)> {
        let tool_name = call["type"].as_str().unwrap_or("").to_string();
        let args = call["args"].clone();

        let imp = self.tools.lock().unwrap().get(&tool_name).cloned();
        if let Some(imp) = imp {
            match imp(args.clone()).await {
                Ok(val) => Ok((tool_name, args, val)),
                Err(e) => {
                    let error_value = serde_json::json!({
                        "tool": tool_name,
                        "message": e,
                    });
                    self.fire_generated_intent("tool_error".to_string(), error_value.clone())
                        .await;
                    let payload = ErrorPayload {
                        session: None,
                        context: self.build_event_context(&self.ir.name, None, None),
                        error: serde_json::json!({
                            "kind": "tool_error",
                            "tool": tool_name,
                            "message": e,
                        }),
                    };
                    let handler = self.middleware_event_handler.lock().unwrap().clone();
                    let _ = auwgent_middleware::apply_error_middleware(handler, payload).await;
                    Ok((tool_name, args, serde_json::json!({ "error": e })))
                }
            }
        } else {
            let message = format!("Tool not found: {}", tool_name);
            self.fire_generated_intent(
                "tool_error".to_string(),
                serde_json::json!({
                    "tool": tool_name,
                    "message": message,
                }),
            )
            .await;

            let payload = ErrorPayload {
                session: None,
                context: self.build_event_context(&self.ir.name, None, None),
                error: serde_json::json!({
                    "kind": "tool_error",
                    "tool": tool_name,
                    "message": message,
                }),
            };
            let handler = self.middleware_event_handler.lock().unwrap().clone();
            let _ = auwgent_middleware::apply_error_middleware(handler, payload).await;
            Ok((
                tool_name.clone(),
                args,
                serde_json::json!({ "error": format!("Tool '{}' is not registered", tool_name) }),
            ))
        }
    }
}
