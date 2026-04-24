// Core engine shell.
// Keep shared engine state, construction, embedding helpers, and cross-module
// utilities here. Move prompt building, runtime loop logic, and execution
// behavior into the dedicated engine submodules.
use crate::errors::{AuwgentError, AuwgentResult};
use crate::evaluator::Evaluator;
use crate::runtime::drivers::{FinishReason, ModelDriver, ModelEvent, TokenUsage};
pub use crate::runtime::engine_types::{
    AsyncIntentCallback, AsyncMiddlewareEventCallback, AsyncSessionPreloadCallback,
    IntentCallback, IntentControl, RunMetadata, SessionSaveCallback, ToolImplementation,
    TurnMetadata,
};
use crate::runtime::middleware;
use crate::runtime::middleware_event::{
    ErrorPayload, EventContext, IntentPayload, LlmStartPayload, RunCompletePayload,
    RunStartPayload,
};
use crate::runtime::session::SessionState;
use crate::runtime::streaming::parser::block_orchestrator::BlockOrchestrator as Orchestrator;
use crate::runtime::streaming::{JsonlEventBuffer, PartialIntentState, StructuredOutputEvent};
use crate::types::*;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

mod execution;
mod prompt;
mod runtime_loop;

fn empty_response_marker(finish_reason: Option<&FinishReason>) -> String {
    match finish_reason {
        Some(FinishReason::ContentFilter) => {
            "(no response: blocked by model safety/content filter)".to_string()
        }
        Some(FinishReason::Length) => {
            "(no response: generation ended due to max token limit)".to_string()
        }
        Some(FinishReason::ToolCalls) => {
            "(no response: model indicated tool calls but no parsable tool intent was produced)"
                .to_string()
        }
        Some(FinishReason::Stop) => "(no response: model returned an empty completion)".to_string(),
        Some(FinishReason::Other(reason)) => {
            format!("(no response: finish reason = {})", reason)
        }
        None => "(no response: stream completed without content)".to_string(),
    }
}

fn should_retry_empty_completion(
    raw_response: &str,
    actions_performed: bool,
    terminal_emitted: bool,
    final_emitted: bool,
    finish_reason: Option<&FinishReason>,
) -> bool {
    if !raw_response.trim().is_empty() || actions_performed || terminal_emitted || final_emitted {
        return false;
    }

    matches!(finish_reason, Some(FinishReason::Stop) | None)
}

const MAX_EMPTY_COMPLETION_RETRIES: usize = 2;
const EMPTY_COMPLETION_RETRY_DELAY_MS: u64 = 250;

pub struct AuwgentEngine {
    ir: AgentIR,
    session: Arc<Mutex<SessionState>>,
    tools: Arc<Mutex<HashMap<String, ToolImplementation>>>,
    orchestrator: Arc<Mutex<Orchestrator>>,
    drivers: Arc<Mutex<HashMap<String, Arc<dyn ModelDriver>>>>,
    context: Arc<Mutex<Option<Value>>>,
    pending_intents: Arc<Mutex<Vec<(String, Value)>>>,
    streaming_jsonl: Arc<Mutex<JsonlEventBuffer>>,
    pending_tool_results: Arc<Mutex<Vec<(String, Value, Value)>>>,
    current_raw_response: Arc<Mutex<String>>,
    last_turn_response_value: Arc<Mutex<Value>>,
    intent_handler: Arc<Mutex<Option<AsyncIntentCallback>>>,
    partial_intent_handler: Arc<Mutex<Option<Arc<dyn Fn(String, Value, String) + Send + Sync>>>>,
    session_preload_handler: Arc<Mutex<Option<AsyncSessionPreloadCallback>>>,
    session_save_handler: Arc<Mutex<Option<SessionSaveCallback>>>,
    middleware_event_handler: Arc<Mutex<Option<AsyncMiddlewareEventCallback>>>,
    fast_forward_stack: Arc<Mutex<Option<Vec<String>>>>,
    terminal_response_emitted: Arc<Mutex<bool>>,
    final_response_emitted: Arc<Mutex<bool>>,
    user_input: Arc<Mutex<Option<serde_json::Value>>>,
    pub last_run_metadata: Arc<Mutex<RunMetadata>>,
}

