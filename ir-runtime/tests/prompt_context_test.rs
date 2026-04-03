use ir_runtime::runtime::AuwgentEngine;
use ir_runtime::AgentIR;
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
fn conditional_context_does_not_leak_into_static_context_when_false() {
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
    assert!(!prompt.contains("gold-tier"));
    assert!(!prompt.contains("is_vip"));
    assert!(!prompt.contains("vip_note"));
    assert!(prompt.contains("region: EU"));
}
