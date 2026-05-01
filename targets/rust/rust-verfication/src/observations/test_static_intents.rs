use crate::main_agent::{
    AuwgentAgent, AuwgentIntentHandler, Intents, ResponseText, ToolCalls, ToolErrors, ToolResults,
    auwgent,
};
use crate::observations::agent_config::get_agent_config;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
struct IntentEvents {
    any: Vec<String>,
    response_texts: Vec<String>,
    tool_calls: Vec<String>,
    tool_errors: Vec<String>,
    tool_results: Vec<String>,
}

#[derive(Clone)]
struct IntentRecorder {
    events: Arc<Mutex<IntentEvents>>,
}

impl IntentRecorder {
    fn new() -> (Self, Arc<Mutex<IntentEvents>>) {
        let events = Arc::new(Mutex::new(IntentEvents::default()));
        (
            Self {
                events: Arc::clone(&events),
            },
            events,
        )
    }
}

impl AuwgentIntentHandler for IntentRecorder {
    fn response_text(&self, value: &ResponseText, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .response_texts
            .push(value.text.clone());
    }

    fn tool_call(&self, value: &ToolCalls, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .tool_calls
            .push(format!("{:?}", value.kind));
    }

    fn tool_result(&self, value: &ToolResults, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .tool_results
            .push(format!("{:?}", value.kind));
    }

    fn tool_error(&self, value: &ToolErrors, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .tool_errors
            .push(format!("{:?}", value.kind));
    }

    fn any(&self, intent: &Intents, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .any
            .push(intent.name().to_string());
    }
}

fn build_agent() -> (AuwgentAgent, Arc<Mutex<IntentEvents>>) {
    let config = get_agent_config(vec![]);
    let agent = auwgent(config).expect("agent should load");
    let (recorder, events) = IntentRecorder::new();
    agent.on_intent(recorder);
    (agent, events)
}

async fn process_static_chunk(agent: &AuwgentAgent, chunk: &str) -> serde_json::Value {
    agent.raw().write_chunk(chunk.to_string());
    agent.raw().end_stream().expect("stream should finalize");
    agent
        .raw()
        .process_intents()
        .await
        .expect("intent processing should succeed")
}

fn assert_process_flags(value: &serde_json::Value, terminal: bool, actions: bool, hard_stop: bool) {
    assert_eq!(value["terminal"], terminal, "terminal flag mismatch");
    assert_eq!(value["actions"], actions, "actions flag mismatch");
    assert_eq!(value["hard_stop"], hard_stop, "hard_stop flag mismatch");
}

pub async fn test_static_intents() {
    response_text_is_terminal_without_actions().await;
    no_arg_tool_call_executes_as_action().await;
    arg_tool_call_executes_as_action().await;
    unknown_tool_call_emits_typed_tool_error().await;
}

async fn response_text_is_terminal_without_actions() {
    let (agent, events) = build_agent();

    let result = process_static_chunk(&agent, "[response_text] hello [/response_text]").await;

    assert_process_flags(&result, true, false, false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.any, vec!["response_text"]);
    assert_eq!(events.response_texts, vec!["hello"]);
    assert!(events.tool_calls.is_empty());
    assert!(events.tool_errors.is_empty());
    assert!(events.tool_results.is_empty());
}

async fn no_arg_tool_call_executes_as_action() {
    let (agent, events) = build_agent();

    let result = process_static_chunk(&agent, "[tool_call:get_location] [/tool_call]").await;

    assert_process_flags(&result, false, true, false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.any, vec!["tool_call", "tool_result"]);
    assert_eq!(events.tool_calls.len(), 1);
    assert!(events.tool_errors.is_empty());
    assert_eq!(events.tool_results.len(), 1);
    assert!(events.tool_calls[0].contains("GetLocation"));
    assert!(events.tool_results[0].contains("GetLocation"));
    assert!(events.tool_results[0].contains("Tarkwa"));
}

async fn arg_tool_call_executes_as_action() {
    let (agent, events) = build_agent();

    let result = process_static_chunk(&agent, r#"[tool_call:get_marks]id:"100"[/tool_call]"#).await;

    assert_process_flags(&result, false, true, false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.any, vec!["tool_call", "tool_result"]);
    assert_eq!(events.tool_calls.len(), 1);
    assert!(events.tool_errors.is_empty());
    assert_eq!(events.tool_results.len(), 1);
    assert!(events.tool_calls[0].contains("GetMarks"));
    assert!(events.tool_calls[0].contains("100"));
    assert!(events.tool_results[0].contains("GetMarks"));
    assert!(events.tool_results[0].contains("A,B,C,D"));
}

async fn unknown_tool_call_emits_typed_tool_error() {
    let (agent, events) = build_agent();

    let result = process_static_chunk(&agent, "[tool_call:get_] [/tool_call]").await;

    assert_process_flags(&result, false, true, false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.any, vec!["tool_error"]);
    assert!(events.tool_calls.is_empty());
    assert_eq!(events.tool_errors.len(), 1);
    assert!(events.tool_errors[0].contains("get_"));
    assert!(events.tool_errors[0].contains("Tool not found"));
    assert!(events.tool_results.is_empty());
}
