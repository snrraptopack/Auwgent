use crate::main_agent::{
    AuwgentMiddleware, AuwgentMiddlewareRegistry, Context, Intents, Session, auwgent,
};
use crate::observations::agent_config::get_agent_config;
use async_trait::async_trait;
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct ContextSnapshot {
    phase: String,
    active_agent: String,
    root_agent: String,
    stack: Vec<String>,
    raw_block: Option<String>,
    system_prompt: Option<String>,
    data: Map<String, Value>,
}

#[derive(Clone)]
struct LifecycleRecorder {
    events: Arc<Mutex<Vec<String>>>,
    snapshots: Arc<Mutex<Vec<ContextSnapshot>>>,
}

impl LifecycleRecorder {
    fn new() -> (Self, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<ContextSnapshot>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
                snapshots: Arc::clone(&snapshots),
            },
            events,
            snapshots,
        )
    }

    fn push(&self, event: impl Into<String>) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .push(event.into());
    }

    fn snapshot(&self, phase: impl Into<String>, ctx: &Context) {
        self.snapshots
            .lock()
            .expect("snapshots mutex poisoned")
            .push(ContextSnapshot {
                phase: phase.into(),
                active_agent: ctx.active_agent.clone(),
                root_agent: ctx.root_agent.clone(),
                stack: ctx.stack.clone(),
                raw_block: ctx.raw_block.clone(),
                system_prompt: ctx.system_prompt.clone(),
                data: ctx.data.clone(),
            });
    }
}

#[async_trait]
impl AuwgentMiddleware for LifecycleRecorder {
    async fn on_run_start(&self, session: Session, ctx: &mut Context) -> Session {
        self.push("run_start");
        self.snapshot("run_start", ctx);
        ctx.data.insert("run_started".to_string(), json!(true));
        ctx.data.insert("turn_counter".to_string(), json!(1));
        ctx.set_context(json!({
            "user_name": "MiddlewareUser",
            "age": 18,
            "id": "middleware-id"
        }));
        session
    }

    async fn on_llm_start(&self, prompt: String, ctx: &mut Context) -> String {
        self.push(format!("llm_start:{prompt}"));
        self.snapshot("llm_start", ctx);
        assert_eq!(ctx.data.get("run_started"), Some(&json!(true)));
        assert_eq!(ctx.data.get("turn_counter"), Some(&json!(1)));
        let system_prompt = ctx
            .system_prompt
            .as_deref()
            .expect("llm_start context should include system prompt");
        assert!(
            system_prompt.contains("not that old"),
            "run_start set_context should regenerate the system prompt"
        );
        assert!(
            system_prompt.contains("user_name: MiddlewareUser"),
            "unused context should still be available as additional prompt context"
        );
        ctx.data
            .insert("llm_start_prompt".to_string(), json!(prompt.clone()));
        format!("{prompt} + middleware")
    }

    async fn on_intent(
        &self,
        intent: &Intents,
        ctx: &mut Context,
    ) -> Option<crate::main_agent::IntentControl> {
        self.push(format!("intent:{}", intent.name()));
        self.snapshot(format!("intent:{}", intent.name()), ctx);
        assert_eq!(
            ctx.data.get("llm_start_prompt"),
            Some(&json!("deterministic prompt"))
        );
        ctx.data
            .insert(format!("intent_{}", intent.name()), json!(true));
        None
    }

    async fn on_llm_end(&self, response: &Value, ctx: &mut Context) {
        self.push(format!(
            "llm_end:{}",
            response
                .as_str()
                .or_else(|| response.get("text").and_then(Value::as_str))
                .unwrap_or("<non-string>")
        ));
        self.snapshot("llm_end", ctx);
        assert_eq!(ctx.data.get("intent_response_text"), Some(&json!(true)));
        ctx.data.insert("llm_ended".to_string(), json!(true));
    }

    async fn on_run_complete(&self, _session: &Session, ctx: &mut Context) {
        self.push("run_complete");
        self.snapshot("run_complete", ctx);
        assert_eq!(ctx.data.get("llm_ended"), Some(&json!(true)));
        ctx.data.insert("completed".to_string(), json!(true));
    }

