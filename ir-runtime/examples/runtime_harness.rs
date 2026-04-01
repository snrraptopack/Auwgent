use ir_runtime::AgentIR;
use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::gemini::GeminiDriver;
use serde_json::json;
use std::{env, fs, sync::Arc};

/// Optional legacy verification harness.
///
/// Run only when needed:
/// cargo run --example runtime_harness
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string("../manual-testing/out/main.agent.json")?;
    let agent: AgentIR = serde_json::from_str(&content)?;
    println!(" Successfully parsed Agent: {}", agent.name);

    println!("\nInitializing AuwgentEngine...");
    let engine = AuwgentEngine::new(agent);

    engine.register_tool(
        "hello",
        Arc::new(|args| {
            Box::pin(async move {
                println!("  [Host] Tool 'hello' called with args: {}", args);
                Ok(json!(format!("Success: Tool 'hello' processed {:?}", args)))
            })
        }),
    );

    engine.on_intent(Arc::new(|name, value, _agent| {
        Box::pin(async move {
            println!("\n  [onIntent] {} -> {}", name, value);
            None
        })
    }));

    if let Ok(key) = env::var("GEMINI_API_KEY") {
        println!("*** LIVE MODE ENABLED (Gemini) ***");
        let driver = GeminiDriver::new(key);
        engine.register_driver("gemini", std::sync::Arc::new(driver));

        println!("\nStarting Live Agentic Run...");
        match engine
            .run(
                Some(json!("Please call the hello tool with id 'live_test_001'")),
                None,
            )
            .await
        {
            Ok(_) => println!("\nLive run completed successfully."),
            Err(e) => println!("\nLive run failed: {}", e),
        }
    } else {
        println!("*** SIMULATION MODE ENABLED (No GEMINI_API_KEY found) ***");

        let engine_prompt = engine.generate_prompt(None)?;
        println!("Engine Generated Prompt (Length: {})", engine_prompt.len());
        println!(
            "\n--- ENGINE PROMPT START ---\n{}\n--- ENGINE PROMPT END ---",
            engine_prompt
        );

        println!("\nSimulating LLM Output (Tool Call Intent)...");
        engine.write_llm_chunk("```yaml\n");
        engine.write_llm_chunk("tool_call:\n  type: hello\n  args:\n    id: \"123\"\n");
        engine.write_llm_chunk("```\n");
        engine.end_llm_stream();

        let _ = engine.process_intents().await?;

        println!("\nSimulating LLM Output (Workflow Call Intent)...");
        engine.write_llm_chunk("```yaml\n");
        engine.write_llm_chunk(
            "workflow_call:\n  type: two\n  args:\n    value: \"simulated_test\"\n",
        );
        engine.write_llm_chunk("```\n");
        engine.end_llm_stream();

        engine.process_intents().await?;
    }

    println!("\nFinal Session Turns:");
    for (i, turn) in engine.session().turns.iter().enumerate() {
        println!(
            "  Turn {}: input={:?} response_len={}",
            i,
            turn.input,
            turn.model_response.len()
        );
    }

    Ok(())
}