impl AuwgentEngine {
    pub fn new(ir: AgentIR) -> Self {
        let mut orchestrator = Orchestrator::new();
        let streaming_partials = Arc::new(Mutex::new(PartialIntentState::default()));
        let streaming_jsonl = Arc::new(Mutex::new(JsonlEventBuffer::default()));
        let partial_intent_handler: Arc<
            Mutex<Option<Arc<dyn Fn(String, Value, String) + Send + Sync>>>,
        > = Arc::new(Mutex::new(None));

        orchestrator.register_intent("tool_call");
        orchestrator.register_intent("workflow_call");
        orchestrator.register_intent("response_schema");
        orchestrator.register_intent("response_text");
        orchestrator.register_intent("helper_call");
        if !ir.components.is_empty() {
            orchestrator.register_intent("component");
            orchestrator.register_intent("render_component");
            for component in &ir.components {
                orchestrator.register_component_shape(component, ir.types.as_ref());
            }
        }

        if let Some(custom) = &ir.custom_intents {
            for ci in custom {
                orchestrator.register_intent(&ci.name);
                orchestrator.register_custom_intent_shape(&ci.name, &ci.fields.0, ir.types.as_ref());
            }
        }

        for tool in &ir.tools {
            orchestrator.register_tool_shape(&tool.name, &tool.params.0, ir.types.as_ref());
        }

        for workflow in &ir.workflows {
            orchestrator.register_workflow_shape(
                &workflow.name,
                &workflow.params.0,
                ir.types.as_ref(),
            );
        }

        for helper in &ir.helpers {
            orchestrator.register_helper_shape(
                &helper.name,
                helper.input.as_ref().map(|value| &value.0),
                ir.types.as_ref(),
            );
        }

        if let Some(output) = &ir.output {
            orchestrator.register_output_shape(&output.0, ir.types.as_ref());
        }

        let pending_intents = Arc::new(Mutex::new(Vec::new()));
        let intents_for_handler = Arc::clone(&pending_intents);
        orchestrator.on_intent_ready(Arc::new(move |name, value| {
            if let Ok(mut pending) = intents_for_handler.lock() {
                let already_pending = pending.iter().any(|(pending_name, pending_value)| {
                    pending_name == &name && pending_value == &value
                });

                if !already_pending {
                    pending.push((name, value));
                }
            }
        }));

        let partial_state_for_handler = Arc::clone(&streaming_partials);
        let jsonl_for_handler = Arc::clone(&streaming_jsonl);
        let partial_handler_for_handler = Arc::clone(&partial_intent_handler);
        let agent_name_for_handler = ir.name.clone();

        orchestrator.on_intent_partial(Arc::new(move |name, value| {
            let value = if name == "response_text" {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    let segment = value.get("segment").and_then(Value::as_u64).unwrap_or(0);
                    let delta = partial_state_for_handler
                        .lock()
                        .map(|mut state| {
                            state.response_text_delta(&agent_name_for_handler, &name, segment, text)
                        })
                        .unwrap_or_else(|_| text.to_string());
                    let mut updated = value.clone();
                    if let Value::Object(ref mut map) = updated {
                        map.insert("delta".to_string(), Value::String(delta));
                    }
                    updated
                } else {
                    value
                }
            } else {
                value
            };

            if let Ok(mut buffer) = jsonl_for_handler.lock() {
                let seq = buffer.next_seq();
                buffer.push_event(StructuredOutputEvent::partial_intent(
                    seq,
                    agent_name_for_handler.clone(),
                    name.clone(),
                    value.clone(),
                ));
            }

            if let Some(handler) = partial_handler_for_handler
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                handler(name, value, agent_name_for_handler.clone());
            }
        }));

        Self {
            ir,
            session: Arc::new(Mutex::new(SessionState::new())),
            tools: Arc::new(Mutex::new(HashMap::new())),
            orchestrator: Arc::new(Mutex::new(orchestrator)),
            drivers: Arc::new(Mutex::new(HashMap::new())),
            context: Arc::new(Mutex::new(None)),
            pending_intents,
            streaming_jsonl,
            pending_tool_results: Arc::new(Mutex::new(Vec::new())),
            current_raw_response: Arc::new(Mutex::new(String::new())),
            last_turn_response_value: Arc::new(Mutex::new(Value::Null)),
            intent_handler: Arc::new(Mutex::new(None)),
            partial_intent_handler,
            session_preload_handler: Arc::new(Mutex::new(None)),
            session_save_handler: Arc::new(Mutex::new(None)),
            middleware_event_handler: Arc::new(Mutex::new(None)),
            fast_forward_stack: Arc::new(Mutex::new(None)),
            terminal_response_emitted: Arc::new(Mutex::new(false)),
            final_response_emitted: Arc::new(Mutex::new(false)),
            user_input: Arc::new(Mutex::new(None)),
            last_run_metadata: Arc::new(Mutex::new(RunMetadata::default())),
        }
    }

    fn next_structured_seq(&self) -> u64 {
        self.streaming_jsonl.lock().unwrap().next_seq()
    }

    fn push_structured_output_event(&self, event: StructuredOutputEvent) {
        self.streaming_jsonl.lock().unwrap().push_event(event);
    }

    fn emit_structured_intent(&self, name: String, payload: Value) {
        let seq = self.next_structured_seq();
        let event = StructuredOutputEvent::intent(seq, self.ir.name.clone(), name, payload);
        self.push_structured_output_event(event);
    }

    fn emit_structured_stream_start(&self) {
        let seq = self.next_structured_seq();
        let event = StructuredOutputEvent::lifecycle_start(seq, self.ir.name.clone());
        self.push_structured_output_event(event);
    }

    fn emit_structured_stream_finish(&self) {
        let seq = self.next_structured_seq();
        let event = StructuredOutputEvent::lifecycle_finish(seq, self.ir.name.clone());
        self.push_structured_output_event(event);
    }

    fn emit_structured_stream_error(&self, message: String) {
        let seq = self.next_structured_seq();
        let event = StructuredOutputEvent::lifecycle_error(seq, self.ir.name.clone(), message);
        self.push_structured_output_event(event);
    }

    pub fn drain_structured_output_jsonl(&self) -> Vec<String> {
        self.streaming_jsonl.lock().unwrap().drain_lines()
    }

    pub fn drain_structured_output_jsonl_text(&self) -> String {
        let lines = self.drain_structured_output_jsonl();
        if lines.is_empty() {
            String::new()
        } else {
            let mut out = lines.join("\n");
            out.push('\n');
            out
        }
    }

    pub fn on_sub_engine_start(&self, handler: AsyncSessionPreloadCallback) {
        *self.session_preload_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_sub_engine_complete(&self, handler: SessionSaveCallback) {
        *self.session_save_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_middleware_event(&self, handler: AsyncMiddlewareEventCallback) {
        *self.middleware_event_handler.lock().unwrap() = Some(handler);
    }

    pub fn register_driver(&self, provider_type: &str, driver: Arc<dyn ModelDriver>) {
        self.drivers
            .lock()
            .unwrap()
            .insert(provider_type.to_string(), driver);
    }

    pub fn set_context(&self, context: Value) {
        *self.context.lock().unwrap() = Some(context);
    }

    pub fn register_tool(&self, name: &str, implementation: ToolImplementation) {
        self.tools
            .lock()
            .unwrap()
            .insert(name.to_string(), implementation);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        *self.intent_handler.lock().unwrap() = Some(handler);
    }

    pub fn on_intent_sync(&self, handler: IntentCallback) {
        let handler = handler.clone();
        *self.intent_handler.lock().unwrap() = Some(Arc::new(move |name, value, agent| {
            let result = handler(name, value, agent);
            Box::pin(async move { result })
        }));
    }

    pub fn on_intent_partial(&self, handler: Arc<dyn Fn(String, Value, String) + Send + Sync>) {
        *self.partial_intent_handler.lock().unwrap() = Some(handler);
    }

    pub fn clear_intent_handlers(&self) {
        *self.intent_handler.lock().unwrap() = None;
        *self.partial_intent_handler.lock().unwrap() = None;
    }

    pub fn clear_sub_engine_handlers(&self) {
        *self.session_preload_handler.lock().unwrap() = None;
        *self.session_save_handler.lock().unwrap() = None;
    }

    pub fn clear_middleware_handler(&self) {
        *self.middleware_event_handler.lock().unwrap() = None;
    }

    pub fn export_session(&self) -> AuwgentResult<String> {
        self.session
            .lock()
            .unwrap()
            .export()
            .map_err(AuwgentError::Serialization)
    }

    pub fn import_session(&self, json: &str) -> AuwgentResult<()> {
        *self.session.lock().unwrap() =
            SessionState::import(json).map_err(AuwgentError::Serialization)?;
        Ok(())
    }

    pub fn session(&self) -> std::sync::MutexGuard<'_, SessionState> {
        self.session.lock().unwrap()
    }

    pub fn clear_session(&self) {
        self.session.lock().unwrap().clear();
    }

    pub async fn embed(&self, text: &str) -> AuwgentResult<Vec<f32>> {
        let (driver, model_name, config) = self.get_embedding_config()?;
        driver
            .embed(&model_name, text, config)
            .await
            .map_err(AuwgentError::Driver)
    }

    pub async fn embed_batch(&self, texts: &[String]) -> AuwgentResult<Vec<Vec<f32>>> {
        let (driver, model_name, config) = self.get_embedding_config()?;
        driver
            .embed_batch(&model_name, texts, config)
            .await
            .map_err(AuwgentError::Driver)
    }

    fn get_embedding_config(&self) -> AuwgentResult<(Arc<dyn ModelDriver>, String, Option<Value>)> {
        let model_entry = self
            .ir
            .model_config
            .first()
            .ok_or(AuwgentError::MissingConfig("No model config".into()))?;
        let default_config = model_entry
            .default_config
            .as_ref()
            .ok_or(AuwgentError::MissingConfig("No default config".into()))?;

        let embedding_provider = default_config
            .embedding
            .as_ref()
            .ok_or(AuwgentError::MissingConfig(
                "No embedding model configured".into(),
            ))?;

        let evaluator = Evaluator::new(&self.ir);
        let mut scope = HashMap::new();
        if let Some(ctx) = self.context.lock().unwrap().as_ref() {
            scope.insert("context".to_string(), ctx.clone());
        }

        let provider_info = evaluator.evaluate_provider(embedding_provider, &mut scope)?;

        let provider_type = provider_info["provider"].as_str().unwrap_or("gemini");
        let provider_id = if provider_type == "custom" {
            provider_info["id"].as_str().unwrap_or("custom")
        } else {
            provider_type
        };

        let model_name = provider_info["modelName"].as_str().ok_or_else(|| {
            AuwgentError::MissingConfig("modelName is required for embedding".into())
        })?;

        let config_params = provider_info.get("config").cloned();

        let driver = self
            .drivers
            .lock()
            .unwrap()
            .get(provider_id)
            .ok_or(AuwgentError::NoDriver)?
            .clone();

        Ok((driver, model_name.to_string(), config_params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_empty_stop_completion_without_actions() {
        assert!(should_retry_empty_completion(
            "",
            false,
            false,
            false,
            Some(&FinishReason::Stop),
        ));
    }

    #[test]
    fn does_not_retry_when_raw_text_exists() {
        assert!(!should_retry_empty_completion(
            "hello",
            false,
            false,
            false,
            Some(&FinishReason::Stop),
        ));
    }

    #[test]
    fn does_not_retry_when_actions_or_terminal_emitted() {
        assert!(!should_retry_empty_completion(
            "",
            true,
            false,
            false,
            Some(&FinishReason::Stop),
        ));
        assert!(!should_retry_empty_completion(
            "",
            false,
            true,
            false,
            Some(&FinishReason::Stop),
        ));
        assert!(!should_retry_empty_completion(
            "",
            false,
            false,
            true,
            Some(&FinishReason::Stop),
        ));
    }
}

