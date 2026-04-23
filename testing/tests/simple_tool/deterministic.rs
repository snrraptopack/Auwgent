use std::sync::{Arc, Mutex};

use super::shared::{build_config, generated_main};

#[tokio::test]
async fn generated_rust_fixture_factory_builds_agent_and_exposes_runtime_shape() {
    let agent =
        generated_main::auwgent(build_config()).expect("generated factory should build agent");

    let prompt = agent
        .generate_prompt(None)
        .expect("prompt generation should work from generated fixture");

    let tool_names = agent.get_tool_names();
    let tool_schemas = agent
        .get_tool_schemas()
        .expect("tool schemas should be available");

    assert!(prompt.contains("You are a helpful assistant."));
    assert!(tool_names.contains(&"get_location".to_string()));
    assert!(tool_schemas.is_array() || tool_schemas.is_object());
}

#[tokio::test]
async fn generated_rust_fixture_can_process_a_deterministic_tool_call() {
    let agent =
        generated_main::auwgent(build_config()).expect("generated factory should build agent");

    let captured = Arc::new(Mutex::new(
        Vec::<(generated_main::SimpleToolIntent, String)>::new(),
    ));
    let captured_clone = Arc::clone(&captured);

    agent.on_intent(move |intent, agent_name| {
        captured_clone
            .lock()
            .expect("intent capture lock")
            .push((intent, agent_name.to_string()));
        None
    });

    agent.write_chunk("[tool_call: get_location]\n[/tool_call]".to_string());

    let terminal = agent
        .end_stream()
        .expect("ending deterministic stream should succeed");

    let processed = agent
        .process_intents()
        .await
        .expect("processing deterministic intents should succeed");

    let events = captured.lock().expect("captured events lock");

    assert!(terminal.is_object() || terminal.is_array() || terminal.is_null());
    assert!(processed.is_object() || processed.is_array() || processed.is_boolean());
    assert!(!events.is_empty());

    let saw_tool_call = events.iter().any(|(intent, _)| {
        matches!(
            intent,
            generated_main::SimpleToolIntent::ToolCall(
                generated_main::SimpleToolToolCallIntent::GetLocation
            )
        )
    });

    let saw_tool_result = events.iter().any(|(intent, _)| {
        matches!(
            intent,
            generated_main::SimpleToolIntent::ToolResult(
                generated_main::SimpleToolToolResultIntent::GetLocation { .. }
            )
        )
    });

    assert!(saw_tool_call || saw_tool_result);
}

#[test]
fn generated_fixture_module_exports_expected_aliases() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<generated_main::SimpleToolAgent>();
    assert_send_sync::<generated_main::SimpleToolIntent>();
    assert_send_sync::<generated_main::SimpleToolIntentPartial>();
    assert_send_sync::<generated_main::SimpleToolToolsRegistry>();
}
