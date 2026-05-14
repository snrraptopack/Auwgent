use auwgent_ir_schema::AgentIR;
use auwgent_prompt::{generate_block_protocol_prompt, generate_helper_block_protocol_prompt};
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
        }, {
            "name": "Joker",
            "description": "Is A Joker Helper use it for jokes",
            "input": null,
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

    assert!(prompt.contains("Blocks:"));
    assert!(prompt.contains("[response_text] plain text [/response_text]"));
    assert!(prompt.contains("[tool_call: name] key: value per line [/tool_call]"));
    assert!(prompt.contains("[workflow_call: name] key: value per line [/workflow_call]"));
    assert!(prompt.contains("[helper_call: name] key: value per line [/helper]"));
    assert!(prompt.contains("Summarizer(text: string)"));
    assert!(prompt.contains("Joker(input: string)"));
    assert!(prompt.contains("[component: name, c_id:\"id\"] key: value per line [/component]"));
    assert!(prompt.contains("[render_component]"));
    assert!(prompt.contains("[schema: name] key: value per line [/schema]"));
    assert!(prompt.contains("Components:"));
    assert!(prompt.contains("Button(label: string"));
    assert!(prompt.contains("action_onclick: confirm_order | delete_user(id: string)"));
    assert!(prompt.contains("root: \"<c_id>\""));
    assert!(prompt.contains("Components require c_id. UI must end with [render_component]."));
    assert!(prompt.contains("emit only action block(s) for that turn and stop"));
    assert!(prompt.contains("No response_text in the same response as action blocks."));
    assert!(!prompt.contains("Generic workflow format:"));
    assert!(!prompt.contains("Generic tool format:"));
}

#[test]
fn helper_protocol_prompt_includes_generic_tool_syntax() {
    let prompt = generate_helper_block_protocol_prompt(&build_ir(), "Summarizer");

    assert!(prompt.contains("Blocks:"));
    assert!(prompt.contains("[response_text] plain text [/response_text]"));
    assert!(prompt.contains("[tool_call: name] key: value per line [/tool_call]"));
    assert!(prompt.contains("[component: name, c_id:\"id\"] key: value per line [/component]"));
    assert!(prompt.contains("[render_component]"));
    assert!(prompt.contains("emit only tool_call block(s) and stop"));
}
