// Auto-generated Rust bindings for SimpleTool
// Do not edit manually
use auwgent_sdk_rust as sdk;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
pub type IntentControl = sdk::IntentControl;
pub type SessionState = sdk::SessionState;
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

pub type SimpleToolOutput = JsonValue;

pub type SimpleToolGetLocationToolResultValue = String;

pub trait SimpleToolTools: Send + Sync {
    fn get_location(&self, args: NoArgs) -> SimpleToolGetLocationToolResultValue;
}

#[derive(Clone)]
pub struct SimpleToolToolsRegistry(pub Arc<dyn SimpleToolTools>);

impl SimpleToolToolsRegistry {
    pub fn new<T>(tools: T) -> Self
    where
        T: SimpleToolTools + 'static,
    {
        Self(Arc::new(tools))
    }

    pub fn from_arc(tools: Arc<dyn SimpleToolTools>) -> Self {
        Self(tools)
    }
}

impl sdk::ToolRegistrar for SimpleToolToolsRegistry {
    fn register_tools(&self, native: &sdk::AuwgentNative) -> sdk::AuwgentResult<()> {
        let tool_impl = Arc::clone(&self.0);
        let tools = Arc::clone(&tool_impl);
        native.register_tool_fn("get_location", move |args| {
            let tools = Arc::clone(&tools);
            Box::pin(async move {
                let parsed: NoArgs = serde_json::from_value(args).map_err(|e| e.to_string())?;
                let result = tools.get_location(parsed);
                serde_json::to_value(result).map_err(|e| e.to_string())
            })
        });
        Ok(())
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SimpleToolToolSkippedIntent {
    #[serde(rename = "get_location")]
    GetLocation,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimpleToolToolErrorIntent {
    pub tool: String,
    pub message: String,
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
}

pub fn parse_intent_name(name: &str) -> Option<SimpleToolIntentName> {
    match name {
        "response_text" => Some(SimpleToolIntentName::ResponseText),
        "response_schema" => Some(SimpleToolIntentName::ResponseSchema),
        "error" => Some(SimpleToolIntentName::Error),
        "tool_call" => Some(SimpleToolIntentName::ToolCall),
        "tool_result" => Some(SimpleToolIntentName::ToolResult),
        "tool_error" => Some(SimpleToolIntentName::ToolError),
        "tool_skipped" => Some(SimpleToolIntentName::ToolSkipped),
        _ => None,
    }
}

pub fn decode_intent(name: SimpleToolIntentName, value: JsonValue) -> Option<SimpleToolIntent> {
    match name {
        SimpleToolIntentName::ResponseText => serde_json::from_value(value).ok().map(SimpleToolIntent::ResponseText),
        SimpleToolIntentName::ResponseSchema => serde_json::from_value(value).ok().map(SimpleToolIntent::ResponseSchema),
        SimpleToolIntentName::Error => serde_json::from_value(value).ok().map(SimpleToolIntent::Error),
        SimpleToolIntentName::ToolCall => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolCall),
        SimpleToolIntentName::ToolResult => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolResult),
        SimpleToolIntentName::ToolError => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolError),
        SimpleToolIntentName::ToolSkipped => serde_json::from_value(value).ok().map(SimpleToolIntent::ToolSkipped),
    }
}

pub fn decode_intent_partial(name: SimpleToolIntentName, value: JsonValue) -> Option<SimpleToolIntentPartial> {
    match name {
        SimpleToolIntentName::ResponseText => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ResponseText),
        SimpleToolIntentName::ResponseSchema => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ResponseSchema),
        SimpleToolIntentName::Error => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::Error),
        SimpleToolIntentName::ToolCall => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolCall),
        SimpleToolIntentName::ToolResult => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolResult),
        SimpleToolIntentName::ToolError => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolError),
        SimpleToolIntentName::ToolSkipped => serde_json::from_value(value).ok().map(SimpleToolIntentPartial::ToolSkipped),
    }
}

pub trait SimpleToolBaseIntentHandler {
    fn on_intent(&mut self, intent: SimpleToolIntent, agent_name: &str) -> Option<IntentControl> { let _ = (intent, agent_name); None }
}

pub trait SimpleToolBasePartialIntentHandler {
    fn on_intent_partial(&mut self, intent: SimpleToolIntentPartial, agent_name: &str) { let _ = (intent, agent_name); }
}

pub fn dispatch_intent<H: SimpleToolBaseIntentHandler>(handler: &mut H, intent: SimpleToolIntent, agent_name: &str) -> Option<IntentControl> {
    handler.on_intent(intent, agent_name)
}

pub fn dispatch_partial_intent<H: SimpleToolBasePartialIntentHandler>(handler: &mut H, intent: SimpleToolIntentPartial, agent_name: &str) {
    handler.on_intent_partial(intent, agent_name)
}

#[derive(Debug, Clone, Default)]
pub struct SimpleToolApiKeys {
    pub groq_api_key: Option<String>,
}

impl From<SimpleToolApiKeys> for sdk::AuwgentApiKeys {
    fn from(value: SimpleToolApiKeys) -> Self {
        let mut custom_api_keys = std::collections::HashMap::new();

        Self {
            openai_api_key: None,
            gemini_api_key: None,
            groq_api_key: value.groq_api_key,
            custom_api_keys,
        }
    }
}

