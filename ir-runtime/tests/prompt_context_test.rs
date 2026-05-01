use ir_runtime::AgentIR;
use ir_runtime::runtime::AuwgentEngine;
use serde_json::json;

fn build_ir(prompt: serde_json::Value) -> AgentIR {
    serde_json::from_value(json!({
        "name": "TestAgent",
        "modelConfig": [
            {
                "defaultConfig": {
                    "model": {
                        "type": "modelRef",
                        "name": "test-model"
                    },
                    "prompt": prompt
                }
            }
        ]
    }))
    .expect("valid test ir")
}

#[test]
fn referenced_context_is_rendered_inline_without_static_duplication() {
    let ir = build_ir(json!({
        "type": "parts",
        "value": [
            { "type": "literal", "value": "Name: " },
            { "type": "contextRef", "property": "name" }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "name": "Ada",
        "role": "Engineer"
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("Name: Ada"));
    assert_eq!(prompt.matches("Ada").count(), 1);
    assert!(prompt.contains("# ADDITIONAL CONTEXT"));
    assert!(prompt.contains("role: Engineer"));
    assert!(!prompt.contains("name: Ada\nrole: Engineer"));
}

#[test]
fn conditional_context_stays_in_additional_context_when_not_rendered() {
    let ir = build_ir(json!({
        "type": "parts",
        "value": [
            { "type": "literal", "value": "Hello" },
            {
                "type": "inlineIf",
                "condition": {
                    "type": "contextRef",
                    "property": "is_vip"
                },
                "then": [
                    { "type": "literal", "value": "\nVIP note: " },
                    { "type": "contextRef", "property": "vip_note" }
                ],
                "else": []
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "is_vip": false,
        "vip_note": "gold-tier",
        "region": "EU"
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("Hello"));
    assert!(prompt.contains("is_vip: false"));
    assert!(prompt.contains("vip_note: gold-tier"));
    assert!(prompt.contains("region: EU"));
}

#[test]
fn numeric_context_in_rendered_conditional_branch_keeps_prompt_text() {
    let ir = build_ir(json!({
        "type": "template",
        "value": [
            { "type": "literal", "value": "You are a helpful assistant\n" },
            {
                "type": "inlineIf",
                "condition": {
                    "type": "comparison",
                    "operator": ">",
                    "left": {
                        "type": "memberAccess",
                        "object": { "type": "varRef", "value": "ctx" },
                        "properties": ["age"]
                    },
                    "right": { "type": "literal", "value": 20.0 }
                },
                "then": [
                    { "type": "literal", "value": "The person is old " },
                    {
                        "type": "memberAccess",
                        "object": { "type": "varRef", "value": "ctx" },
                        "properties": ["age"]
                    }
                ],
                "else": [
                    { "type": "literal", "value": "not that old" }
                ]
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "age": 25.4,
        "user_name": "Amihere"
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("You are a helpful assistant"));
    assert!(prompt.contains("The person is old 25.4"));
    assert!(!prompt.contains("age: 25.4"));
    assert!(prompt.contains("user_name: Amihere"));
}

#[test]
fn condition_only_context_is_not_treated_as_rendered() {
    let ir = build_ir(json!({
        "type": "template",
        "value": [
            { "type": "literal", "value": "You are a helpful assistant\n" },
            {
                "type": "inlineIf",
                "condition": {
                    "type": "comparison",
                    "operator": ">",
                    "left": {
                        "type": "memberAccess",
                        "object": { "type": "varRef", "value": "ctx" },
                        "properties": ["age"]
                    },
                    "right": { "type": "literal", "value": 20.0 }
                },
                "then": [
                    { "type": "literal", "value": "The person is old" }
                ],
                "else": [
                    { "type": "literal", "value": "not that old" }
                ]
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "age": 18,
        "user_name": "Amihere"
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("You are a helpful assistant"));
    assert!(prompt.contains("not that old"));
    assert!(prompt.contains("age: 18"));
    assert!(prompt.contains("user_name: Amihere"));
}
