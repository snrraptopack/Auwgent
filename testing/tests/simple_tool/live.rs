use std::env;
use std::sync::{Arc, Mutex};

use super::shared::{build_config, generated_main};

/// Live-provider variant for the `simple-tool` scenario.
///
/// This test mirrors the deterministic variant as closely as possible:
/// - same `.agent` source
/// - same committed generated Rust fixture
/// - same intent capture path
/// - same core assertion: the runtime should surface a tool call/result flow
///
/// The only difference is that this variant relies on a real model/provider
/// instead of injecting the tool-call stream manually.
///
/// Run manually with:
/// `cargo test --manifest-path testing/Cargo.toml --test simple_tool live_simple_tool_mirrors_deterministic_tool_call -- --ignored`
#[tokio::test]
#[ignore = "requires a real provider API key and makes live network calls"]
async fn live_simple_tool_mirrors_deterministic_tool_call() {
    let groq_api_key = env::var("GROQ_API_KEY")
        .expect("set GROQ_API_KEY to run the live simple-tool provider test");

    let mut config = build_config();
    config.api_keys.groq_api_key = Some(groq_api_key);

    let agent =
        generated_main::auwgent(config).expect("generated factory should build a live agent");

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

    let _ = agent
        .run(Some(serde_json::json!(
            "Please use the get_location tool to find my location, wait for the tool result, and then answer briefly."
        )))
        .await
        .expect("live provider run should complete successfully");

    let events = captured.lock().expect("captured events lock");
    assert!(
        !events.is_empty(),
        "expected the live run to emit at least one intent"
    );

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

    assert!(
        saw_tool_call || saw_tool_result,
        "expected the live run to surface a get_location tool call/result flow, got: {:?}",
        *events
    );
}