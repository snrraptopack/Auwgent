use crate::main_agent::{
    AuwgentIntentHandler, AuwgentMiddleware, AuwgentMiddlewareRegistry, Context, Intents,
    ResponseText, Session, auwgent,
};
use crate::observations::agent_config::get_agent_config;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
struct StackEvents {
    run_start_context_stack: Vec<String>,
    run_start_session_stack: Vec<String>,
    llm_start_context_stack: Vec<String>,
    llm_start_root_agent: String,
    shared_marker_at_llm_start: Option<Value>,
}

#[derive(Clone, Debug, Default)]
struct IntentEvents {
    any: Vec<String>,
    response_texts: Vec<String>,
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
    fn any(&self, intent: &Intents, _agent: &str) {
        self.events
            .lock()
            .expect("intent events mutex poisoned")
            .any
            .push(intent.name().to_string());
    }

    fn response_text(&self, value: &ResponseText, agent: &str) {
        self.events
            .lock()
            .expect("intent events mutex poisoned")
            .response_texts
            .push(format!("{agent}:{}", value.text));
    }
}

#[derive(Clone)]
struct StoredSessionMiddleware {
    events: Arc<Mutex<StackEvents>>,
    stored_session: Session,
}

impl StoredSessionMiddleware {
    fn new(stored_session: Session) -> (Self, Arc<Mutex<StackEvents>>) {
        let events = Arc::new(Mutex::new(StackEvents::default()));
        (
            Self {
                events: Arc::clone(&events),
                stored_session,
            },
            events,
        )
    }
}

#[async_trait]
impl AuwgentMiddleware for StoredSessionMiddleware {
    async fn on_run_start(&self, session: Session, ctx: &mut Context) -> Session {
        {
            let mut events = self.events.lock().expect("events mutex poisoned");
            events.run_start_context_stack = ctx.stack.clone();
            events.run_start_session_stack = session.stack.clone();
        }

        ctx.data.insert("restored_from_store".to_string(), json!(true));
        self.stored_session.clone()
    }

    async fn on_llm_start(&self, prompt: String, ctx: &mut Context) -> String {
        let mut events = self.events.lock().expect("events mutex poisoned");
        events.llm_start_context_stack = ctx.stack.clone();
        events.llm_start_root_agent = ctx.root_agent.clone();
        events.shared_marker_at_llm_start = ctx.data.get("restored_from_store").cloned();
        prompt
    }
}

pub async fn test_stack_resumption() {
    run_start_can_restore_stored_helper_stack_and_teleport_into_helper().await;
}

async fn create_stored_helper_stack_session() -> Session {
    let agent = auwgent(get_agent_config(vec![])).expect("stored agent should load");
    agent
        .raw()
        .begin_run(
            Some(Value::String("check this fact".to_string())),
            Some(vec!["SimpleTool".to_string(), "Fact".to_string()]),
        )
        .await
        .expect("stored helper stack session should be created through SDK");

    let stored_session = agent
        .export_session()
        .expect("stored helper session should export");
    assert_eq!(stored_session.stack, vec!["SimpleTool", "Fact"]);
    assert_eq!(stored_session.turns.len(), 1);
    assert_eq!(stored_session.turns[0].input, "check this fact");
    stored_session
}

async fn run_start_can_restore_stored_helper_stack_and_teleport_into_helper() {
    let stored_session = create_stored_helper_stack_session().await;

    let (middleware, events) = StoredSessionMiddleware::new(stored_session.clone());
    let middleware: Vec<AuwgentMiddlewareRegistry> = vec![middleware.into()];
    let config = get_agent_config(middleware);
    let agent = auwgent(config).expect("agent should load");
    agent.raw().set_deterministic_driver(
        "groq".to_string(),
        vec!["[response_text] helper resumed [/response_text]".to_string()],
    );
    let (recorder, intent_events) = IntentRecorder::new();
    agent.on_intent(recorder);

    let session = agent
        .raw()
        .run(None, Some(vec!["SimpleTool".to_string()]))
        .await
        .expect("run should restore and teleport into helper");

    assert_eq!(session.stack, vec!["SimpleTool"]);
    assert_eq!(session.turns.len(), stored_session.turns.len());
    assert_eq!(session.turns[0].input, stored_session.turns[0].input);
    assert_eq!(session.initial_input, stored_session.initial_input);

    let intent_events = intent_events
        .lock()
        .expect("intent events mutex poisoned")
        .clone();
    assert_eq!(intent_events.any, vec!["helper_call", "response_text"]);
    assert_eq!(intent_events.response_texts, vec!["Fact:helper resumed"]);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events.run_start_context_stack, vec!["SimpleTool"]);
    assert_eq!(events.run_start_session_stack, vec!["SimpleTool"]);
    assert!(
        events.llm_start_context_stack.is_empty(),
        "parent llm_start is skipped because restored stack teleports directly into Fact"
    );
    assert_eq!(events.llm_start_root_agent, "");
    assert_eq!(events.shared_marker_at_llm_start, None);
    assert_eq!(
        agent.shared_context().get("restored_from_store"),
        Some(&json!(true))
    );
}
