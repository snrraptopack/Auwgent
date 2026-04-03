use ir_runtime::intents::{generate_block_protocol_prompt, generate_helper_block_protocol_prompt};
use ir_runtime::AgentIR;
use serde_json::json;

fn build_ir() -> AgentIR {
    serde_json::from_value(json!({
        "name": "Main",
        "modelConfig": [{
            "defaultConfig": {
                "model": { "type": "modelRef", "name": "test-model" },
                "prompt": { "type": "literal", "value": "hi" }
            }
        }],
        "tools": [{
            "name": "lookup_user",
            "description": "Find a user",
            "params": {
                "id": { "type": "string", "optional": false }
            },
            "returns": { "type": "string" }
        }],
        "workflows": [{
            "flowName": "route_case",
            "flowParams": {
                "priority": { "type": "string", "optional": false }
            },
            "returns": { "type": "string" },
            "body": []
        }],
        "helpers": [{
            "name": "Summarizer",
            "input": {
                "kind": "properties",
                "fields": {
                    "text": { "type": "string", "optional": false }
                }
            },
            "modelConfig": [],
            "tools": [],
            "workflows": [],
            "examples": []
        }],
        "helperToolGrants": {
            "Summarizer": "all"
        },
        "output": {
            "status": { "type": "string", "optional": false }
        }
    }))
    .expect("valid test ir")
}

#[test]
fn main_protocol_prompt_includes_generic_block_syntax() {
    let prompt = generate_block_protocol_prompt(&build_ir());

    assert!(prompt.contains("Block syntax:"));
    assert!(prompt.contains("[tool_call: valid_tool_name]"));
    assert!(prompt.contains("[workflow_call: valid_workflow_name]"));
    assert!(prompt.contains("[helper_call: valid_helper_name]"));
    assert!(prompt.contains("[schema: valid_schema_name]"));
    assert!(prompt.contains("then write one `key: value` field per line"));
    assert!(prompt.contains("close with [/tool]"));
    assert!(prompt.contains("close with [/workflow]"));
    assert!(prompt.contains("close with [/helper]"));
    assert!(!prompt.contains("Generic workflow format:"));
    assert!(!prompt.contains("Generic tool format:"));
}

#[test]
fn helper_protocol_prompt_includes_generic_tool_syntax() {
    let prompt = generate_helper_block_protocol_prompt(&build_ir(), "Summarizer");

    assert!(prompt.contains("Block syntax:"));
    assert!(prompt.contains("[tool_call: valid_tool_name]"));
    assert!(prompt.contains("close with [/tool]"));
}
