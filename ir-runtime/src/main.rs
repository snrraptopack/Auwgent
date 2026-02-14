use ir_runtime::AgentIR;
use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::runtime::drivers::gemini::GeminiDriver;
use serde_json::json;
use std::{env, fs, sync::Arc};

/**
 * Main entry point for the IR Runtime verification harness.
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ======================================================================================
    // 1. Load and Parse IR
    // ======================================================================================
    let content = fs::read_to_string("../manual-testing/out/main.agent.json")?;
    let agent: AgentIR = serde_json::from_str(&content)?;
    println!(" Successfully parsed Agent: {}", agent.name);

    // ======================================================================================
    // 2. Setup AuwgentEngine
    // ======================================================================================
    println!("\nInitializing AuwgentEngine...");
    let mut engine = AuwgentEngine::new(agent);

    // Register tools
    engine.register_tool(
        "hello",
        Arc::new(|args| {
            Box::pin(async move {
                println!("  [Host] Tool 'hello' called with args: {}", args);
                Ok(json!(format!("Success: Tool 'hello' processed {:?}", args)))
            })
        }),
    );

    // Check for real LLM integration
    if let Ok(key) = env::var("GEMINI_API_KEY") {
        println!("*** LIVE MODE ENABLED (Gemini) ***");
        let driver = GeminiDriver::new(key);
        engine.set_driver(Box::new(driver));

        println!("\nStarting Live Agentic Run...");
        // Ask a question that should trigger the "hello" tool
        match engine
            .run(Some(json!(
                "Please call the hello tool with id 'live_test_001'"
            )))
            .await
        {
            Ok(_) => println!("\nLive run completed successfully."),
            Err(e) => println!("\nLive run failed: {}", e),
        }
    } else {
        println!("*** SIMULATION MODE ENABLED (No GEMINI_API_KEY found) ***");

        let engine_prompt = engine.generate_prompt()?;
        println!("Engine Generated Prompt (Length: {})", engine_prompt.len());
        println!("\n--- ENGINE PROMPT START ---\n{}\n--- ENGINE PROMPT END ---", engine_prompt);

        // Simulate LLM Output with a tool_call intent
        println!("\nSimulating LLM Output (Tool Call Intent)...");
        engine.write_llm_chunk("```yaml\n");
        engine.write_llm_chunk("tool_call:\n  type: hello\n  args:\n    id: \"123\"\n");
        engine.write_llm_chunk("```\n");
        engine.end_llm_stream();

        let _ = engine.process_intents().await?;

        // Simulate Workflow
        println!("\nSimulating LLM Output (Workflow Call Intent)...");
        engine.write_llm_chunk("```yaml\n");
        engine.write_llm_chunk(
            "workflow_call:\n  type: two\n  args:\n    value: \"simulated_test\"\n",
        );
        engine.write_llm_chunk("```\n");
        engine.end_llm_stream();

        engine.process_intents().await?;
    }

    // ======================================================================================
    // 3. Final Session State
    // ======================================================================================
    println!("\nFinal Session Steps:");
    let steps = engine.get_session_steps();
    for (i, step) in steps.iter().enumerate() {
        println!("  Step {}: {:?}", i, step);
    }

    Ok(())
}
