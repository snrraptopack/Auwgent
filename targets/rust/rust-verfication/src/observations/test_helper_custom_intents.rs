use crate::main_agent::{
    AuwgentIntentHandler, AuwgentMiddleware, AuwgentMiddlewareRegistry, Context, FactOutput,
    HelperCalls, HelperResults, IntentControl, Intents, LoudIntent, Session, auwgent,
};
use crate::observations::agent_config::get_agent_config;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
struct IntentEvents {
    any: Vec<String>,
    loud: Vec<String>,
    helper_calls: Vec<String>,
    helper_results: Vec<String>,
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
            .expect("events mutex poisoned")
            .any
            .push(intent.name().to_string());
    }

    fn loud(&self, value: &LoudIntent, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .loud
            .push(format!("{}:{}", value.actions, value.reason));
    }

    fn helper_call(&self, value: &HelperCalls, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .helper_calls
            .push(format!("{:?}", value.kind));
    }

    fn helper_result(&self, value: &HelperResults, _agent: &str) {
        self.events
            .lock()
            .expect("events mutex poisoned")
            .helper_results
            .push(format!("{:?}", value.kind));
    }
}

#[derive(Clone, Debug, Default)]
struct MiddlewareEvents {
    names: Vec<String>,
    raw_blocks: Vec<Option<String>>,
}

#[derive(Clone)]
struct HelperCustomMiddleware {
    events: Arc<Mutex<MiddlewareEvents>>,
}

impl HelperCustomMiddleware {
    fn new() -> (Self, Arc<Mutex<MiddlewareEvents>>) {
        let events = Arc::new(Mutex::new(MiddlewareEvents::default()));
        (
            Self {
                events: Arc::clone(&events),
            },
            events,
        )
    }
}

#[async_trait]
impl AuwgentMiddleware for HelperCustomMiddleware {
    async fn on_run_start(&self, session: Session, ctx: &mut Context) -> Session {
        ctx.data.insert("run_started".to_string(), json!(true));
        session
    }

    async fn on_intent(&self, intent: &Intents, ctx: &mut Context) -> Option<IntentControl> {
        let name = intent.name().to_string();
        {
            let mut events = self.events.lock().expect("events mutex poisoned");
            events.names.push(name.clone());
            events.raw_blocks.push(ctx.raw_block.clone());
        }

        assert_eq!(ctx.data.get("run_started"), Some(&json!(true)));

        match intent.name() {
            "helper_call" => {
                ctx.data.insert("helper_call_seen".to_string(), json!(true));
                Some(IntentControl::Override {
                    result: json!({
                        "is_fact": true,
                        "confidence": 0.99,
                        "reasons": "deterministic helper override"
                    }),
                })
            }
            "helper_result" => {
                assert_eq!(ctx.data.get("helper_call_seen"), Some(&json!(true)));
                ctx.data
                    .insert("helper_result_seen".to_string(), json!(true));
                None
            }
            "Loud" => {
                ctx.data.insert("custom_seen".to_string(), json!(true));
                None
            }
            _ => None,
        }
    }
}

fn build_agent() -> (
    crate::main_agent::AuwgentAgent,
    Arc<Mutex<IntentEvents>>,
    Arc<Mutex<MiddlewareEvents>>,
) {
    let (middleware, middleware_events) = HelperCustomMiddleware::new();
    let middleware: Vec<AuwgentMiddlewareRegistry> = vec![middleware.into()];
    let config = get_agent_config(middleware);
    let agent = auwgent(config).expect("agent should load");
    let (recorder, intent_events) = IntentRecorder::new();
    agent.on_intent(recorder);
    (agent, intent_events, middleware_events)
}

async fn process_static_chunk(
    agent: &crate::main_agent::AuwgentAgent,
    chunk: &str,
) -> serde_json::Value {
    agent.raw().write_chunk(chunk.to_string());
    agent.raw().end_stream().expect("stream should finalize");
    agent
        .raw()
        .process_intents()
        .await
        .expect("intent processing should succeed")
}

pub async fn test_helper_custom_intents() {
    custom_intent_is_deterministic_and_preserves_raw_block().await;
    helper_call_can_be_deterministically_overridden_without_model_driver().await;
}

async fn custom_intent_is_deterministic_and_preserves_raw_block() {
    let (agent, intent_events, middleware_events) = build_agent();

    agent
        .raw()
        .begin_run(Some(Value::String("say it loud".to_string())), None)
        .await
        .expect("begin_run should succeed");
    let result = process_static_chunk(
        &agent,
        "[custom:Loud]actions:take_action\nreason:nothing[/custom]",
    )
    .await;

    assert_eq!(result["terminal"], true);
    assert_eq!(result["actions"], false);
    assert_eq!(result["hard_stop"], false);

    let intent_events = intent_events.lock().expect("events mutex poisoned").clone();
    assert_eq!(intent_events.any, vec!["Loud"]);
    assert_eq!(intent_events.loud, vec!["take_action:nothing"]);
    assert!(intent_events.helper_calls.is_empty());
    assert!(intent_events.helper_results.is_empty());

    let middleware_events = middleware_events
        .lock()
        .expect("middleware events mutex poisoned")
        .clone();
    assert_eq!(middleware_events.names, vec!["Loud"]);
    assert_eq!(
        middleware_events.raw_blocks,
        vec![Some(
            "[custom:Loud]actions:take_action\nreason:nothing[/custom]".to_string()
        )]
    );
    assert_eq!(agent.shared_context().get("custom_seen"), Some(&json!(true)));
}

async fn helper_call_can_be_deterministically_overridden_without_model_driver() {
    let (agent, intent_events, middleware_events) = build_agent();

    agent
        .raw()
        .begin_run(Some(Value::String("check this fact".to_string())), None)
        .await
        .expect("begin_run should succeed");
    let result = process_static_chunk(
        &agent,
        "[helper_call:Fact]input:called helper[/helper]",
    )
    .await;

    assert_eq!(result["terminal"], false);
    assert_eq!(result["actions"], true);
    assert_eq!(result["hard_stop"], false);

    let intent_events = intent_events.lock().expect("events mutex poisoned").clone();
    assert_eq!(intent_events.any, vec!["helper_result"]);
    assert!(intent_events.helper_calls.is_empty());
    assert_eq!(intent_events.helper_results.len(), 1);
    assert!(intent_events.helper_results[0].contains("Fact"));
    assert!(intent_events.helper_results[0].contains("deterministic helper override"));

    let middleware_events = middleware_events
        .lock()
        .expect("middleware events mutex poisoned")
        .clone();
    assert_eq!(middleware_events.names, vec!["helper_call", "helper_result"]);
    assert_eq!(
        middleware_events.raw_blocks,
        vec![
            Some("[helper_call:Fact]input:called helper[/helper]".to_string()),
            None
        ]
    );

    let final_context = agent.shared_context();
    assert_eq!(final_context.get("helper_call_seen"), Some(&json!(true)));
    assert_eq!(final_context.get("helper_result_seen"), Some(&json!(true)));

    let helper_result = &intent_events.helper_results[0];
    let expected_fact = FactOutput {
        is_fact: true,
        confidence: 0.99,
        reasons: "deterministic helper override".to_string(),
    };
    assert!(helper_result.contains(&format!("{:?}", expected_fact)));
}
