// Auto-generated Rust bindings for SimpleTool
// Do not edit manually
use async_trait::async_trait;
use auwgent_sdk_rust as sdk;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::marker::PhantomData;
use std::sync::Arc;
pub type IntentControl = sdk::IntentControl;
pub type SessionState = sdk::SessionState;
pub type Session = SessionState;
pub type Context = sdk::MiddlewareContext;
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NoArgs {}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PartialTextIntentValue {
    #[serde(flatten)]
    pub raw: JsonMap<String, JsonValue>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PartialStructuredIntentValue<T> {
    #[serde(flatten)]
    pub raw: JsonMap<String, JsonValue>,
    #[serde(skip)]
    pub marker: PhantomData<T>,
}

pub type SimpleToolInput = JsonValue;

pub type JokerOutput = JsonValue;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolBaseOutput;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SimpleToolOutput {
    Base(SimpleToolBaseOutput),
    Joker(JokerOutput),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolContext {
    pub user_name: String,
    pub age: f64,
    pub id: String,
}

pub type SimpleToolGetLocationToolResultValue = String;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolGetMarksToolArgs {
    pub id: String,
}

pub type SimpleToolGetMarksToolResultValue = String;

pub trait SimpleToolTools: Send + Sync + 'static {
    fn get_location(&self, args: NoArgs) -> SimpleToolGetLocationToolResultValue;
    fn get_marks(&self, args: SimpleToolGetMarksToolArgs) -> SimpleToolGetMarksToolResultValue;
}

#[derive(Clone)]
pub struct SimpleToolToolsRegistry(pub Arc<dyn SimpleToolTools>);

impl<T> From<T> for SimpleToolToolsRegistry
where
    T: SimpleToolTools,
{
    fn from(value: T) -> Self {
        Self(Arc::new(value))
    }
}

impl sdk::ToolRegistrar for SimpleToolToolsRegistry {
    fn tool_names(&self) -> &'static [&'static str] {
        &["get_location", "get_marks"]
    }

    fn invoke_tool(
        &self,
        name: &'static str,
        args: JsonValue,
    ) -> sdk::BoxFuture<'static, sdk::AuwgentResult<JsonValue>> {
        match name {
            "get_location" => {
                let tools = Arc::clone(&self.0);
                Box::pin(async move {
                    let parsed: NoArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;
                    let result = tools.get_location(parsed);
                    serde_json::to_value(result).map_err(|e| e.to_string())
                })
            },
            "get_marks" => {
                let tools = Arc::clone(&self.0);
                Box::pin(async move {
                    let parsed: SimpleToolGetMarksToolArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;
                    let result = tools.get_marks(parsed);
                    serde_json::to_value(result).map_err(|e| e.to_string())
                })
            }
            _ => Box::pin(async move { Err(format!("Unknown tool: {name}")) }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleToolIntentName {
    ResponseText,
    ResponseSchema,
    Error,
    ToolCall,
    ToolResult,
    ToolError,
    ToolSkipped,
    WorkflowCall,
    WorkflowResult,
    HelperCall,
    HelperResult,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolResponseTextIntent {
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolResponseSchemaIntent {
    #[serde(rename = "type")]
    pub kind: String,
    pub response: SimpleToolOutput,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolErrorIntent {
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SimpleToolToolCallIntent {
    #[serde(rename = "get_location")]
    GetLocation,
    #[serde(rename = "get_marks")]
    GetMarks {
        args: SimpleToolGetMarksToolArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum SimpleToolToolResultIntent {
    #[serde(rename = "get_location")]
    GetLocation {
        args: NoArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "get_marks")]
    GetMarks {
        args: SimpleToolGetMarksToolArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SimpleToolToolSkippedIntent {
    #[serde(rename = "get_location")]
    GetLocation,
    #[serde(rename = "get_marks")]
    GetMarks {
        args: SimpleToolGetMarksToolArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolToolErrorIntent {
    pub tool: String,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolMarksAndLocationWorkflowArgs {
    pub user_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SimpleToolWorkflowCallIntent {
    #[serde(rename = "marks_and_location")]
    MarksAndLocation {
        args: SimpleToolMarksAndLocationWorkflowArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum SimpleToolWorkflowResultIntent {
    #[serde(rename = "marks_and_location")]
    MarksAndLocation {
        args: SimpleToolMarksAndLocationWorkflowArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
}

pub type SimpleToolJokerHelperArgs = JsonValue;

pub type SimpleToolPlanHelperArgs = JsonValue;

pub type SimpleToolFactHelperArgs = JsonValue;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SimpleToolHelperCallIntent {
    #[serde(rename = "Joker")]
    Joker {
        args: SimpleToolJokerHelperArgs,
    },
    #[serde(rename = "Plan")]
    Plan {
        args: SimpleToolPlanHelperArgs,
    },
    #[serde(rename = "Fact")]
    Fact {
        args: SimpleToolFactHelperArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum SimpleToolHelperResultIntent {
    #[serde(rename = "Joker")]
    Joker {
        args: SimpleToolJokerHelperArgs,
        result: (),
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "Plan")]
    Plan {
        args: SimpleToolPlanHelperArgs,
        result: JsonValue,
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "Fact")]
    Fact {
        args: SimpleToolFactHelperArgs,
        result: JsonValue,
        #[serde(default)]
        overridden: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SimpleToolIntent {
    ResponseText(SimpleToolResponseTextIntent),
    ResponseSchema(SimpleToolResponseSchemaIntent),
    Error(SimpleToolErrorIntent),
    ToolCall(SimpleToolToolCallIntent),
    ToolResult(SimpleToolToolResultIntent),
    ToolError(SimpleToolToolErrorIntent),
    ToolSkipped(SimpleToolToolSkippedIntent),
    WorkflowCall(SimpleToolWorkflowCallIntent),
    WorkflowResult(SimpleToolWorkflowResultIntent),
    HelperCall(SimpleToolHelperCallIntent),
    HelperResult(SimpleToolHelperResultIntent),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SimpleToolIntentPartial {
    ResponseText(PartialTextIntentValue),
    ResponseSchema(PartialStructuredIntentValue<SimpleToolResponseSchemaIntent>),
    Error(PartialStructuredIntentValue<SimpleToolErrorIntent>),
    ToolCall(PartialStructuredIntentValue<SimpleToolToolCallIntent>),
    ToolResult(PartialStructuredIntentValue<SimpleToolToolResultIntent>),
    ToolError(PartialStructuredIntentValue<SimpleToolToolErrorIntent>),
    ToolSkipped(PartialStructuredIntentValue<SimpleToolToolSkippedIntent>),
    WorkflowCall(PartialStructuredIntentValue<SimpleToolWorkflowCallIntent>),
    WorkflowResult(PartialStructuredIntentValue<SimpleToolWorkflowResultIntent>),
    HelperCall(PartialStructuredIntentValue<SimpleToolHelperCallIntent>),
    HelperResult(PartialStructuredIntentValue<SimpleToolHelperResultIntent>),
}

impl SimpleToolIntentName {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
        "response_text" => Some(SimpleToolIntentName::ResponseText),
        "response_schema" => Some(SimpleToolIntentName::ResponseSchema),
        "error" => Some(SimpleToolIntentName::Error),
        "tool_call" => Some(SimpleToolIntentName::ToolCall),
        "tool_result" => Some(SimpleToolIntentName::ToolResult),
        "tool_error" => Some(SimpleToolIntentName::ToolError),
        "tool_skipped" => Some(SimpleToolIntentName::ToolSkipped),
        "workflow_call" => Some(SimpleToolIntentName::WorkflowCall),
        "workflow_result" => Some(SimpleToolIntentName::WorkflowResult),
        "helper_call" => Some(SimpleToolIntentName::HelperCall),
        "helper_result" => Some(SimpleToolIntentName::HelperResult),
            _ => None,
        }
    }
}

impl SimpleToolIntent {
    pub fn decode(name: SimpleToolIntentName, value: JsonValue) -> Option<Self> {
        match name {
        SimpleToolIntentName::ResponseText => serde_json::from_value(value).ok().map(SimpleToolIntent::ResponseText),
        SimpleToolIntentName::ResponseSchema => serde_json::from_value(value).ok().map(SimpleToolIntent::ResponseSchema),
        SimpleToolIntentName::Error => serde_json::from_value(value).ok().map(SimpleToolIntent::Error),
        SimpleToolIntentName::ToolCall => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolCall),
        SimpleToolIntentName::ToolResult => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolResult),
        SimpleToolIntentName::ToolError => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolError),
        SimpleToolIntentName::ToolSkipped => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolSkipped),
        SimpleToolIntentName::WorkflowCall => serde_json::from_value(value).ok().map(SimpleToolIntent::WorkflowCall),
        SimpleToolIntentName::WorkflowResult => serde_json::from_value(value).ok().map(SimpleToolIntent::WorkflowResult),
        SimpleToolIntentName::HelperCall => serde_json::from_value(value).ok().map(SimpleToolIntent::HelperCall),
        SimpleToolIntentName::HelperResult => serde_json::from_value(value).ok().map(SimpleToolIntent::HelperResult),
        }
    }
}

impl SimpleToolIntentPartial {
    pub fn decode(name: SimpleToolIntentName, value: JsonValue) -> Option<Self> {
        match name {
        SimpleToolIntentName::ResponseText => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ResponseText),
        SimpleToolIntentName::ResponseSchema => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ResponseSchema),
        SimpleToolIntentName::Error => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::Error),
        SimpleToolIntentName::ToolCall => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolCall),
        SimpleToolIntentName::ToolResult => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolResult),
        SimpleToolIntentName::ToolError => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolError),
        SimpleToolIntentName::ToolSkipped => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolSkipped),
        SimpleToolIntentName::WorkflowCall => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::WorkflowCall),
        SimpleToolIntentName::WorkflowResult => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::WorkflowResult),
        SimpleToolIntentName::HelperCall => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::HelperCall),
        SimpleToolIntentName::HelperResult => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::HelperResult),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleToolIntentView {
    inner: SimpleToolIntent,
}

impl SimpleToolIntentView {
    pub fn new(inner: SimpleToolIntent) -> Self {
        Self { inner }
    }

    pub fn raw(&self) -> &SimpleToolIntent {
        &self.inner
    }

    pub fn name(&self) -> &'static str {
        match &self.inner {
            SimpleToolIntent::ResponseText(_) => "response_text",
            SimpleToolIntent::ResponseSchema(_) => "response_schema",
            SimpleToolIntent::Error(_) => "error",
            SimpleToolIntent::ToolCall(..) => "tool_call",
            SimpleToolIntent::ToolResult(..) => "tool_result",
            SimpleToolIntent::ToolError(..) => "tool_error",
            SimpleToolIntent::ToolSkipped(..) => "tool_skipped",
            SimpleToolIntent::WorkflowCall(..) => "workflow_call",
            SimpleToolIntent::WorkflowResult(..) => "workflow_result",
            SimpleToolIntent::HelperCall(..) => "helper_call",
            SimpleToolIntent::HelperResult(..) => "helper_result",
        }
    }

    pub fn text(&self) -> &str {
        match &self.inner {
            SimpleToolIntent::ResponseText(intent) => &intent.text,
            _ => panic!("intent does not contain text"),
        }
    }

    pub fn message(&self) -> &str {
        match &self.inner {
            SimpleToolIntent::Error(intent) => &intent.message,
            SimpleToolIntent::ToolError(intent) => &intent.message,
            _ => panic!("intent does not contain a message"),
        }
    }

    pub fn response<T>(&self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let value = match &self.inner {
            SimpleToolIntent::ResponseSchema(intent) => serde_json::to_value(intent.response.clone()),
            _ => panic!("intent does not contain a response"),
        }.expect("response should serialize");
        serde_json::from_value(value).expect("response should deserialize")
    }

    pub fn args<T>(&self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let value = match &self.inner {
            SimpleToolIntent::ToolCall(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::ToolResult(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::ToolSkipped(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::WorkflowCall(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::WorkflowResult(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::HelperCall(intent) => serde_json::to_value(intent.clone()),
            SimpleToolIntent::HelperResult(intent) => serde_json::to_value(intent.clone()),
            _ => panic!("intent does not contain args"),
        }.expect("args should serialize");
        serde_json::from_value(value).expect("args should deserialize")
    }
}

pub trait SimpleToolIntentHandler: Send + Sync + 'static {
    fn response_text(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn response_schema(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn tool_call(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn tool_result(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn tool_error(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn tool_skipped(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn workflow_call(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn workflow_result(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn helper_call(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn helper_result(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn error(&self, _intent: &SimpleToolIntentView, _agent: &str) {}
    fn any(&self, _intent: &SimpleToolIntentView, _agent: &str) {}

    fn dispatch(&self, intent: &SimpleToolIntentView, agent_name: &str) -> Option<IntentControl> {
        self.any(intent, agent_name);
        match intent.raw() {
        SimpleToolIntent::ResponseText(_) => self.response_text(intent, agent_name),
        SimpleToolIntent::ResponseSchema(_) => self.response_schema(intent, agent_name),
        SimpleToolIntent::Error(_) => self.error(intent, agent_name),
        SimpleToolIntent::ToolCall(..) => self.tool_call(intent, agent_name),
        SimpleToolIntent::ToolResult(..) => self.tool_result(intent, agent_name),
        SimpleToolIntent::ToolError(..) => self.tool_error(intent, agent_name),
        SimpleToolIntent::ToolSkipped(..) => self.tool_skipped(intent, agent_name),
        SimpleToolIntent::WorkflowCall(..) => self.workflow_call(intent, agent_name),
        SimpleToolIntent::WorkflowResult(..) => self.workflow_result(intent, agent_name),
        SimpleToolIntent::HelperCall(..) => self.helper_call(intent, agent_name),
        SimpleToolIntent::HelperResult(..) => self.helper_result(intent, agent_name),
        }
        None
    }
}

pub trait SimpleToolBasePartialIntentHandler {
    fn on_intent_partial(&self, intent: SimpleToolIntentPartial, agent_name: &str) { let _ = (intent, agent_name); }

    fn dispatch_partial(&self, intent: SimpleToolIntentPartial, agent_name: &str) {
        self.on_intent_partial(intent, agent_name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SimpleToolApiKeys {
    pub groq_api_key: Option<String>,
}

impl From<SimpleToolApiKeys> for sdk::AuwgentApiKeys {
    fn from(value: SimpleToolApiKeys) -> Self {
        sdk::AuwgentApiKeys {
            groq_api_key: value.groq_api_key,
            ..sdk::AuwgentApiKeys::default()
        }
    }
}

#[async_trait]
pub trait SimpleToolMiddleware: Send + Sync + 'static {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn target(&self) -> Option<Vec<String>> {
        None
    }

    async fn on_run_start(&self, session: Session, _ctx: &Context) -> Session {
        session
    }

    async fn on_llm_start(&self, prompt: String, _ctx: &Context) -> String {
        prompt
    }

    async fn on_intent(&self, _intent: &SimpleToolIntentView, _ctx: &Context) -> Option<IntentControl> {
        None
    }

    async fn on_intent_partial(&self, _intent: &SimpleToolIntentPartial, _ctx: &Context) {}

    async fn on_llm_end(&self, _response: &JsonValue, _ctx: &Context) {}

    async fn on_run_complete(&self, _session: &Session, _ctx: &Context) {}

    async fn on_error(&self, _error: &JsonValue, _session: Option<&Session>, _ctx: &Context) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct SimpleToolMiddlewareRegistry(pub sdk::MiddlewareRegistry);

struct SimpleToolMiddlewareAdapter<T>(T);

#[async_trait]
impl<T> sdk::Middleware for SimpleToolMiddlewareAdapter<T>
where
    T: SimpleToolMiddleware,
{
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn target(&self) -> Option<Vec<String>> {
        self.0.target()
    }

    async fn on_run_start(
        &self,
        session: SessionState,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<SessionState> {
        Ok(self.0.on_run_start(session, ctx).await)
    }

    async fn on_llm_start(
        &self,
        prompt: String,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<Option<String>> {
        Ok(Some(self.0.on_llm_start(prompt, ctx).await))
    }

    async fn on_intent(
        &self,
        name: &str,
        value: &JsonValue,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<Option<IntentControl>> {
        let Some(intent_name) = SimpleToolIntentName::parse(name) else {
            return Ok(None);
        };
        let Some(intent) = SimpleToolIntent::decode(intent_name, value.clone()) else {
            return Ok(None);
        };
        let intent = SimpleToolIntentView::new(intent);
        Ok(self.0.on_intent(&intent, ctx).await)
    }

    async fn on_intent_partial(
        &self,
        name: &str,
        value: &JsonValue,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<()> {
        if let Some(intent_name) = SimpleToolIntentName::parse(name)
            && let Some(intent) = SimpleToolIntentPartial::decode(intent_name, value.clone())
        {
            self.0.on_intent_partial(&intent, ctx).await;
        }
        Ok(())
    }

    async fn on_llm_end(
        &self,
        response: &JsonValue,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<()> {
        self.0.on_llm_end(response, ctx).await;
        Ok(())
    }

    async fn on_run_complete(
        &self,
        session: &SessionState,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<()> {
        self.0.on_run_complete(session, ctx).await;
        Ok(())
    }

    async fn on_error(
        &self,
        error: &JsonValue,
        session: Option<&SessionState>,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<bool> {
        Ok(self.0.on_error(error, session, ctx).await)
    }
}

impl<T> From<T> for SimpleToolMiddlewareRegistry
where
    T: SimpleToolMiddleware,
{
    fn from(value: T) -> Self {
        Self(Arc::new(SimpleToolMiddlewareAdapter(value)))
    }
}

impl From<sdk::MiddlewareRegistry> for SimpleToolMiddlewareRegistry {
    fn from(value: sdk::MiddlewareRegistry) -> Self {
        Self(value)
    }
}

#[derive(Clone)]
pub struct SimpleToolConfig<TTools = SimpleToolToolsRegistry, TMiddleware = SimpleToolMiddlewareRegistry> {
    pub tools: TTools,
    pub middleware: Vec<TMiddleware>,
    pub context: SimpleToolContext,
    pub api_keys: SimpleToolApiKeys,
}

pub struct SimpleToolAgent {
    inner: sdk::TypedAuwgent<SimpleToolToolsRegistry>,
}

impl std::ops::Deref for SimpleToolAgent {
    type Target = sdk::TypedAuwgent<SimpleToolToolsRegistry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl SimpleToolAgent {
    pub fn on_intent<H>(&self, handler: H)
    where
        H: SimpleToolIntentHandler,
    {
        let handler = Arc::new(handler);
        self.inner.on_decoded_intent(SimpleToolIntentName::parse, SimpleToolIntent::decode, move |intent, agent_name| {
            let intent = SimpleToolIntentView::new(intent);
            handler.dispatch(&intent, agent_name)
        });
    }

    pub fn on_intent_raw<F>(&self, handler: F)
    where
        F: FnMut(SimpleToolIntent, &str) -> Option<IntentControl> + Send + 'static,
    {
        self.inner.on_decoded_intent(SimpleToolIntentName::parse, SimpleToolIntent::decode, handler);
    }

    pub fn on_intent_handler<H>(&self, handler: H)
    where
        H: SimpleToolIntentHandler,
    {
        self.on_intent(handler);
    }

    pub fn on_intent_partial<F>(&self, handler: F)
    where
        F: FnMut(SimpleToolIntentPartial, &str) + Send + 'static,
    {
        self.inner.on_decoded_intent_partial(SimpleToolIntentName::parse, SimpleToolIntentPartial::decode, handler);
    }

    pub fn on_intent_partial_handler<H>(&self, handler: H)
    where
        H: SimpleToolBasePartialIntentHandler + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        self.on_intent_partial(move |intent, agent_name| {
            handler.dispatch_partial(intent, agent_name);
        });
    }

    pub async fn run(&self, input: Option<SimpleToolInput>) -> sdk::AuwgentResult<SessionState> {
        let input = input.map(serde_json::to_value).transpose().map_err(|e| e.to_string())?;
        self.inner.run(input).await
    }
}

pub fn create_simpletool<TTools, TMiddleware>(config: SimpleToolConfig<TTools, TMiddleware>) -> sdk::AuwgentResult<SimpleToolAgent>
where
    TTools: Into<SimpleToolToolsRegistry>,
    TMiddleware: Into<SimpleToolMiddlewareRegistry>,
{
    let ir = sdk::parse_ir(include_str!("./main.agent.json"))?;
    let middleware = config.middleware.into_iter().map(|item| {
        let registry: SimpleToolMiddlewareRegistry = item.into();
        registry.0
    }).collect();
    let sdk_config = sdk::AuwgentConfig {
        tools: config.tools.into(),
        middleware,
        context: Some(serde_json::to_value(config.context).map_err(|e| e.to_string())?),
        api_keys: config.api_keys.into(),
    };
    let inner = sdk::create_auwgent(ir, sdk_config)?;
    Ok(SimpleToolAgent { inner })
}

pub fn auwgent<TTools, TMiddleware>(config: SimpleToolConfig<TTools, TMiddleware>) -> sdk::AuwgentResult<SimpleToolAgent>
where
    TTools: Into<SimpleToolToolsRegistry>,
    TMiddleware: Into<SimpleToolMiddlewareRegistry>,
{
    create_simpletool(config)
}

pub use SimpleToolAgent as AuwgentAgent;
pub use SimpleToolConfig as AuwgentConfig;
pub use SimpleToolIntent as AuwgentIntent;
pub use SimpleToolIntentPartial as AuwgentIntentPartial;
pub use SimpleToolIntentName as AuwgentIntentName;
pub use SimpleToolIntentHandler as AuwgentIntentHandler;
pub use SimpleToolBasePartialIntentHandler as AuwgentBasePartialIntentHandler;
pub use SimpleToolMiddleware as AuwgentMiddleware;
pub use SimpleToolMiddlewareRegistry as AuwgentMiddlewareRegistry;
pub use SimpleToolIntentView as Intent;
pub use SimpleToolContext as AuwgentContext;
pub use SimpleToolTools as AuwgentTools;
pub use SimpleToolApiKeys as AuwgentApiKeys;