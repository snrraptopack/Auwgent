use crate::main_agent::{
    AuwgentAgent, AuwgentIntentHandler, Intents, ResponseText, ToolCalls, ToolErrors, ToolResults,
    auwgent,
};
use crate::observations::agent_config::get_agent_config;
use serde_json::{Value, json};
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
    let agent = auwgent(get_agent_config(vec![])).expect("agent should load");
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

pub async fn test_bridge_surface() {
    prompt_and_tool_introspection_are_available().await;
    session_import_export_and_clear_round_trip().await;
    raw_tool_registration_replaces_existing_tool_implementation().await;
    raw_tool_registration_can_add_untyped_runtime_tool().await;
}

async fn prompt_and_tool_introspection_are_available() {
    let (agent, _events) = build_agent();

    let prompt = agent
        .generate_prompt(None)
        .expect("prompt should generate through SDK bridge");
    assert!(prompt.contains("You are a helpful assistant"));
    assert!(prompt.contains("[tool_call"));
    assert!(prompt.contains("get_location"));

    let tool_names = agent.get_tool_names();
    assert_eq!(tool_names, vec!["get_location", "get_marks"]);

    let schemas = agent
        .get_tool_schemas()
        .expect("tool schemas should be available");
    assert!(schemas.as_array().is_some_and(|items| items.len() == 2));
    assert!(schemas.to_string().contains("get_location"));
    assert!(schemas.to_string().contains("get_marks"));
}

async fn session_import_export_and_clear_round_trip() {
    let (source, _events) = build_agent();
    source
        .raw()
        .begin_run(Some(json!("first input")), Some(vec!["SimpleTool".to_string()]))
        .await
        .expect("source run should begin");
    source
        .raw()
        .apply_llm_end(json!("stored response"))
        .await
        .expect("llm_end should store response");
    let stored = source.export_session().expect("source session should export");
    assert_eq!(stored.stack, vec!["SimpleTool"]);
    assert_eq!(stored.turns.len(), 1);
    assert_eq!(stored.turns[0].input, "first input");
    assert_eq!(stored.turns[0].model_response, "stored response");

    let (target, _events) = build_agent();
    target
        .import_session(&stored)
        .expect("target should import session");
    let imported = target.export_session().expect("target should export session");
    assert_eq!(imported.stack, stored.stack);
    assert_eq!(imported.turns.len(), stored.turns.len());
    assert_eq!(imported.turns[0].input, stored.turns[0].input);
    assert_eq!(
        imported.turns[0].model_response,
        stored.turns[0].model_response
    );
    let imported_prompt = imported.system_prompt.clone();

    target.clear_session();
    let cleared = target
        .export_session()
        .expect("cleared session should export");
    assert!(cleared.stack.is_empty());
    assert!(cleared.turns.is_empty());
    assert_eq!(cleared.system_prompt, imported_prompt);
}

async fn raw_tool_registration_replaces_existing_tool_implementation() {
    let (agent, events) = build_agent();
    agent.raw().register_tool_fn("get_location", |_args| {
        Box::pin(async { Ok(Value::String("Accra".to_string())) })
    });

    let result = process_static_chunk(&agent, "[tool_call:get_location] [/tool_call]").await;

    assert_eq!(result["terminal"], false);
    assert_eq!(result["actions"], true);
    assert_eq!(result["hard_stop"], false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.any, vec!["tool_call", "tool_result"]);
    assert_eq!(events.tool_calls.len(), 1);
    assert_eq!(events.tool_results.len(), 1);
    assert!(events.tool_results[0].contains("GetLocation"));
    assert!(events.tool_results[0].contains("Accra"));
    assert!(events.tool_errors.is_empty());
}

async fn raw_tool_registration_can_add_untyped_runtime_tool() {
    let agent = auwgent(get_agent_config(vec![])).expect("agent should load");
    agent.raw().register_tool_fn("dynamic_weather", |args| {
        Box::pin(async move {
            Ok(json!({
                "tool": "dynamic_weather",
                "args": args,
                "forecast": "sunny"
            }))
        })
    });

    let raw_events = Arc::new(Mutex::new(Vec::<(String, Value, String)>::new()));
    let raw_events_for_handler = Arc::clone(&raw_events);
    agent.raw().on_intent(Arc::new(move |name, value, agent_name| {
        let raw_events = Arc::clone(&raw_events_for_handler);
        Box::pin(async move {
            raw_events
                .lock()
                .expect("raw events mutex poisoned")
                .push((name, value, agent_name));
            None
        })
    }));

    let result = process_static_chunk(
        &agent,
        "[tool_call:dynamic_weather]city:Tarkwa[/tool_call]",
    )
    .await;

    assert_eq!(result["terminal"], false);
    assert_eq!(result["actions"], true);
    assert_eq!(result["hard_stop"], false);

    let raw_events = raw_events.lock().expect("raw events mutex poisoned").clone();
    assert_eq!(raw_events.len(), 2);
    assert_eq!(raw_events[0].0, "tool_call");
    assert_eq!(raw_events[0].1["type"], "dynamic_weather");
    assert_eq!(raw_events[0].1["args"]["city"], "Tarkwa");
    assert_eq!(raw_events[0].2, "SimpleTool");
    assert_eq!(raw_events[1].0, "tool_result");
    assert_eq!(raw_events[1].1["name"], "dynamic_weather");
    assert_eq!(raw_events[1].1["result"]["forecast"], "sunny");
}