    async fn on_error(&self, error: &Value, _session: Option<&Session>, ctx: &mut Context) -> bool {
        self.push(format!(
            "error:{}",
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("<missing>")
        ));
        self.snapshot("error", ctx);
        assert_eq!(ctx.data.get("completed"), Some(&json!(true)));
        ctx.data
            .insert("error_was_swallowed".to_string(), json!(true));
        true
    }
}

pub async fn test_middleware_lifecycle_driver() {
    test_context_sharing_and_prompt_mutation().await;
    test_tool_action_middleware_context().await;
}

async fn test_context_sharing_and_prompt_mutation() {
    let (recorder, events, snapshots) = LifecycleRecorder::new();
    let middleware: Vec<AuwgentMiddlewareRegistry> = vec![recorder.into()];
    let config = get_agent_config(middleware);
    let agent = auwgent(config).expect("agent should load");

    let session = agent
        .raw()
        .begin_run(
            Some(Value::String("hello".to_string())),
            Some(vec![
                "SimpleTool".to_string(),
                "ObservationFrame".to_string(),
            ]),
        )
        .await
        .expect("begin_run should succeed");
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.turns[0].input, "hello");
    let system_prompt = session
        .system_prompt
        .as_deref()
        .expect("begin_run should store system prompt");
    assert!(system_prompt.contains("not that old"));
    assert!(!system_prompt.contains("The person is old 25.4"));

    let prompt = agent
        .raw()
        .apply_llm_start("deterministic prompt".to_string())
        .await
        .expect("llm_start should succeed");
    assert_eq!(prompt, "deterministic prompt + middleware");

    agent
        .raw()
        .write_chunk("[response_text] deterministic [/response_text]".to_string());
    agent.raw().end_stream().expect("stream should finalize");
    let result = agent
        .raw()
        .process_intents()
        .await
        .expect("intent processing should succeed");
    assert_eq!(result["terminal"], true);
    assert_eq!(result["actions"], false);

    agent
        .raw()
        .apply_llm_end(Value::String("deterministic".to_string()))
        .await
        .expect("llm_end should succeed");

    let completed = agent
        .raw()
        .complete_run()
        .await
        .expect("run_complete should succeed");
    assert_eq!(completed.turns[0].input, "deterministic prompt + middleware");
    assert_eq!(completed.turns[0].model_response, "deterministic");

    let swallowed = agent
        .raw()
        .apply_error(
            serde_json::json!({
                "message": "manual failure"
            }),
            true,
        )
        .await
        .expect("error middleware should succeed");
    assert!(swallowed);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(
        events,
        vec![
            "run_start",
            "llm_start:deterministic prompt",
            "intent:response_text",
            "llm_end:deterministic",
            "run_complete",
            "error:manual failure",
        ]
    );

    let snapshots = snapshots.lock().expect("snapshots mutex poisoned").clone();
    assert_eq!(
        snapshots
            .iter()
            .map(|snapshot| snapshot.phase.as_str())
            .collect::<Vec<_>>(),
        vec![
            "run_start",
            "llm_start",
            "intent:response_text",
            "llm_end",
            "run_complete",
            "error",
        ]
    );

    for snapshot in &snapshots {
        assert_eq!(snapshot.active_agent, "SimpleTool");
        assert_eq!(snapshot.root_agent, "SimpleTool");
        assert_eq!(
            snapshot.stack,
            vec!["SimpleTool".to_string(), "ObservationFrame".to_string()]
        );
    }

    let run_start = snapshots
        .iter()
        .find(|snapshot| snapshot.phase == "run_start")
        .expect("run_start snapshot exists");
    assert!(
        run_start
            .system_prompt
            .as_deref()
            .expect("run_start should see original prompt")
            .contains("The person is old 25.4")
    );

    let response_text = snapshots
        .iter()
        .find(|snapshot| snapshot.phase == "intent:response_text")
        .expect("response_text snapshot exists");
    assert_eq!(
        response_text.raw_block.as_deref(),
        Some("[response_text] deterministic [/response_text]")
    );
    assert!(
        response_text.system_prompt.is_none(),
        "intent middleware receives raw block context, not prompt context"
    );

    let final_context = agent.shared_context();
    assert_eq!(final_context.get("run_started"), Some(&json!(true)));
    assert_eq!(
        final_context.get("llm_start_prompt"),
        Some(&json!("deterministic prompt"))
    );
    assert_eq!(final_context.get("intent_response_text"), Some(&json!(true)));
    assert_eq!(final_context.get("llm_ended"), Some(&json!(true)));
    assert_eq!(final_context.get("completed"), Some(&json!(true)));
    assert_eq!(
        final_context.get("error_was_swallowed"),
        Some(&json!(true))
    );
}