pub use sdk::MiddlewareContext;

pub trait SimpleToolMiddleware: sdk::Middleware {}

impl<T> SimpleToolMiddleware for T where T: sdk::Middleware + ?Sized {}

pub type SimpleToolMiddlewareRegistry = sdk::MiddlewareRegistry;


#[derive(Clone)]
pub struct SimpleToolConfig {
    pub tools: SimpleToolToolsRegistry,
    pub middleware: Vec<SimpleToolMiddlewareRegistry>,
    pub api_keys: SimpleToolApiKeys,
}

pub struct SimpleToolAgent {
    inner: sdk::TypedAuwgent<SimpleToolToolsRegistry>,
}

impl SimpleToolAgent {
    pub fn on_intent<F>(&self, handler: F)
    where
        F: FnMut(SimpleToolIntent, &str) -> Option<IntentControl> + Send + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));
        self.inner.on_intent(move |name, value, agent_name| {
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let intent_name = parse_intent_name(&name)?;
                let intent = decode_intent(intent_name, value)?;
                let mut handler = handler.lock().ok()?;
                (*handler)(intent, &agent_name)
            })
        });
    }

    pub fn on_intent_partial<F>(&self, handler: F)
    where
        F: FnMut(SimpleToolIntentPartial, &str) + Send + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));
        self.inner.on_intent_partial(move |name, value, agent_name| {
            if let Some(intent_name) = parse_intent_name(&name)
                && let Some(intent) = decode_intent_partial(intent_name, value)
                && let Ok(mut handler) = handler.lock()
            {
                (*handler)(intent, &agent_name);
            }
        });
    }

    pub fn on_intent_handler<H>(&self, handler: H)
    where
        H: SimpleToolBaseIntentHandler + Send + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));
        self.on_intent(move |intent, agent_name| {
            let mut handler = handler.lock().ok()?;
            dispatch_intent(&mut *handler, intent, agent_name)
        });
    }

    pub fn on_intent_partial_handler<H>(&self, handler: H)
    where
        H: SimpleToolBasePartialIntentHandler + Send + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));
        self.on_intent_partial(move |intent, agent_name| {
            if let Ok(mut handler) = handler.lock() {
                dispatch_partial_intent(&mut *handler, intent, agent_name);
            }
        });
    }

    pub async fn run(&self, input: Option<SimpleToolInput>) -> sdk::AuwgentResult<SessionState> {
        let input = input.map(serde_json::to_value).transpose().map_err(|e| e.to_string())?;
        self.inner.run(input).await
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> sdk::AuwgentResult<String> {
        self.inner.generate_prompt(helper_name)
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.inner.get_tool_names()
    }

    pub fn get_tool_schemas(&self) -> sdk::AuwgentResult<JsonValue> {
        self.inner.get_tool_schemas()
    }

    pub fn write_chunk(&self, chunk: String) {
        self.inner.raw().write_chunk(chunk);
    }

    pub fn end_stream(&self) -> sdk::AuwgentResult<JsonValue> {
        self.inner.raw().end_stream()
    }

    pub async fn process_intents(&self) -> sdk::AuwgentResult<JsonValue> {
        self.inner.raw().process_intents().await
    }

    pub fn export_session(&self) -> sdk::AuwgentResult<SessionState> {
        self.inner.export_session()
    }

    pub fn import_session(&self, session: &SessionState) -> sdk::AuwgentResult<()> {
        self.inner.import_session(session)
    }

    pub fn clear_session(&self) {
        self.inner.clear_session();
    }

    pub fn get_metadata(&self) -> sdk::AuwgentResult<sdk::RunMetadata> {
        self.inner.get_metadata()
    }

    pub fn raw(&self) -> &sdk::TypedAuwgent<SimpleToolToolsRegistry> {
        &self.inner
    }
}

pub fn create_simpletool(config: SimpleToolConfig) -> sdk::AuwgentResult<SimpleToolAgent> {
    let ir = sdk::parse_ir(include_str!("./main.agent.json"))?;
    let sdk_config = sdk::AuwgentConfig {
        tools: config.tools,
        middleware: config.middleware,
        context: None,
        api_keys: config.api_keys.into(),
    };
    let inner = sdk::create_auwgent(ir, sdk_config)?;
    Ok(SimpleToolAgent { inner })
}

pub fn auwgent(config: SimpleToolConfig) -> sdk::AuwgentResult<SimpleToolAgent> {
    create_simpletool(config)
}

pub use SimpleToolAgent as AuwgentAgent;
pub use SimpleToolConfig as AuwgentConfig;
pub use SimpleToolIntent as AuwgentIntent;
pub use SimpleToolIntentPartial as AuwgentIntentPartial;
pub use SimpleToolIntentName as AuwgentIntentName;
pub use SimpleToolBaseIntentHandler as AuwgentBaseIntentHandler;
pub use SimpleToolBasePartialIntentHandler as AuwgentBasePartialIntentHandler;
pub use SimpleToolMiddleware as AuwgentMiddleware;
pub use SimpleToolMiddlewareRegistry as AuwgentMiddlewareRegistry;
pub use SimpleToolTools as AuwgentTools;
pub use SimpleToolApiKeys as AuwgentApiKeys;