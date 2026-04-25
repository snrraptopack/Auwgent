use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Map, Value};

pub mod fixture {
    #![allow(clippy::all)]
    #![allow(dead_code)]

    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test_case/main.agent.rs"));
}

pub fn crate_name() -> &'static str {
    "auwgent-testing"
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordedIntent {
    ResponseText(String),
    Error(String),
    ToolCall { name: String, args: Value },
    ToolResult {
        name: String,
        args: Value,
        result: Value,
        overridden: bool,
    },
    ToolSkipped { name: String, args: Value },
    WorkflowCall { name: String, args: Value },
    WorkflowResult {
        name: String,
        args: Value,
        result: Value,
        overridden: bool,
    },
    HelperCall { name: String, args: Value },
    HelperResult {
        name: String,
        args: Value,
        result: Value,
        overridden: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TestTools;

impl fixture::AuwgentTools for TestTools {
    fn get_location(&self) -> fixture::GetLocationResult {
        "Accra".to_string()
    }

    fn get_marks(&self, args: fixture::GetMarksArgs) -> fixture::GetMarksResult {
        format!("marks:{}", args.id)
    }
}

pub fn base_context() -> fixture::AuwgentContext {
    fixture::AuwgentContext {
        user_name: "Ada".into(),
        age: 42.0,
        id: "user_42".into(),
    }
}

pub fn build_agent<TMiddleware>(
    middleware: Vec<TMiddleware>,
) -> fixture::AuwgentAgent
where
    TMiddleware: Into<fixture::AuwgentMiddlewareRegistry>,
{
    fixture::auwgent(fixture::AuwgentConfig {
        tools: TestTools,
        middleware,
        context: base_context(),
        api_keys: fixture::AuwgentApiKeys::default(),
    })
    .expect("fixture agent should build")
}

pub fn drive_chunk(
    agent: &fixture::AuwgentAgent,
    chunk: &str,
) -> auwgent_sdk_rust::AuwgentResult<Value> {
    agent.raw().write_chunk(chunk.to_string());
    let end = agent.raw().end_stream()?;
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        agent.raw().process_intents().await?;
        Ok::<_, String>(end)
    })
}

pub fn attach_intent_collector(
    agent: &fixture::AuwgentAgent,
) -> Arc<Mutex<Vec<RecordedIntent>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&events);
    agent.on_intent_raw(move |intent, _agent_name| {
        shared.lock().unwrap().push(record_intent(intent));
        None
    });
    events
}

