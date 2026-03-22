/// Integration tests for block protocol

use ir_runtime::intent_parser::block_orchestrator::BlockOrchestrator;
use std::sync::{Arc, Mutex};

#[test]
fn test_chat_to_response_text() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@chat\nHello world\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "response_text");
    assert_eq!(results[0].1["text"], "Hello world");
}

#[test]
fn test_tool_to_tool_call() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@tool\nfetch_session(session_id = \"sess_123\")\nget_user(user_id = \"usr_456\")\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "tool_call");
    assert_eq!(results[0].1["type"], "fetch_session");
    assert_eq!(results[0].1["args"]["session_id"], "sess_123");
    assert_eq!(results[1].0, "tool_call");
    assert_eq!(results[1].1["type"], "get_user");
}

#[test]
fn test_out_to_response_schema() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_schema");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@out ContextCompilerOutput\n{session_id: \"sess_123\", user: {id: \"usr_456\", name: \"Nana\"}}\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "response_schema");
    assert_eq!(results[0].1["type"], "ContextCompilerOutput");
    assert_eq!(results[0].1["response"]["session_id"], "sess_123");
    assert_eq!(results[0].1["response"]["user"]["name"], "Nana");
}

#[test]
fn test_workflow_to_workflow_call() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("workflow_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@workflow\nprocess_data(input = \"test\", config = {timeout = 30})\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "workflow_call");
    assert_eq!(results[0].1["type"], "process_data");
    assert_eq!(results[0].1["args"]["input"], "test");
}

#[test]
fn test_helper_to_helper_call() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("helper_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@helper\nStoryTeller(city = \"Accra\", days = 3)\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "helper_call");
    assert_eq!(results[0].1["type"], "StoryTeller");
    assert_eq!(results[0].1["args"]["city"], "Accra");
}

#[test]
fn test_multi_block_response() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");
    orch.register_intent("tool_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    let input = r#"
@@chat
Let me fetch that data.
@@end

@@tool
fetch_session(session_id = "sess_123")
get_user(user_id = "usr_456")
@@end

@@chat
Here's the result.
@@end
"#;

    orch.write(input);
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 4); // 2 chat + 2 tools
    assert_eq!(results[0].0, "response_text");
    assert_eq!(results[1].0, "tool_call");
    assert_eq!(results[2].0, "tool_call");
    assert_eq!(results[3].0, "response_text");
}

#[test]
fn test_implicit_chat() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");
    orch.register_intent("tool_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    let input = r#"
Let me help you.

@@tool
fetch(id = "123")
@@end

Here's the result.
"#;

    orch.write(input);
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 3); // implicit chat + tool + implicit chat
    assert_eq!(results[0].0, "response_text");
    assert!(results[0].1["text"].as_str().unwrap().contains("Let me help"));
    assert_eq!(results[1].0, "tool_call");
    assert_eq!(results[2].0, "response_text");
    assert!(results[2].1["text"].as_str().unwrap().contains("Here's the result"));
}

#[test]
fn test_auto_close() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");
    orch.register_intent("tool_call");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    // Missing @@end - should auto-close when next marker appears
    orch.write("@@chat\nHello\n@@tool\nfetch(id = \"123\")");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "response_text");
    assert_eq!(results[1].0, "tool_call");
}

#[test]
fn test_last_wins_for_terminal() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    // Multiple chat blocks - in block protocol, each block is intentional and should be emitted
    // The "last-wins" behavior applies to response_schema (structured output), not response_text
    orch.write("@@chat\nFirst attempt\n@@end\n@@chat\nSecond attempt\n@@end\n@@chat\nFinal answer\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 3); // All chat blocks emitted
    assert_eq!(results[0].1["text"], "First attempt");
    assert_eq!(results[1].1["text"], "Second attempt");
    assert_eq!(results[2].1["text"], "Final answer");
}

#[test]
fn test_custom_intent() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("ask_user");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("@@ask_user\nconfirm(question = \"Are you sure?\", options = [\"yes\", \"no\"])\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "ask_user");
    assert_eq!(results[0].1["question"], "Are you sure?");
}

#[test]
fn test_last_wins_for_response_schema() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_schema");
    
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);
    
    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    // Multiple @@out blocks - should only emit the last one (last-wins for structured output)
    orch.write("@@out Result\n{status: \"first\"}\n@@end\n@@out Result\n{status: \"second\"}\n@@end\n@@out Result\n{status: \"final\"}\n@@end");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1); // Only last one
    assert_eq!(results[0].0, "response_schema");
    assert_eq!(results[0].1["response"]["status"], "final");
}
