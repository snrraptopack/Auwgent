use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::AgentIR;
use serde_json::json;

fn build_ir() -> AgentIR {
    serde_json::from_value(json!({
        "name": "TestAgent",
        "customIntents": [
            {
                "name": "ask_user",
                "fields": {
                    "question": { "type": "string", "optional": false },
                    "options": {
                        "type": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "optional": false
                    }
                }
            }
        ],
        "modelConfig": [{
            "defaultConfig": {
                "model": { "type": "modelRef", "name": "test-model" },
                "prompt": { "type": "literal", "value": "hi" }
            }
        }]
    }))
    .expect("valid test ir")
}

fn process_and_drain(engine: &AuwgentEngine, input: &str) -> Vec<serde_json::Value> {
    engine.write_llm_chunk(input);
    engine.end_llm_stream();

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        engine.process_intents().await.expect("process intents");
    });

    engine
        .drain_structured_output_jsonl()
        .into_iter()
        .map(|line| serde_json::from_str(&line).expect("valid jsonl event"))
        .collect()
}

#[test]
fn structured_output_emits_jsonl_for_multiple_intent_shapes() {
    let engine = AuwgentEngine::new(build_ir());

    let events = process_and_drain(
        &engine,
        r#"
[tool_call: fetch_session]
session_id: "sess_123"
[/tool_call]
[workflow_call: process_data]
input: "hello"
[/workflow]
[schema: Output]
status: "ok"
[/schema]
[custom: ask_user]
question: "Continue?"
options: ["yes", "no"]
[/custom]
"#,
    );

    assert_eq!(events.len(), 3);

    let names: Vec<String> = events
        .iter()
        .map(|event| event["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "tool_call",
            "workflow_call",
            "ask_user"
        ]
    );

    for (idx, event) in events.iter().enumerate() {
        assert_eq!(event["event"], "intent");
        assert_eq!(event["phase"], "final");
        assert_eq!(event["seq"], (idx as u64) + 1);
    }

    let tool_call = events.iter().find(|event| event["name"] == "tool_call").unwrap();
    let workflow_call = events.iter().find(|event| event["name"] == "workflow_call").unwrap();
    let ask_user = events.iter().find(|event| event["name"] == "ask_user").unwrap();

    assert_eq!(tool_call["payload"]["type"], "fetch_session");
    assert_eq!(tool_call["payload"]["args"]["session_id"], "sess_123");
    assert_eq!(workflow_call["payload"]["type"], "process_data");
    assert_eq!(workflow_call["payload"]["args"]["input"], "hello");
    assert_eq!(ask_user["payload"]["question"], "Continue?");
    assert_eq!(ask_user["payload"]["options"][0], "yes");
}

#[test]
fn structured_output_drain_clears_buffer() {
    let engine = AuwgentEngine::new(build_ir());

    let first_events = process_and_drain(&engine, "[response_text]First[/response_text]");
    assert_eq!(first_events.len(), 1);

    let second_drain = engine.drain_structured_output_jsonl();
    assert!(second_drain.is_empty());
}

#[test]
fn structured_output_drain_text_is_newline_delimited() {
    let engine = AuwgentEngine::new(build_ir());

    engine.write_llm_chunk("[response_text]One[/response_text][response_text]Two[/response_text]");
    engine.end_llm_stream();

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        engine.process_intents().await.expect("process intents");
    });

    let jsonl = engine.drain_structured_output_jsonl_text();
    let lines: Vec<&str> = jsonl.lines().collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("line 1 json");
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("line 2 json");

    assert_eq!(first["seq"], 1);
    assert_eq!(second["seq"], 2);
    assert_eq!(first["payload"]["text"], "One");
    assert_eq!(second["payload"]["text"], "Two");
}

#[test]
fn structured_output_jsonl_does_not_leak_stray_response_text_closer() {
    let engine = AuwgentEngine::new(build_ir());

    let events = process_and_drain(
        &engine,
        "[response_text]Hello structured world[/response_text][/response_text]",
    );

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["name"], "response_text");
    assert_eq!(events[0]["payload"]["text"], "Hello structured world");
}

#[test]
fn structured_output_ignores_terminal_text_when_action_exists_in_same_turn() {
    let engine = AuwgentEngine::new(build_ir());

    let events = process_and_drain(
        &engine,
        r#"
[tool_call: user_name]
name: null
[/tool_call]
[response_text]I don't have your name.[/response_text]
"#,
    );

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["name"], "tool_call");
    assert_eq!(events[0]["payload"]["type"], "user_name");
}
