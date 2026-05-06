use async_trait::async_trait;
pub use futures_util::future::BoxFuture;
use futures_util::{Stream, stream};
use ir_runtime::runtime::bridge::EngineBridge;
use ir_runtime::runtime::drivers::{ModelDriver, ModelEvent};
use ir_runtime::runtime::engine::{
    AsyncIntentCallback, AsyncMiddlewareEventCallback, AsyncSessionPreloadCallback,
    SessionSaveCallback, ToolImplementation,
};
use ir_runtime::runtime::middleware::parse_intent_control_response;
use ir_runtime::{AgentIR, ModelConfigEntry, ModelProvider};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub type AuwgentResult<T> = Result<T, String>;
pub type MiddlewareRegistry = Arc<dyn Middleware>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuwgentInputPart {
    Text(AuwgentTextPart),
    Image(AuwgentImagePart),
    File(AuwgentFilePart),
    Audio(AuwgentAudioPart),
    Video(AuwgentVideoPart),
}

#[derive(Debug, Clone, Serialize)]
pub struct AuwgentTextPart {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuwgentMediaSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuwgentImagePart {
    #[serde(flatten)]
    pub source: AuwgentMediaSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuwgentFilePart {
    #[serde(flatten)]
    pub source: AuwgentMediaSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuwgentAudioPart {
    #[serde(flatten)]
    pub source: AuwgentMediaSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuwgentVideoPart {
    #[serde(flatten)]
    pub source: AuwgentMediaSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampled_frames: Option<Vec<AuwgentImagePart>>,
}

#[derive(Debug, Clone, Default)]
pub struct AuwgentApiKeys {
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub groq_api_key: Option<String>,
    pub custom_api_keys: HashMap<String, String>,
}

pub trait ToolRegistrar: Send + Sync {
    fn tool_names(&self) -> &'static [&'static str];

    fn invoke_tool(
        &self,
        name: &'static str,
        args: Value,
    ) -> BoxFuture<'static, AuwgentResult<Value>>;
}

impl ToolRegistrar for () {
    fn tool_names(&self) -> &'static [&'static str] {
        &[]
    }

    fn invoke_tool(
        &self,
        _name: &'static str,
        _args: Value,
    ) -> BoxFuture<'static, AuwgentResult<Value>> {
        Box::pin(async { Err("No tools registered".to_string()) })
    }
}

pub trait IntoMiddlewareRegistry {
    fn into_middleware_registry(self) -> MiddlewareRegistry;
}

impl IntoMiddlewareRegistry for MiddlewareRegistry {
    fn into_middleware_registry(self) -> MiddlewareRegistry {
        self
    }
}

#[derive(Clone)]
pub struct MiddlewareContext {
    pub active_agent: String,
    pub stack: Vec<String>,
    pub root_agent: String,
    pub raw_block: Option<String>,
    pub system_prompt: Option<String>,
    pub data: Map<String, Value>,
    native: AuwgentNative,
}

impl MiddlewareContext {
    pub fn set_context(&mut self, data: Value) {
        if let Value::Object(values) = &data {
            for (key, value) in values {
                self.data.insert(key.clone(), value.clone());
            }
        } else {
            self.data.insert("dynamic_context".to_string(), data.clone());
        }
        self.native.set_context(data);
    }

    pub async fn embed(&self, text: String) -> AuwgentResult<Vec<f32>> {
        self.native.embed(text).await
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> AuwgentResult<Vec<Vec<f32>>> {
        self.native.embed_batch(texts).await
    }
}

#[async_trait]
pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn target(&self) -> Option<Vec<String>> {
        None
    }

