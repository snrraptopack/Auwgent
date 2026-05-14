use auwgent_ir_schema::AgentIR;
use auwgent_engine::AuwgentEngine;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

fn build_ir() -> AgentIR {
    serde_json::from_value(json!({
        "name": "ParallelActions",
        "modelConfig": [],
        "input": null,
        "output": null,
        "context": null,
        "tools": [
            {
                "name": "slow",
                "description": null,
                "params": {},
                "returns": { "type": "string" }
            },
            {
                "name": "fast",
                "description": null,
                "params": {},
                "returns": { "type": "string" }
            }
        ],
        "workflows": [],
        "helpers": [],
        "components": [],
        "tests": []
    }))
    .expect("valid test ir")
}

#[tokio::test]
async fn tool_calls_from_same_model_turn_execute_concurrently() {
    let engine = AuwgentEngine::new(build_ir());
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let result_order = Arc::new(Mutex::new(Vec::<String>::new()));

    {
        let result_order = Arc::clone(&result_order);
        engine.on_intent(Arc::new(move |name, value, _agent| {
            let result_order = Arc::clone(&result_order);
            Box::pin(async move {
                if name == "tool_result"
                    && let Some(tool_name) = value.get("name").and_then(Value::as_str)
                {
                    result_order.lock().unwrap().push(tool_name.to_string());
                }
                None
            })
        }));
    }

    for (name, label, delay_ms) in [("slow", "slow", 120), ("fast", "fast", 120)] {
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        engine.register_tool(
            name,
            Arc::new(move |_args: Value| {
                let active = Arc::clone(&active);
                let max_active = Arc::clone(&max_active);
                let label = label.to_string();
                Box::pin(async move {
                    let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(now_active, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(Value::String(label))
                })
            }),
        );
    }

    engine.write_llm_chunk(
        r#"
[tool_call: slow]
[/tool_call]
[tool_call: fast]
[/tool_call]
"#,
    );
    engine.end_llm_stream();

    let started = Instant::now();
    let (_terminal, actions, _hard_stop) = engine.process_intents().await.unwrap();
    let elapsed = started.elapsed();

    assert!(actions);
    assert_eq!(max_active.load(Ordering::SeqCst), 2);
    assert!(
        elapsed < Duration::from_millis(220),
        "expected overlapping tool execution, got {elapsed:?}"
    );

    assert_eq!(
        *result_order.lock().unwrap(),
        vec!["slow".to_string(), "fast".to_string()]
    );
}
