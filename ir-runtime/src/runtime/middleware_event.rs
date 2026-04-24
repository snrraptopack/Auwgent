use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug,Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MiddlewareEvent {
    RunStart(RunStartPayload),
    RunComplete(RunCompletePayload),
    LlmStart(LlmStartPayload),
    LlmEnd(LlmEndPayload),
    Intent(IntentPayload),
    Error(ErrorPayload),
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct EventContext {
    #[serde(rename = "activeAgent")]
    pub active_agent: String,
    #[serde(rename = "stack")]
    pub stack: Vec<String>,
    #[serde(rename = "rootAgent")]
    pub root_agent: String,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: Option<String>,
    #[serde(rename = "rawBlock")]
    pub raw_block: Option<String>,
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct RunStartPayload {
    pub session: Value,
    pub context: EventContext,
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct RunCompletePayload {
    pub session: Value,
    pub context: EventContext,
}


#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct LlmStartPayload {
    pub prompt: String,
    pub context: EventContext,
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct LlmEndPayload {
    pub response: Value,
    pub context: EventContext,
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct ErrorPayload {
    pub session:Option<Value>,
    pub context: EventContext,
    pub error: Value,
}

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct IntentPayload {
    pub name: String,
    pub context: EventContext,
    pub value: Value,
}