    async fn on_run_start(
        &self,
        session: SessionState,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<SessionState> {
        Ok(session)
    }

    async fn on_llm_start(
        &self,
        _prompt: String,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<Option<String>> {
        Ok(None)
    }

    async fn on_intent(
        &self,
        _name: &str,
        _value: &Value,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<Option<IntentControl>> {
        Ok(None)
    }

    async fn on_intent_partial(
        &self,
        _name: &str,
        _value: &Value,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<()> {
        Ok(())
    }

    async fn on_llm_end(
        &self,
        _response: &Value,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<()> {
        Ok(())
    }

    async fn on_run_complete(
        &self,
        _session: &SessionState,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<()> {
        Ok(())
    }

    async fn on_error(
        &self,
        _error: &Value,
        _session: Option<&SessionState>,
        _ctx: &mut MiddlewareContext,
    ) -> AuwgentResult<bool> {
        Ok(false)
    }
}

impl<T> IntoMiddlewareRegistry for T
where
    T: Middleware + 'static,
{
    fn into_middleware_registry(self) -> MiddlewareRegistry {
        Arc::new(self)
    }
}

#[derive(Clone)]
pub struct AuwgentNative {
    bridge: EngineBridge,
}

impl AuwgentNative {
    pub fn new(ir_json: String) -> AuwgentResult<Self> {
        let bridge = EngineBridge::new(ir_json)?;
        Ok(Self { bridge })
    }

    pub fn from_ir(ir: &AgentIR) -> AuwgentResult<Self> {
        let ir_json = serde_json::to_string(ir).map_err(|e| e.to_string())?;
        Self::new(ir_json)
    }

    pub fn set_gemini_driver(&self, api_key: String) {
        self.bridge.set_gemini_driver(api_key);
    }

    pub fn set_groq_driver(&self, api_key: String) {
        self.bridge.set_groq_driver(api_key);
    }

    pub fn set_openai_driver(&self, api_key: String, base_url: Option<String>) {
        self.bridge.set_openai_driver(api_key, base_url);
    }

    pub fn set_custom_driver(&self, id: String, api_key: String, base_url: String) {
        self.bridge.set_custom_driver(id, api_key, base_url);
    }

    pub fn set_deterministic_driver(&self, provider_type: String, outputs: Vec<String>) {
        self.bridge.register_driver(
            provider_type,
            Arc::new(DeterministicDriver::new(outputs)) as Arc<dyn ModelDriver>,
        );
    }

    pub fn set_context(&self, context: Value) {
        self.bridge.set_context(context);
    }

    pub fn get_metadata(&self) -> AuwgentResult<RunMetadata> {
        serde_json::from_str(&self.bridge.get_metadata()?).map_err(|e| e.to_string())
    }

    pub fn export_session(&self) -> AuwgentResult<SessionState> {
        serde_json::from_str(&self.bridge.export_session()?).map_err(|e| e.to_string())
    }

    pub fn import_session(&self, session: &SessionState) -> AuwgentResult<()> {
        let json = serde_json::to_string(session).map_err(|e| e.to_string())?;
        self.bridge.import_session(json)
    }

    pub fn clear_session(&self) {
        self.bridge.clear_session();
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> AuwgentResult<String> {
        self.bridge.generate_prompt(helper_name)
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.bridge.get_tool_names()
    }

    pub fn get_tool_schemas(&self) -> AuwgentResult<Value> {
        serde_json::from_str(&self.bridge.get_tool_schemas()?).map_err(|e| e.to_string())
    }

    pub fn write_chunk(&self, chunk: String) {
        self.bridge.write_chunk(chunk);
    }

    pub fn end_stream(&self) -> AuwgentResult<Value> {
        serde_json::from_str(&self.bridge.end_stream()?).map_err(|e| e.to_string())
    }

    pub fn clear_listeners(&self) {
        self.bridge.clear_listeners();
    }

    pub async fn run(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<SessionState> {
        let json = self.bridge.run_async(input, initial_stack).await?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub async fn begin_run(
        &self,
        input: Option<Value>,
        initial_stack: Option<Vec<String>>,
    ) -> AuwgentResult<SessionState> {
        let json = self.bridge.begin_run_async(input, initial_stack).await?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub async fn apply_llm_start(&self, prompt: String) -> AuwgentResult<String> {
        let json = self.bridge.apply_llm_start_async(prompt).await?;
        let value: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        value
            .get("prompt")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| "llm_start response did not include prompt".to_string())
    }

    pub async fn apply_llm_end(&self, response: Value) -> AuwgentResult<()> {
        self.bridge.apply_llm_end_async(response).await?;
        Ok(())
    }

    pub async fn complete_run(&self) -> AuwgentResult<SessionState> {
        let json = self.bridge.complete_run_async().await?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub async fn apply_error(&self, error: Value, include_session: bool) -> AuwgentResult<bool> {
        let json = self
            .bridge
            .apply_error_async(error, include_session)
            .await?;
        let value: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(value
            .get("swallowed")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn process_intents(&self) -> AuwgentResult<Value> {
        let json = self.bridge.process_intents_async().await?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub async fn embed(&self, text: String) -> AuwgentResult<Vec<f32>> {
        self.bridge.embed(text).await
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> AuwgentResult<Vec<Vec<f32>>> {
        self.bridge.embed_batch(texts).await
    }

    pub fn register_tool_fn<F>(&self, name: impl Into<String>, f: F)
    where
        F: Fn(Value) -> BoxFuture<'static, AuwgentResult<Value>> + Send + Sync + 'static,
    {
        let callback: ToolImplementation = Arc::new(move |args| {
            let future = f(args);
            Box::pin(async move { future.await })
        });
        self.bridge.register_tool(&name.into(), callback);
    }

    pub fn on_intent(&self, handler: AsyncIntentCallback) {
        self.bridge.on_intent(handler);
    }

    pub fn on_intent_partial(&self, handler: Arc<dyn Fn(String, Value, String) + Send + Sync>) {
        self.bridge.on_intent_partial(handler);
    }

    pub fn on_sub_engine_start(&self, handler: AsyncSessionPreloadCallback) {
        self.bridge.on_sub_engine_start(handler);
    }

    pub fn on_sub_engine_complete(&self, handler: SessionSaveCallback) {
        self.bridge.on_sub_engine_complete(handler);
    }

    pub fn on_middleware_event(&self, handler: AsyncMiddlewareEventCallback) {
        self.bridge.on_middleware_event(handler);
    }
}

struct DeterministicDriver {
    outputs: Arc<Mutex<VecDeque<String>>>,
}

impl DeterministicDriver {
    fn new(outputs: Vec<String>) -> Self {
        Self {
            outputs: Arc::new(Mutex::new(outputs.into())),
        }
    }
}

#[async_trait]
impl ModelDriver for DeterministicDriver {
    async fn stream_generate(
        &self,
        _model: &str,
        _messages: &[Message],
        _config: Option<Value>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ModelEvent, String>> + Send>>, String> {
        let output = self
            .outputs
            .lock()
            .map_err(|_| "deterministic driver output queue poisoned".to_string())?
            .pop_front()
            .unwrap_or_default();
        Ok(Box::pin(stream::iter(vec![Ok(ModelEvent::ContentChunk(
            output,
        ))])))
    }

    async fn embed(
        &self,
        _model: &str,
        _text: &str,
        _config: Option<Value>,
    ) -> Result<Vec<f32>, String> {
        Ok(vec![0.0])
    }

    async fn embed_batch(
        &self,
        _model: &str,
        texts: &[String],
        _config: Option<Value>,
    ) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|_| vec![0.0]).collect())
    }
}

impl std::fmt::Debug for AuwgentNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuwgentNative").finish_non_exhaustive()
    }
}

pub struct AuwgentConfig<Tools = (), MiddlewareItem = MiddlewareRegistry> {
    pub tools: Tools,
    pub middleware: Vec<MiddlewareItem>,
    pub context: Option<Value>,
    pub api_keys: AuwgentApiKeys,
}

impl<Tools: Default, MiddlewareItem> Default for AuwgentConfig<Tools, MiddlewareItem> {
    fn default() -> Self {
        Self {
            tools: Tools::default(),
            middleware: Vec::new(),
            context: None,
            api_keys: AuwgentApiKeys::default(),
        }
    }
}

pub struct TypedAuwgent<Tools = ()> {
    native: AuwgentNative,
    ir: AgentIR,
    middleware: Vec<MiddlewareRegistry>,
    shared_context: Arc<Mutex<Map<String, Value>>>,
    _tools: Tools,
}

impl<Tools> TypedAuwgent<Tools>
where
    Tools: ToolRegistrar + Clone + 'static,
{
    pub fn new<MiddlewareItem>(
        ir: AgentIR,
        config: AuwgentConfig<Tools, MiddlewareItem>,
    ) -> AuwgentResult<Self>
    where
        MiddlewareItem: IntoMiddlewareRegistry,
    {
        let native = AuwgentNative::from_ir(&ir)?;

        if let Some(context) = config.context.clone() {
            native.set_context(context);
        }

        if let Some(key) = config.api_keys.gemini_api_key.clone() {
            native.set_gemini_driver(key);
        }
        if let Some(key) = config.api_keys.openai_api_key.clone() {
            native.set_openai_driver(key, None);
        }
        if let Some(key) = config.api_keys.groq_api_key.clone() {
            native.set_groq_driver(key);
        }

        register_custom_drivers(&native, &ir, &config.api_keys);
        register_tool_registrar(&native, &config.tools);

        let middleware = config
            .middleware
            .into_iter()
            .map(IntoMiddlewareRegistry::into_middleware_registry)
            .collect::<Vec<_>>();

        let shared_context = Arc::new(Mutex::new(Map::new()));
        if !middleware.is_empty() {
            attach_middleware(
                native.clone(),
                middleware.clone(),
                Arc::clone(&shared_context),
            );
        }

        Ok(Self {
            native,
            ir,
            middleware,
            shared_context,
            _tools: config.tools,
        })
    }

    pub fn on_intent<F>(&self, handler: F)
    where
        F: Fn(String, Value, String) -> BoxFuture<'static, Option<IntentControl>>
            + Send
            + Sync
            + 'static,
    {
        let callback: AsyncIntentCallback =
            Arc::new(move |name, value, agent| handler(name, value, agent));
        self.native.on_intent(callback);
    }

    pub fn on_decoded_intent<Name, Intent, Parse, Decode, F>(
        &self,
        parse_name: Parse,
        decode: Decode,
        handler: F,
    ) where
        Name: Send + 'static,
        Intent: Send + 'static,
        Parse: Fn(&str) -> Option<Name> + Send + Sync + 'static,
        Decode: Fn(Name, Value) -> Option<Intent> + Send + Sync + 'static,
        F: FnMut(Intent, &str) -> Option<IntentControl> + Send + 'static,
    {
        let parse_name = Arc::new(parse_name);
        let decode = Arc::new(decode);
        let handler = Arc::new(Mutex::new(handler));

        self.on_intent(move |name, value, agent_name| {
            let parse_name = Arc::clone(&parse_name);
            let decode = Arc::clone(&decode);
            let handler = Arc::clone(&handler);
            Box::pin(async move {
                let intent_name = parse_name(&name)?;
                let intent = decode(intent_name, value)?;
                let mut handler = handler.lock().ok()?;
                (*handler)(intent, &agent_name)
            })
        });
    }

    pub fn on_intent_partial<F>(&self, handler: F)
    where
        F: Fn(String, Value, String) + Send + Sync + 'static,
    {
        self.native.on_intent_partial(Arc::new(handler));
    }

    pub fn on_decoded_intent_partial<Name, Intent, Parse, Decode, F>(
        &self,
        parse_name: Parse,
        decode: Decode,
        handler: F,
    ) where
        Name: Send + 'static,
        Intent: Send + 'static,
        Parse: Fn(&str) -> Option<Name> + Send + Sync + 'static,
        Decode: Fn(Name, Value) -> Option<Intent> + Send + Sync + 'static,
        F: FnMut(Intent, &str) + Send + 'static,
    {
        let parse_name = Arc::new(parse_name);
        let decode = Arc::new(decode);
        let handler = Arc::new(Mutex::new(handler));

        self.on_intent_partial(move |name, value, agent_name| {
            let Some(intent_name) = parse_name(&name) else {
                return;
            };
            let Some(intent) = decode(intent_name, value) else {
                return;
            };
            if let Ok(mut handler) = handler.lock() {
                (*handler)(intent, &agent_name);
            }
        });
    }

    pub fn on_sub_engine_start<F>(&self, handler: F)
    where
        F: Fn(String, String) -> BoxFuture<'static, Option<String>> + Send + Sync + 'static,
    {
        let callback: AsyncSessionPreloadCallback =
            Arc::new(move |name, session| handler(name, session));
        self.native.on_sub_engine_start(callback);
    }

    pub fn on_sub_engine_complete<F>(&self, handler: F)
    where
        F: Fn(String, String) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        let callback: SessionSaveCallback = Arc::new(move |name, session| handler(name, session));
        self.native.on_sub_engine_complete(callback);
    }

    pub async fn run(&self, input: Option<Value>) -> AuwgentResult<SessionState> {
        self.native.run(input, None).await
    }

    pub fn get_metadata(&self) -> AuwgentResult<RunMetadata> {
        self.native.get_metadata()
    }

    pub fn export_session(&self) -> AuwgentResult<SessionState> {
        self.native.export_session()
    }

    pub fn import_session(&self, session: &SessionState) -> AuwgentResult<()> {
        self.native.import_session(session)
    }

    pub fn clear_session(&self) {
        self.native.clear_session();
    }

    pub fn generate_prompt(&self, helper_name: Option<String>) -> AuwgentResult<String> {
        self.native.generate_prompt(helper_name)
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.native.get_tool_names()
    }

    pub fn get_tool_schemas(&self) -> AuwgentResult<Value> {
        self.native.get_tool_schemas()
    }

    pub async fn embed(&self, text: String) -> AuwgentResult<Vec<f32>> {
        self.native.embed(text).await
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> AuwgentResult<Vec<Vec<f32>>> {
        self.native.embed_batch(texts).await
    }

    pub fn raw(&self) -> &AuwgentNative {
        &self.native
    }

    pub fn ir(&self) -> &AgentIR {
        &self.ir
    }

    pub fn middleware(&self) -> &[MiddlewareRegistry] {
        &self.middleware
    }

    pub fn shared_context(&self) -> Map<String, Value> {
        self.shared_context.lock().unwrap().clone()
    }
}

pub fn create_auwgent<Tools, MiddlewareItem>(
    ir: AgentIR,
    config: AuwgentConfig<Tools, MiddlewareItem>,
) -> AuwgentResult<TypedAuwgent<Tools>>
where
    Tools: ToolRegistrar + Clone + 'static,
    MiddlewareItem: IntoMiddlewareRegistry,
{
    TypedAuwgent::new(ir, config)
}

fn register_tool_registrar<Tools>(native: &AuwgentNative, tools: &Tools)
where
    Tools: ToolRegistrar + Clone + 'static,
{
    for &name in tools.tool_names() {
        let registrar = tools.clone();
        native.register_tool_fn(name, move |args| registrar.invoke_tool(name, args));
    }
}

pub fn parse_ir(json: &str) -> AuwgentResult<AgentIR> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

fn register_custom_drivers(native: &AuwgentNative, ir: &AgentIR, api_keys: &AuwgentApiKeys) {
    for entry in &ir.model_config {
        register_custom_drivers_from_entry(native, entry, api_keys);
    }
    for helper in &ir.helpers {
        for entry in &helper.model_config {
            register_custom_drivers_from_entry(native, entry, api_keys);
        }
    }
}

fn register_custom_drivers_from_entry(
    native: &AuwgentNative,
    entry: &ModelConfigEntry,
    api_keys: &AuwgentApiKeys,
) {
    if let Some(default_config) = &entry.default_config {
        let model = &default_config.model;
        maybe_register_custom_driver(native, model, api_keys);
    }
    if let Some(named_configs) = &entry.named_config {
        for named in named_configs {
            let model = &named.config.model;
            maybe_register_custom_driver(native, model, api_keys);
        }
    }
}

fn maybe_register_custom_driver(
    native: &AuwgentNative,
    model: &ModelProvider,
    api_keys: &AuwgentApiKeys,
) {
    if let ModelProvider::Custom { id, url, .. } = model
        && let Some(key) = api_keys.custom_api_keys.get(id)
    {
        native.set_custom_driver(id.clone(), key.clone(), url.clone());
    }
}

fn attach_middleware(
    native: AuwgentNative,
    middleware: Vec<MiddlewareRegistry>,
    shared_context: Arc<Mutex<Map<String, Value>>>,
) {
    let native_for_handler = native.clone();
    let handler: AsyncMiddlewareEventCallback = Arc::new(move |event_json: String| {
        let middleware = middleware.clone();
        let native = native_for_handler.clone();
        let shared_context = Arc::clone(&shared_context);
        Box::pin(async move {
            let event: Value = serde_json::from_str(&event_json).ok()?;
            handle_middleware_event(native, middleware, shared_context, event).await
        })
    });
    native.on_middleware_event(handler);
}

async fn handle_middleware_event(
    native: AuwgentNative,
    middleware: Vec<MiddlewareRegistry>,
    shared_context: Arc<Mutex<Map<String, Value>>>,
    event: Value,
) -> Option<String> {
    let mut ctx = build_context(&native, &shared_context, &event);
    let selected = select_middleware(&middleware, &ctx.active_agent);

    match event.get("type").and_then(Value::as_str) {
        Some("intent") => {
            let name = event.get("name")?.as_str()?;
            let value = event.get("value")?.clone();
            for item in selected {
                if let Ok(Some(control)) = item.on_intent(name, &value, &mut ctx).await {
                    persist_context(shared_context, &ctx);
                    return Some(intent_control_to_json(&control).to_string());
                }
            }
            persist_context(shared_context, &ctx);
            None
        }
        Some("llm_start") => {
            let mut prompt = event.get("prompt")?.as_str()?.to_string();
            for item in selected {
                if let Ok(Some(updated)) = item.on_llm_start(prompt.clone(), &mut ctx).await {
                    prompt = updated;
                }
            }
            persist_context(shared_context, &ctx);
            Some(
                serde_json::json!({
                    "prompt": prompt,
                    "stack": ctx.stack,
                })
                .to_string(),
            )
        }
        Some("llm_end") => {
            let response = event.get("response")?.clone();
            for item in selected {
                let _ = item.on_llm_end(&response, &mut ctx).await;
            }
            persist_context(shared_context, &ctx);
            None
        }
        Some("run_start") => {
            let session_value = event.get("session")?.clone();
            let mut session: SessionState = serde_json::from_value(session_value).ok()?;
            for item in selected {
                if let Ok(updated) = item.on_run_start(session.clone(), &mut ctx).await {
                    session = updated;
                }
            }
            persist_context(shared_context, &ctx);
            Some(serde_json::json!({ "session": session }).to_string())
        }
        Some("run_complete") => {
            let session_value = event.get("session")?.clone();
            let session: SessionState = serde_json::from_value(session_value).ok()?;
            for item in selected {
                let _ = item.on_run_complete(&session, &mut ctx).await;
            }
            persist_context(shared_context, &ctx);
            None
        }
        Some("error") => {
            let error = event.get("error")?.clone();
            let session = event
                .get("session")
                .cloned()
                .and_then(|value| serde_json::from_value::<SessionState>(value).ok());
            for item in selected {
                if let Ok(true) = item.on_error(&error, session.as_ref(), &mut ctx).await {
                    persist_context(shared_context, &ctx);
                    return Some(serde_json::json!({ "swallow": true }).to_string());
                }
            }
            persist_context(shared_context, &ctx);
            None
        }
        _ => None,
    }
}

fn select_middleware<'a>(
    middleware: &'a [MiddlewareRegistry],
    active_agent: &str,
) -> Vec<&'a MiddlewareRegistry> {
    middleware
        .iter()
        .filter(|item| match item.target() {
            Some(targets) => targets.iter().any(|target| target == active_agent),
            None => true,
        })
        .collect()
}

fn build_context(
    native: &AuwgentNative,
    shared_context: &Arc<Mutex<Map<String, Value>>>,
    event: &Value,
) -> MiddlewareContext {
    let context = event.get("context");
    MiddlewareContext {
        active_agent: context
            .and_then(|ctx| ctx.get("activeAgent"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        stack: context
            .and_then(|ctx| ctx.get("stack"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        root_agent: context
            .and_then(|ctx| ctx.get("rootAgent"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        raw_block: context
            .and_then(|ctx| ctx.get("rawBlock"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        system_prompt: context
            .and_then(|ctx| ctx.get("systemPrompt"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        data: shared_context.lock().unwrap().clone(),
        native: native.clone(),
    }
}

fn persist_context(shared_context: Arc<Mutex<Map<String, Value>>>, ctx: &MiddlewareContext) {
    *shared_context.lock().unwrap() = ctx.data.clone();
}

fn intent_control_to_json(control: &IntentControl) -> Value {
    match control {
        IntentControl::Skip => serde_json::json!({ "skip": true }),
        IntentControl::Override { result } => serde_json::json!({ "result": result }),
    }
}

pub fn parse_intent_control(value: &Value) -> Option<IntentControl> {
    parse_intent_control_response(value)
}

pub fn to_value<T: Serialize>(value: T) -> AuwgentResult<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

pub use ir_runtime::runtime::engine::{IntentControl, RunMetadata};
pub use ir_runtime::runtime::engine_types::AggregateUsage;
pub use ir_runtime::runtime::session::{Message, Role, SessionState, Turn};
pub use ir_runtime::{
    Comparison, ComponentChildrenConstraint, ComponentDefinition, Condition, CustomIntentDef,
    ExamplePair, Expression, HandoffMode, Helper, JsonValue, ModelConfig, NamedModelConfig, Tool,
    TypeDefinition, TypeProperty, Workflow,
};
