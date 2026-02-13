use ir_runtime::intent_parser::orchestrator::{Orchestrator, extract_yaml};
use std::sync::Arc;
use std::sync::Mutex;

fn main() {
    println!("=== Testing Intent Orchestrator ===");

    let mut orchestrator = Orchestrator::new(None);
    orchestrator.register_intent("response_text");
    orchestrator.register_intent("tool_call");

    let intents = Arc::new(Mutex::new(Vec::new()));
    let intents_clone = intents.clone();

    orchestrator.on_intent_ready(Arc::new(move |name, value| {
        println!("READY: intent='{}' value={}", name, value);
        intents_clone.lock().unwrap().push((name, value));
    }));

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = partials.clone();
    orche_strator_on_intent_partial(
        &mut orchestrator,
        Arc::new(move |name, value| {
            println!("PARTIAL: intent='{}' value={}", name, value);
            partials_clone.lock().unwrap().push((name, value));
        }),
    );

    println!("\n--- Part 1: Noisy Output Extraction ---");
    let noisy = r#"Sure! Here is the response:
```yaml
response_text:
  text: "Hello, world!"
```
Hope that helps!"#;
    let cleaned = extract_yaml(noisy);
    println!("Cleaned YAML:\n{}", cleaned);

    println!("\n--- Part 2: Streaming Parsing ---");
    let yaml_chunks = vec![
        "response_text:\n",
        "  text: \"Hello",
        " world",
        "!\"\n",
        "tool_call:\n",
        "  type: calculator\n",
        "  args:\n",
        "    a: 10\n",
        "    b: 20\n",
    ];

    for chunk in yaml_chunks {
        println!("Writing chunk: {:?}", chunk);
        orchestrator.write(chunk);
    }

    println!("\nFinal End:");
    let final_res = orchestrator.end();
    println!("Final Document: {}", final_res);
}

// Helper to handle the Arc signature
fn orche_strator_on_intent_partial(
    orchestrator: &mut Orchestrator,
    handler: Arc<dyn Fn(String, serde_json::Value) + Send + Sync>,
) {
    orchestrator.on_intent_partial(handler);
}
