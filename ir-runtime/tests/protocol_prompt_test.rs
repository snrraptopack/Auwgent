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
        "components": [{
            "name": "Button",
            "props": {
                "label": { "type": "string", "optional": false },
                "variant": { "type": "string", "optional": false }
            },
            "action": {
                "onclick": [
                    { "name": "confirm_order" },
                    {
                        "name": "delete_user",
                        "params": {
                            "id": { "type": "string", "optional": false }
                        }
                    }
                ]
            }
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
    assert!(prompt.contains("[response_text]...[/response_text]"));
    assert!(prompt.contains("[tool_call: valid_tool_name]"));
    assert!(prompt.contains("[workflow_call: valid_workflow_name]"));
    assert!(prompt.contains("[helper_call: valid_helper_name]"));
    assert!(prompt.contains("[component: valid_component_name, c_id:\"meaningful_accessible_id\"]"));
    assert!(prompt.contains("[render_component]"));
    assert!(prompt.contains("[schema: valid_schema_name]"));
    assert!(prompt.contains("Components available:"));
    assert!(prompt.contains("Button(label: string"));
    assert!(prompt.contains("action_onclick: confirm_order | delete_user(id: string)"));
    assert!(prompt.contains("root: \"component_c_id\""));
    assert!(prompt.contains("UI output must end with a [render_component] block"));
    assert!(prompt.contains("emit only the action block(s) for that turn and stop"));
    assert!(prompt.contains("Do not emit response_text or response_schema in the same response"));
    assert!(prompt.contains("then write one `key: value` or `key = value` field per line"));
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
    assert!(prompt.contains("[response_text]...[/response_text]"));
    assert!(prompt.contains("[tool_call: valid_tool_name]"));
    assert!(prompt.contains("[component: valid_component_name, c_id:\"meaningful_accessible_id\"]"));
    assert!(prompt.contains("[render_component]"));
    assert!(prompt.contains("emit only the tool_call block(s) for that turn and stop"));
    assert!(prompt.contains("close with [/tool]"));
    assert!(prompt.contains("close with [/component]"));
    assert!(prompt.contains("close with [/render_component]"));
}
