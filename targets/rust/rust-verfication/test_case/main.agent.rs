// Auto-generated Rust bindings for SimpleTool
// Do not edit manually
use async_trait::async_trait;
use auwgent_sdk_rust as sdk;
use serde_json::Value as JsonValue;
use std::sync::Arc;
pub type IntentControl = sdk::IntentControl;
pub type SessionState = sdk::SessionState;
pub type Session = SessionState;
pub type Context = sdk::MiddlewareContext;
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NoArgs {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialIntentMode {
    Text,
    Structured,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartialIntentEnvelope {
    pub partial: bool,
    pub complete: bool,
    pub mode: PartialIntentMode,
    pub segment: i64,
    pub raw: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartialTextIntentValue {
    #[serde(flatten)]
    pub envelope: PartialIntentEnvelope,
    pub text: String,
    #[serde(default)]
    pub delta: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartialStructuredIntentValue<T> {
    #[serde(flatten)]
    pub envelope: PartialIntentEnvelope,
    #[serde(flatten)]
    pub value: T,
}

pub type AuwgentInput = JsonValue;

pub type JokerOutput = ();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanOutput {
    pub steps: Vec<String>,
    pub motivation: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FactOutput {
    pub is_fact: bool,
    pub confidence: f64,
    pub reasons: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AuwgentOutput {
    Plan(PlanOutput),
    Fact(FactOutput),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuwgentContext {
    pub user_name: String,
    pub age: f64,
    pub id: String,
}

pub type GetLocationResult = String;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetMarksArgs {
    pub id: String,
}

pub type GetMarksResult = String;

pub trait AuwgentTools: Send + Sync + 'static {
    fn get_location(&self) -> GetLocationResult;
    fn get_marks(&self, args: GetMarksArgs) -> GetMarksResult;
}

#[derive(Clone)]
pub struct AuwgentToolsRegistry(pub Arc<dyn AuwgentTools>);

impl<T> From<T> for AuwgentToolsRegistry
where
    T: AuwgentTools,
{
    fn from(value: T) -> Self {
        Self(Arc::new(value))
    }
}

impl sdk::ToolRegistrar for AuwgentToolsRegistry {
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
                    let _: NoArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;
                    let result = tools.get_location();
                    serde_json::to_value(result).map_err(|e| e.to_string())
                })
            },
            "get_marks" => {
                let tools = Arc::clone(&self.0);
                Box::pin(async move {
                    let parsed: GetMarksArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;
                    let result = tools.get_marks(parsed);
                    serde_json::to_value(result).map_err(|e| e.to_string())
                })
            }
            _ => Box::pin(async move { Err(format!("Unknown tool: {name}")) }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuwgentIntentName {
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
    Loud,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoudIntent {
    pub actions: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponseText {
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ResponseSchema {
    #[serde(rename = "PlanOutput")]
    PlanOutput {
        response: PlanOutput,
    },
    #[serde(rename = "FactOutput")]
    FactOutput {
        response: FactOutput,
    },
}

impl ResponseSchema {
    pub fn response_value(&self) -> JsonValue {
        match self {
            ResponseSchema::PlanOutput { response } => serde_json::to_value(response.clone()),
            ResponseSchema::FactOutput { response } => serde_json::to_value(response.clone()),
        }.expect("response should serialize")
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            ResponseSchema::PlanOutput { .. } => "PlanOutput",
            ResponseSchema::FactOutput { .. } => "FactOutput",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorIntent {
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetMarksToolArgs {
    pub id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ToolCall {
    #[serde(rename = "get_location")]
    GetLocation,
    #[serde(rename = "get_marks")]
    GetMarks {
        args: GetMarksToolArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum ToolResult {
    #[serde(rename = "get_location")]
    GetLocation {
        args: NoArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "get_marks")]
    GetMarks {
        args: GetMarksToolArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ToolSkipped {
    #[serde(rename = "get_location")]
    GetLocation,
    #[serde(rename = "get_marks")]
    GetMarks {
        args: GetMarksToolArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolError {
    pub tool: String,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarksAndLocationWorkflowArgs {
    pub user_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowCall {
    #[serde(rename = "marks_and_location")]
    MarksAndLocation {
        args: MarksAndLocationWorkflowArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum WorkflowResult {
    #[serde(rename = "marks_and_location")]
    MarksAndLocation {
        args: MarksAndLocationWorkflowArgs,
        result: String,
        #[serde(default)]
        overridden: bool,
    },
}

pub type JokerHelperArgs = JsonValue;

pub type PlanHelperArgs = JsonValue;

pub type FactHelperArgs = JsonValue;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum HelperCall {
    #[serde(rename = "Joker")]
    Joker {
        args: JokerHelperArgs,
    },
    #[serde(rename = "Plan")]
    Plan {
        args: PlanHelperArgs,
    },
    #[serde(rename = "Fact")]
    Fact {
        args: FactHelperArgs,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "name")]
pub enum HelperResult {
    #[serde(rename = "Joker")]
    Joker {
        args: JokerHelperArgs,
        result: JokerOutput,
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "Plan")]
    Plan {
        args: PlanHelperArgs,
        result: PlanOutput,
        #[serde(default)]
        overridden: bool,
    },
    #[serde(rename = "Fact")]
    Fact {
        args: FactHelperArgs,
        result: FactOutput,
        #[serde(default)]
        overridden: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuwgentIntent {
    ResponseText(ResponseText),
    ResponseSchema(ResponseSchema),
    Error(ErrorIntent),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    ToolError(ToolError),
    ToolSkipped(ToolSkipped),
    WorkflowCall(WorkflowCall),
    WorkflowResult(WorkflowResult),
    HelperCall(HelperCall),
    HelperResult(HelperResult),
    Loud(LoudIntent),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuwgentIntentPartial {
    ResponseText(PartialTextIntentValue),
    ResponseSchema(PartialStructuredIntentValue<ResponseSchema>),
    Error(PartialStructuredIntentValue<ErrorIntent>),
    ToolCall(PartialStructuredIntentValue<ToolCall>),
    ToolResult(PartialStructuredIntentValue<ToolResult>),
    ToolError(PartialStructuredIntentValue<ToolError>),
    ToolSkipped(PartialStructuredIntentValue<ToolSkipped>),
    WorkflowCall(PartialStructuredIntentValue<WorkflowCall>),
    WorkflowResult(PartialStructuredIntentValue<WorkflowResult>),
    HelperCall(PartialStructuredIntentValue<HelperCall>),
    HelperResult(PartialStructuredIntentValue<HelperResult>),
    Loud(PartialStructuredIntentValue<LoudIntent>),
}

impl AuwgentIntentName {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
        "response_text" => Some(AuwgentIntentName::ResponseText),
        "response_schema" => Some(AuwgentIntentName::ResponseSchema),
        "error" => Some(AuwgentIntentName::Error),
        "tool_call" => Some(AuwgentIntentName::ToolCall),
        "tool_result" => Some(AuwgentIntentName::ToolResult),
        "tool_error" => Some(AuwgentIntentName::ToolError),
        "tool_skipped" => Some(AuwgentIntentName::ToolSkipped),
        "workflow_call" => Some(AuwgentIntentName::WorkflowCall),
        "workflow_result" => Some(AuwgentIntentName::WorkflowResult),
        "helper_call" => Some(AuwgentIntentName::HelperCall),
        "helper_result" => Some(AuwgentIntentName::HelperResult),
        "Loud" => Some(AuwgentIntentName::Loud),
            _ => None,
        }
    }
}

impl AuwgentIntent {
    pub fn decode(name: AuwgentIntentName, value: JsonValue) -> Option<Self> {
        match name {
        AuwgentIntentName::ResponseText => serde_json::from_value(value).ok().map(AuwgentIntent::ResponseText),
        AuwgentIntentName::ResponseSchema => serde_json::from_value(value).ok().map(AuwgentIntent::ResponseSchema),
        AuwgentIntentName::Error => serde_json::from_value(value).ok().map(AuwgentIntent::Error),
        AuwgentIntentName::ToolCall => serde_json::from_value(value).ok().map(AuwgentIntent::ToolCall),
        AuwgentIntentName::ToolResult => serde_json::from_value(value).ok().map(AuwgentIntent::ToolResult),
        AuwgentIntentName::ToolError => serde_json::from_value(value).ok().map(AuwgentIntent::ToolError),
        AuwgentIntentName::ToolSkipped => serde_json::from_value(value).ok().map(AuwgentIntent::ToolSkipped),
        AuwgentIntentName::WorkflowCall => serde_json::from_value(value).ok().map(AuwgentIntent::WorkflowCall),
        AuwgentIntentName::WorkflowResult => serde_json::from_value(value).ok().map(AuwgentIntent::WorkflowResult),
        AuwgentIntentName::HelperCall => serde_json::from_value(value).ok().map(AuwgentIntent::HelperCall),
        AuwgentIntentName::HelperResult => serde_json::from_value(value).ok().map(AuwgentIntent::HelperResult),
        AuwgentIntentName::Loud => serde_json::from_value(value).ok().map(AuwgentIntent::Loud),
        }
    }
}

impl AuwgentIntentPartial {
    pub fn decode(name: AuwgentIntentName, value: JsonValue) -> Option<Self> {
        match name {
        AuwgentIntentName::ResponseText => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ResponseText),
        AuwgentIntentName::ResponseSchema => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ResponseSchema),
        AuwgentIntentName::Error => serde_json::from_value(value).ok().map(AuwgentIntentPartial::Error),
        AuwgentIntentName::ToolCall => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ToolCall),
        AuwgentIntentName::ToolResult => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ToolResult),
        AuwgentIntentName::ToolError => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ToolError),
        AuwgentIntentName::ToolSkipped => serde_json::from_value(value).ok().map(AuwgentIntentPartial::ToolSkipped),
        AuwgentIntentName::WorkflowCall => serde_json::from_value(value).ok().map(AuwgentIntentPartial::WorkflowCall),
        AuwgentIntentName::WorkflowResult => serde_json::from_value(value).ok().map(AuwgentIntentPartial::WorkflowResult),
        AuwgentIntentName::HelperCall => serde_json::from_value(value).ok().map(AuwgentIntentPartial::HelperCall),
        AuwgentIntentName::HelperResult => serde_json::from_value(value).ok().map(AuwgentIntentPartial::HelperResult),
        AuwgentIntentName::Loud => serde_json::from_value(value).ok().map(AuwgentIntentPartial::Loud),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Intents {
    inner: AuwgentIntent,
}

impl Intents {
    pub fn new(inner: AuwgentIntent) -> Self {
        Self { inner }
    }

    pub fn raw(&self) -> &AuwgentIntent {
        &self.inner
    }

    pub fn name(&self) -> &'static str {
        match &self.inner {
            AuwgentIntent::ResponseText(_) => "response_text",
            AuwgentIntent::ResponseSchema(_) => "response_schema",
            AuwgentIntent::Error(_) => "error",
            AuwgentIntent::ToolCall(..) => "tool_call",
            AuwgentIntent::ToolResult(..) => "tool_result",
            AuwgentIntent::ToolError(..) => "tool_error",
            AuwgentIntent::ToolSkipped(..) => "tool_skipped",
            AuwgentIntent::WorkflowCall(..) => "workflow_call",
            AuwgentIntent::WorkflowResult(..) => "workflow_result",
            AuwgentIntent::HelperCall(..) => "helper_call",
            AuwgentIntent::HelperResult(..) => "helper_result",
            AuwgentIntent::Loud(..) => "Loud",
        }
    }

    pub fn text(&self) -> &str {
        match &self.inner {
            AuwgentIntent::ResponseText(intent) => &intent.text,
            _ => panic!("intent does not contain text"),
        }
    }

    pub fn message(&self) -> &str {
        match &self.inner {
            AuwgentIntent::Error(intent) => &intent.message,
            AuwgentIntent::ToolError(intent) => &intent.message,
            _ => panic!("intent does not contain a message"),
        }
    }

    pub fn response<T>(&self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let value = match &self.inner {
            AuwgentIntent::ResponseSchema(intent) => intent.response_value(),
            _ => panic!("intent does not contain a response"),
        };
        serde_json::from_value(value).expect("response should deserialize")
    }

    pub fn value<T>(&self) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let value = match &self.inner {
            AuwgentIntent::ToolCall(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::ToolResult(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::ToolError(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::ToolSkipped(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::WorkflowCall(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::WorkflowResult(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::HelperCall(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::HelperResult(intent) => serde_json::to_value(intent.clone()),
            AuwgentIntent::Loud(intent) => serde_json::to_value(intent.clone()),
            _ => panic!("intent does not contain a typed value"),
        }.expect("intent value should serialize");
        serde_json::from_value(value).expect("intent value should deserialize")
    }
}

#[derive(Debug, Clone)]
pub struct ToolCalls {
    pub kind: ToolCall,
}

#[derive(Debug, Clone)]
pub struct ToolResults {
    pub kind: ToolResult,
}

#[derive(Debug, Clone)]
pub struct ToolErrors {
    pub kind: ToolError,
}

#[derive(Debug, Clone)]
pub struct ToolSkippeds {
    pub kind: ToolSkipped,
}

#[derive(Debug, Clone)]
pub struct WorkflowCalls {
    pub kind: WorkflowCall,
}

#[derive(Debug, Clone)]
pub struct WorkflowResults {
    pub kind: WorkflowResult,
}

#[derive(Debug, Clone)]
pub struct HelperCalls {
    pub kind: HelperCall,
}

#[derive(Debug, Clone)]
pub struct HelperResults {
    pub kind: HelperResult,
}

pub trait AuwgentIntentHandler: Send + Sync + 'static {
    fn response_text(&self, _value: &ResponseText, _agent: &str) {}
    fn response_schema(&self, _value: &ResponseSchema, _agent: &str) {}
    fn tool_call(&self, _value: &ToolCalls, _agent: &str) {}
    fn tool_result(&self, _value: &ToolResults, _agent: &str) {}
    fn tool_error(&self, _value: &ToolErrors, _agent: &str) {}
    fn tool_skipped(&self, _value: &ToolSkippeds, _agent: &str) {}
    fn workflow_call(&self, _value: &WorkflowCalls, _agent: &str) {}
    fn workflow_result(&self, _value: &WorkflowResults, _agent: &str) {}
    fn helper_call(&self, _value: &HelperCalls, _agent: &str) {}
    fn helper_result(&self, _value: &HelperResults, _agent: &str) {}
    fn loud(&self, _value: &LoudIntent, _agent: &str) {}
    fn error(&self, _value: &ErrorIntent, _agent: &str) {}
    fn any(&self, _intent: &Intents, _agent: &str) {}

    fn dispatch(&self, intent: &Intents, agent_name: &str) -> Option<IntentControl> {
        self.any(intent, agent_name);
        match intent.raw() {
            AuwgentIntent::ResponseText(value) => self.response_text(value, agent_name),
            AuwgentIntent::ResponseSchema(value) => self.response_schema(value, agent_name),
            AuwgentIntent::Error(value) => self.error(value, agent_name),
            AuwgentIntent::ToolCall(value) => self.tool_call(&ToolCalls { kind: value.clone() }, agent_name),
            AuwgentIntent::ToolResult(value) => self.tool_result(&ToolResults { kind: value.clone() }, agent_name),
            AuwgentIntent::ToolError(value) => self.tool_error(&ToolErrors { kind: value.clone() }, agent_name),
            AuwgentIntent::ToolSkipped(value) => self.tool_skipped(&ToolSkippeds { kind: value.clone() }, agent_name),
            AuwgentIntent::WorkflowCall(value) => self.workflow_call(&WorkflowCalls { kind: value.clone() }, agent_name),
            AuwgentIntent::WorkflowResult(value) => self.workflow_result(&WorkflowResults { kind: value.clone() }, agent_name),
            AuwgentIntent::HelperCall(value) => self.helper_call(&HelperCalls { kind: value.clone() }, agent_name),
            AuwgentIntent::HelperResult(value) => self.helper_result(&HelperResults { kind: value.clone() }, agent_name),
            AuwgentIntent::Loud(value) => self.loud(value, agent_name),
        }
        None
    }
}

pub trait AuwgentBasePartialIntentHandler {
    fn on_intent_partial(&self, intent: AuwgentIntentPartial, agent_name: &str) { let _ = (intent, agent_name); }

    fn dispatch_partial(&self, intent: AuwgentIntentPartial, agent_name: &str) {
        self.on_intent_partial(intent, agent_name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuwgentApiKeys {
    pub groq_api_key: Option<String>,
}

impl From<AuwgentApiKeys> for sdk::AuwgentApiKeys {
    fn from(value: AuwgentApiKeys) -> Self {
        sdk::AuwgentApiKeys {
            groq_api_key: value.groq_api_key,
            ..sdk::AuwgentApiKeys::default()
        }
    }
}

#[async_trait]
pub trait AuwgentMiddleware: Send + Sync + 'static {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn target(&self) -> Option<Vec<String>> {
        None
    }

    async fn on_run_start(&self, session: Session, _ctx: &mut Context) -> Session {
        session
    }

    async fn on_llm_start(&self, prompt: String, _ctx: &mut Context) -> String {
        prompt
    }

    async fn on_intent(&self, _intent: &Intents, _ctx: &mut Context) -> Option<IntentControl> {
        None
    }

    async fn on_intent_partial(&self, _intent: &AuwgentIntentPartial, _ctx: &mut Context) {}

    async fn on_llm_end(&self, _response: &JsonValue, _ctx: &mut Context) {}

    async fn on_run_complete(&self, _session: &Session, _ctx: &mut Context) {}

    async fn on_error(&self, _error: &JsonValue, _session: Option<&Session>, _ctx: &mut Context) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct AuwgentMiddlewareRegistry(pub sdk::MiddlewareRegistry);

struct MiddlewareAdapter<T>(T);

#[async_trait]
impl<T> sdk::Middleware for MiddlewareAdapter<T>
where
    T: AuwgentMiddleware,
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
        let Some(intent_name) = AuwgentIntentName::parse(name) else {
            return Ok(None);
        };
        let Some(intent) = AuwgentIntent::decode(intent_name, value.clone()) else {
            return Ok(None);
        };
        let intent = Intents::new(intent);
        Ok(self.0.on_intent(&intent, ctx).await)
    }

    async fn on_intent_partial(
        &self,
        name: &str,
        value: &JsonValue,
        ctx: &mut sdk::MiddlewareContext,
    ) -> sdk::AuwgentResult<()> {
        if let Some(intent_name) = AuwgentIntentName::parse(name)
            && let Some(intent) = AuwgentIntentPartial::decode(intent_name, value.clone())
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

impl<T> From<T> for AuwgentMiddlewareRegistry
where
    T: AuwgentMiddleware,
{
    fn from(value: T) -> Self {
        Self(Arc::new(MiddlewareAdapter(value)))
    }
}

impl From<sdk::MiddlewareRegistry> for AuwgentMiddlewareRegistry {
    fn from(value: sdk::MiddlewareRegistry) -> Self {
        Self(value)
    }
}

#[derive(Clone)]
pub struct AuwgentConfig<TTools = AuwgentToolsRegistry, TMiddleware = AuwgentMiddlewareRegistry> {
    pub tools: TTools,
    pub middleware: Vec<TMiddleware>,
    pub context: AuwgentContext,
    pub api_keys: AuwgentApiKeys,
}

pub struct AuwgentAgent {
    inner: sdk::TypedAuwgent<AuwgentToolsRegistry>,
}

impl std::ops::Deref for AuwgentAgent {
    type Target = sdk::TypedAuwgent<AuwgentToolsRegistry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AuwgentAgent {
    pub fn on_intent<H>(&self, handler: H)
    where
        H: AuwgentIntentHandler,
    {
        let handler = Arc::new(handler);
        self.inner.on_decoded_intent(AuwgentIntentName::parse, AuwgentIntent::decode, move |intent, agent_name| {
            let intent = Intents::new(intent);
            handler.dispatch(&intent, agent_name)
        });
    }

    pub fn on_intent_raw<F>(&self, handler: F)
    where
        F: FnMut(AuwgentIntent, &str) -> Option<IntentControl> + Send + 'static,
    {
        self.inner.on_decoded_intent(AuwgentIntentName::parse, AuwgentIntent::decode, handler);
    }

    pub fn on_intent_handler<H>(&self, handler: H)
    where
        H: AuwgentIntentHandler,
    {
        self.on_intent(handler);
    }

    pub fn on_intent_partial<F>(&self, handler: F)
    where
        F: FnMut(AuwgentIntentPartial, &str) + Send + 'static,
    {
        self.inner.on_decoded_intent_partial(AuwgentIntentName::parse, AuwgentIntentPartial::decode, handler);
    }

    pub fn on_intent_partial_handler<H>(&self, handler: H)
    where
        H: AuwgentBasePartialIntentHandler + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        self.on_intent_partial(move |intent, agent_name| {
            handler.dispatch_partial(intent, agent_name);
        });
    }

    pub async fn run(&self, input: Option<AuwgentInput>) -> sdk::AuwgentResult<SessionState> {
        let input = input.map(serde_json::to_value).transpose().map_err(|e| e.to_string())?;
        self.inner.run(input).await
    }
}

pub fn create_simpletool<TTools, TMiddleware>(config: AuwgentConfig<TTools, TMiddleware>) -> sdk::AuwgentResult<AuwgentAgent>
where
    TTools: Into<AuwgentToolsRegistry>,
    TMiddleware: Into<AuwgentMiddlewareRegistry>,
{
    let ir = sdk::parse_ir(include_str!("./main.agent.json"))?;
    let middleware = config.middleware.into_iter().map(|item| {
        let registry: AuwgentMiddlewareRegistry = item.into();
        registry.0
    }).collect();
    let sdk_config = sdk::AuwgentConfig {
        tools: config.tools.into(),
        middleware,
        context: Some(serde_json::to_value(config.context).map_err(|e| e.to_string())?),
        api_keys: config.api_keys.into(),
    };
    let inner = sdk::create_auwgent(ir, sdk_config)?;
    Ok(AuwgentAgent { inner })
}

pub fn auwgent<TTools, TMiddleware>(config: AuwgentConfig<TTools, TMiddleware>) -> sdk::AuwgentResult<AuwgentAgent>
where
    TTools: Into<AuwgentToolsRegistry>,
    TMiddleware: Into<AuwgentMiddlewareRegistry>,
{
    create_simpletool(config)
}
