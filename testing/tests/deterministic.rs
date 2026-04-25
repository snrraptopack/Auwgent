use auwgent_testing::{
    attach_intent_collector, build_agent, drive_chunk, LifecycleMiddleware, RecordedIntent,
    ToolControlMode, TraceMiddleware,
};
use serde_json::json;

#[test]
fn prompt_generation_contains_agent_contract() {
    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);

    let prompt = agent.generate_prompt(None).expect("prompt generation should work");

    assert!(prompt.contains("You are a helpful assistant"));
    assert!(prompt.contains("get_location"));
    assert!(prompt.contains("get_marks"));
    assert!(prompt.contains("marks_and_location"));
    assert!(prompt.contains("Joker"));
    assert!(prompt.contains("Plan"));
    assert!(prompt.contains("[tool_call:"));
    assert!(prompt.contains("[helper_call:"));
}

#[test]
fn response_text_is_emitted_deterministically() {
    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    let events = attach_intent_collector(&agent);

    drive_chunk(&agent, "[response_text]Hello from deterministic test[/response_text]")
        .expect("processing should succeed");

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured,
        vec![RecordedIntent::ResponseText(
            "Hello from deterministic test".into()
        )]
    );
}

#[test]
fn tool_call_executes_and_emits_result() {
    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    let events = attach_intent_collector(&agent);

    drive_chunk(
        &agent,
        "[tool_call: get_marks]\nid: \"student_7\"\n[/tool_call]",
    )
    .expect("tool processing should succeed");

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured[0],
        RecordedIntent::ToolCall {
            name: "get_marks".into(),
            args: json!({ "id": "student_7" }),
        }
    );
    assert_eq!(
        captured[1],
        RecordedIntent::ToolResult {
            name: "get_marks".into(),
            args: json!({ "id": "student_7" }),
            result: json!("marks:student_7"),
            overridden: false,
        }
    );
}

#[test]
fn workflow_call_executes_and_emits_result() {
    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    let events = attach_intent_collector(&agent);

    drive_chunk(
        &agent,
        "[workflow_call: marks_and_location]\nuser_id: \"student_9\"\n[/workflow]",
    )
    .expect("workflow processing should succeed");

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured[0],
        RecordedIntent::WorkflowCall {
            name: "marks_and_location".into(),
            args: json!({ "user_id": "student_9" }),
        }
    );
    match &captured[1] {
        RecordedIntent::WorkflowResult {
            name,
            args,
            result,
            overridden,
        } => {
            assert_eq!(name, "marks_and_location");
            assert_eq!(args, &json!({ "user_id": "student_9" }));
            assert_eq!(*overridden, false);
            let text = result.as_str().expect("workflow result should be text");
            assert!(text.contains("user location: Accra"));
            assert!(text.contains("user marks: marks:student_9"));
        }
        other => panic!("expected workflow result, got {other:?}"),
    }
}

#[test]
fn middleware_can_skip_tool_and_record_trace() {
    let middleware = TraceMiddleware {
        control_mode: ToolControlMode::SkipGetMarks,
        ..Default::default()
    };
    let trace = middleware.events.clone();
    let agent = build_agent(vec![middleware]);
    let events = attach_intent_collector(&agent);

    drive_chunk(
        &agent,
        "[tool_call: get_marks]\nid: \"student_4\"\n[/tool_call]",
    )
    .expect("tool processing should succeed");

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured,
        vec![RecordedIntent::ToolSkipped {
            name: "get_marks".into(),
            args: json!({ "id": "student_4" }),
        }]
    );

    assert_eq!(
        trace.lock().unwrap().clone(),
        vec!["intent:tool_call".to_string()]
    );
}

#[test]
fn middleware_can_override_tool_result() {
    let middleware = TraceMiddleware {
        control_mode: ToolControlMode::OverrideGetMarks,
        ..Default::default()
    };
    let agent = build_agent(vec![middleware]);
    let events = attach_intent_collector(&agent);

    drive_chunk(
        &agent,
        "[tool_call: get_marks]\nid: \"student_5\"\n[/tool_call]",
    )
    .expect("tool processing should succeed");

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured,
        vec![RecordedIntent::ToolResult {
            name: "get_marks".into(),
            args: json!({ "id": "student_5" }),
            result: json!("override:marks"),
            overridden: true,
        },]
    );
}

#[test]
fn middleware_target_filtering_works_for_deterministic_intents() {
    let off_target = TraceMiddleware {
        target: Some(vec!["OtherAgent".into()]),
        ..Default::default()
    };
    let off_trace = off_target.events.clone();

    let on_target = TraceMiddleware::default();
    let on_trace = on_target.events.clone();

    let agent = build_agent(vec![off_target, on_target]);

    drive_chunk(&agent, "[response_text]hello[/response_text]").expect("processing should succeed");

    assert!(off_trace.lock().unwrap().is_empty());
    assert_eq!(
        on_trace.lock().unwrap().clone(),
        vec!["intent:response_text".to_string()]
    );
}

#[test]
fn session_export_import_round_trips() {
    let agent = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    drive_chunk(&agent, "[response_text]persist me[/response_text]").expect("processing should succeed");

    let exported = agent.export_session().expect("session export should work");
    let restored = build_agent::<auwgent_testing::fixture::SimpleToolMiddlewareRegistry>(vec![]);
    restored
        .import_session(&exported)
        .expect("session import should work");

    let re_exported = restored.export_session().expect("re-export should work");
    assert_eq!(
        serde_json::to_value(re_exported).unwrap(),
        serde_json::to_value(exported).unwrap()
    );
}

#[tokio::test]
async fn middleware_lifecycle_records_run_and_error_flow() {
    let middleware = LifecycleMiddleware::default();
    let events = middleware.events.clone();
    let seen = middleware.seen_data.clone();

    let result = auwgent_testing::run_without_keys(vec![middleware.into()]).await;
    assert!(result.is_err(), "run without provider keys should fail deterministically");

    let recorded = events.lock().unwrap().clone();
    assert!(recorded.iter().any(|entry| entry == "run_start"));
    assert!(recorded.iter().any(|entry| entry.starts_with("error:")));

    let snapshots = seen.lock().unwrap().clone();
    assert!(!snapshots.is_empty());
}

#[tokio::test]
async fn middleware_context_is_available_on_run_start_and_error() {
    let middleware = LifecycleMiddleware::default();
    let seen = middleware.seen_data.clone();

    let _ = auwgent_testing::run_without_keys(vec![middleware.into()]).await;

    let snapshots = seen.lock().unwrap().clone();
    assert!(
        snapshots.len() >= 2,
        "expected at least run_start and error snapshots"
    );
    assert!(snapshots[0].is_empty());
}