fn record_intent(intent: fixture::AuwgentIntent) -> RecordedIntent {
    match intent {
        fixture::AuwgentIntent::ResponseText(value) => RecordedIntent::ResponseText(value.text),
        fixture::AuwgentIntent::Error(value) => RecordedIntent::Error(value.message),
        fixture::AuwgentIntent::ToolCall(value) => match value {
            fixture::SimpleToolToolCall::GetLocation => RecordedIntent::ToolCall {
                name: "get_location".into(),
                args: json!({}),
            },
            fixture::SimpleToolToolCall::GetMarks { args } => RecordedIntent::ToolCall {
                name: "get_marks".into(),
                args: serde_json::to_value(args).unwrap(),
            },
        },
        fixture::SimpleToolIntent::ToolResult(value) => match value {
            fixture::SimpleToolToolResult::GetLocation {
                args,
                result,
                overridden,
            } => RecordedIntent::ToolResult {
                name: "get_location".into(),
                args: serde_json::to_value(args).unwrap(),
                result: json!(result),
                overridden,
            },
            fixture::SimpleToolToolResult::GetMarks {
                args,
                result,
                overridden,
            } => RecordedIntent::ToolResult {
                name: "get_marks".into(),
                args: serde_json::to_value(args).unwrap(),
                result: json!(result),
                overridden,
            },
        },
        fixture::SimpleToolIntent::ToolSkipped(value) => match value {
            fixture::SimpleToolToolSkipped::GetLocation => RecordedIntent::ToolSkipped {
                name: "get_location".into(),
                args: json!({}),
            },
            fixture::SimpleToolToolSkipped::GetMarks { args } => RecordedIntent::ToolSkipped {
                name: "get_marks".into(),
                args: serde_json::to_value(args).unwrap(),
            },
        },
        fixture::SimpleToolIntent::WorkflowCall(value) => match value {
            fixture::SimpleToolWorkflowCall::MarksAndLocation { args } => {
                RecordedIntent::WorkflowCall {
                    name: "marks_and_location".into(),
                    args: serde_json::to_value(args).unwrap(),
                }
            }
        },
        fixture::SimpleToolIntent::WorkflowResult(value) => match value {
            fixture::SimpleToolWorkflowResult::MarksAndLocation {
                args,
                result,
                overridden,
            } => RecordedIntent::WorkflowResult {
                name: "marks_and_location".into(),
                args: serde_json::to_value(args).unwrap(),
                result: json!(result),
                overridden,
            },
        },
        fixture::SimpleToolIntent::HelperCall(value) => match value {
            fixture::SimpleToolHelperCall::Joker { args } => RecordedIntent::HelperCall {
                name: "Joker".into(),
                args,
            },
            fixture::SimpleToolHelperCall::Plan { args } => RecordedIntent::HelperCall {
                name: "Plan".into(),
                args,
            },
            fixture::SimpleToolHelperCall::Fact { args } => RecordedIntent::HelperCall {
                name: "Fact".into(),
                args,
            },
        },
        fixture::SimpleToolIntent::HelperResult(value) => match value {
            fixture::SimpleToolHelperResult::Joker {
                args,
                overridden,
                ..
            } => RecordedIntent::HelperResult {
                name: "Joker".into(),
                args,
                result: Value::Null,
                overridden,
            },
            fixture::SimpleToolHelperResult::Plan {
                args,
                result,
                overridden,
            } => RecordedIntent::HelperResult {
                name: "Plan".into(),
                args,
                result,
                overridden,
            },
            fixture::SimpleToolHelperResult::Fact {
                args,
                result,
                overridden,
            } => RecordedIntent::HelperResult {
                name: "Fact".into(),
                args,
                result,
                overridden,
            },
        },
        fixture::SimpleToolIntent::ToolError(value) => RecordedIntent::Error(value.message),
        fixture::SimpleToolIntent::ResponseSchema(value) => {
            RecordedIntent::HelperResult {
                name: value.kind,
                args: Value::Null,
                result: serde_json::to_value(value.response).unwrap(),
                overridden: false,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolControlMode {
    None,
    SkipGetMarks,
    OverrideGetMarks,
}

#[derive(Clone)]
pub struct TraceMiddleware {
    pub events: Arc<Mutex<Vec<String>>>,
    pub control_mode: ToolControlMode,
    pub target: Option<Vec<String>>,
}

impl Default for TraceMiddleware {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            control_mode: ToolControlMode::None,
            target: None,
        }
    }
}

#[async_trait]
impl fixture::SimpleToolMiddleware for TraceMiddleware {
    fn name(&self) -> &'static str {
        "trace"
    }

    fn target(&self) -> Option<Vec<String>> {
        self.target.clone()
    }

    async fn on_intent(
        &self,
        intent: &fixture::Intent,
        ctx: &fixture::Context,
    ) -> Option<fixture::IntentControl> {
        self.events
            .lock()
            .unwrap()
            .push(format!("intent:{}", intent.name()));

        let mut data: Map<String, Value> = ctx.data.clone();
        data.insert("last_intent".into(), json!(intent.name()));
        ctx.set_context(Value::Object(data.clone()));

        match (self.control_mode, intent.raw()) {
            (
                ToolControlMode::SkipGetMarks,
                fixture::SimpleToolIntent::ToolCall(
                    fixture::SimpleToolToolCall::GetMarks { .. },
                ),
            ) => Some(fixture::IntentControl::Skip),
            (
                ToolControlMode::OverrideGetMarks,
                fixture::SimpleToolIntent::ToolCall(
                    fixture::SimpleToolToolCall::GetMarks { .. },
                ),
            ) => Some(fixture::IntentControl::Override {
                result: json!("override:marks"),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub struct LifecycleMiddleware {
    pub events: Arc<Mutex<Vec<String>>>,
    pub seen_data: Arc<Mutex<Vec<Map<String, Value>>>>,
    pub swallow_errors: bool,
}

#[async_trait]
impl fixture::SimpleToolMiddleware for LifecycleMiddleware {
    fn name(&self) -> &'static str {
        "lifecycle"
    }

    async fn on_run_start(
        &self,
        mut session: fixture::Session,
        ctx: &fixture::Context,
    ) -> fixture::Session {
        self.events.lock().unwrap().push("run_start".into());
        self.seen_data.lock().unwrap().push(ctx.data.clone());

        let mut next = ctx.data.clone();
        next.insert("phase".into(), json!("run_start"));
        ctx.set_context(Value::Object(next));

        session.stack.push("run_start_seen".into());
        session
    }

    async fn on_llm_start(&self, prompt: String, ctx: &fixture::Context) -> String {
        self.events.lock().unwrap().push("llm_start".into());
        self.seen_data.lock().unwrap().push(ctx.data.clone());

        let mut next = ctx.data.clone();
        next.insert("phase".into(), json!("llm_start"));
        ctx.set_context(Value::Object(next));

        format!("[middleware-prefix]\n{prompt}")
    }

    async fn on_llm_end(&self, _response: &Value, ctx: &fixture::Context) {
        self.events.lock().unwrap().push("llm_end".into());
        self.seen_data.lock().unwrap().push(ctx.data.clone());
    }

    async fn on_run_complete(&self, _session: &fixture::Session, ctx: &fixture::Context) {
        self.events.lock().unwrap().push("run_complete".into());
        self.seen_data.lock().unwrap().push(ctx.data.clone());
    }

    async fn on_error(
        &self,
        error: &Value,
        _session: Option<&fixture::Session>,
        ctx: &fixture::Context,
    ) -> bool {
        self.events
            .lock()
            .unwrap()
            .push(format!("error:{}", error.get("message").and_then(Value::as_str).unwrap_or("unknown")));
        self.seen_data.lock().unwrap().push(ctx.data.clone());
        self.swallow_errors
    }
}

pub async fn run_without_keys(
    middleware: Vec<fixture::SimpleToolMiddlewareRegistry>,
) -> auwgent_sdk_rust::AuwgentResult<fixture::SessionState> {
    let agent = build_agent(middleware);
    agent.run(Some(json!("say hello"))).await
}

pub fn live_guard() -> Result<(), String> {
    if std::env::var("GROQ_API_KEY").ok().filter(|v| !v.is_empty()).is_none() {
        return Err("set GROQ_API_KEY to run live testing".into());
    }
    Ok(())
}
