use std::sync::{Arc, Mutex};

#[path = "../../cases/simple-tool/generated/main.agent.rs"]
pub mod generated_main;

pub struct TestTools;

impl generated_main::SimpleToolTools for TestTools {
    fn get_location(
        &self,
        _args: generated_main::NoArgs,
    ) -> generated_main::SimpleToolGetLocationToolResultValue {
        "Accra".to_string()
    }
}

pub fn build_config() -> generated_main::SimpleToolConfig {
    generated_main::SimpleToolConfig {
        tools: generated_main::SimpleToolToolsRegistry::new(TestTools),
        middleware: Vec::new(),
        api_keys: generated_main::SimpleToolApiKeys::default(),
    }
}

pub fn build_agent() -> generated_main::SimpleToolAgent {
    generated_main::auwgent(build_config()).expect("generated simple-tool fixture should build")
}

pub type CapturedIntent = (generated_main::SimpleToolIntent, String);
pub type CapturedIntents = Arc<Mutex<Vec<CapturedIntent>>>;

pub fn attach_intent_capture(agent: &generated_main::SimpleToolAgent) -> CapturedIntents {
    let captured = Arc::new(Mutex::new(Vec::<CapturedIntent>::new()));
    let captured_clone = Arc::clone(&captured);

    agent.on_intent(move |intent, agent_name| {
        captured_clone
            .lock()
            .expect("intent capture lock")
            .push((intent, agent_name.to_string()));
        None
    });

    captured
}

pub fn saw_tool_call(events: &[CapturedIntent]) -> bool {
    events.iter().any(|(intent, _)| {
        matches!(
            intent,
            generated_main::SimpleToolIntent::ToolCall(
                generated_main::SimpleToolToolCallIntent::GetLocation
            )
        )
    })
}

pub fn saw_tool_result(events: &[CapturedIntent]) -> bool {
    events.iter().any(|(intent, _)| {
        matches!(
            intent,
            generated_main::SimpleToolIntent::ToolResult(
                generated_main::SimpleToolToolResultIntent::GetLocation { .. }
            )
        )
    })
}