#[derive(Clone)]
struct ToolActionRecorder {
    events: Arc<Mutex<Vec<String>>>,
    snapshots: Arc<Mutex<Vec<ContextSnapshot>>>,
}

impl ToolActionRecorder {
    fn new() -> (Self, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<ContextSnapshot>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
                snapshots: Arc::clone(&snapshots),
            },
            events,
            snapshots,
        )
    }

    fn record(&self, intent: &Intents, ctx: &mut Context) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .push(intent.name().to_string());
        self.snapshots
            .lock()
            .expect("snapshots mutex poisoned")
            .push(ContextSnapshot {
                phase: format!("intent:{}", intent.name()),
                active_agent: ctx.active_agent.clone(),
                root_agent: ctx.root_agent.clone(),
                stack: ctx.stack.clone(),
                raw_block: ctx.raw_block.clone(),
                system_prompt: ctx.system_prompt.clone(),
                data: ctx.data.clone(),
            });
    }
}

#[async_trait]
impl AuwgentMiddleware for ToolActionRecorder {
    async fn on_run_start(&self, session: Session, ctx: &mut Context) -> Session {
        ctx.data.insert("run_started".to_string(), json!(true));
        session
    }

    async fn on_intent(
        &self,
        intent: &Intents,
        ctx: &mut Context,
    ) -> Option<crate::main_agent::IntentControl> {
        assert_eq!(ctx.data.get("run_started"), Some(&json!(true)));
        self.record(intent, ctx);
        ctx.data.insert(format!("saw_{}", intent.name()), json!(true));
        None
    }
}

async fn test_tool_action_middleware_context() {
    let (recorder, events, snapshots) = ToolActionRecorder::new();
    let middleware: Vec<AuwgentMiddlewareRegistry> = vec![recorder.into()];
    let config = get_agent_config(middleware);
    let agent = auwgent(config).expect("agent should load");

    agent
        .raw()
        .begin_run(Some(Value::String("where am I?".to_string())), None)
        .await
        .expect("begin_run should succeed");
    agent
        .raw()
        .write_chunk("[tool_call:get_location] [/tool_call]".to_string());
    agent.raw().end_stream().expect("stream should finalize");
    let result = agent
        .raw()
        .process_intents()
        .await
        .expect("tool intent processing should succeed");
    assert_eq!(result["terminal"], false);
    assert_eq!(result["actions"], true);
    assert_eq!(result["hard_stop"], false);

    let events = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(events, vec!["tool_call", "tool_result"]);

    let snapshots = snapshots.lock().expect("snapshots mutex poisoned").clone();
    let tool_call = snapshots
        .iter()
        .find(|snapshot| snapshot.phase == "intent:tool_call")
        .expect("tool_call snapshot exists");
    assert_eq!(
        tool_call.raw_block.as_deref(),
        Some("[tool_call:get_location] [/tool_call]")
    );
    assert_eq!(tool_call.data.get("run_started"), Some(&json!(true)));

    let tool_result = snapshots
        .iter()
        .find(|snapshot| snapshot.phase == "intent:tool_result")
        .expect("tool_result snapshot exists");
    assert_eq!(tool_result.raw_block, None);
    assert_eq!(tool_result.data.get("saw_tool_call"), Some(&json!(true)));

    let final_context = agent.shared_context();
    assert_eq!(final_context.get("saw_tool_call"), Some(&json!(true)));
    assert_eq!(final_context.get("saw_tool_result"), Some(&json!(true)));
}
