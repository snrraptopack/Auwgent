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

fn build_named_ir(name: &str, prompt: serde_json::Value) -> AgentIR {
    let mut ir = build_ir(prompt);
    ir.name = name.to_string();
    ir
}

#[test]
fn referenced_context_is_rendered_as_symbol_without_static_duplication() {
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

    assert!(prompt.contains("Name: @@name"));
    assert!(!prompt.contains("Ada"));
    assert!(!prompt.contains("# ADDITIONAL CONTEXT"));
    assert!(!prompt.contains("role: Engineer"));
}

#[test]
fn set_context_merges_object_context_updates() {
    let ir = build_ir(json!({
        "type": "template",
        "value": [
            { "type": "literal", "value": "Age: " },
            {
                "type": "memberAccess",
                "object": { "type": "varRef", "value": "ctx" },
                "properties": ["age"]
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "age": 100,
        "id": "100",
        "user_name": "Amihere"
    }));
    engine.set_context(json!({
        "location": "Tarkwa",
        "marks": ["A", "B", "D"]
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("Age: @@age"));
    assert!(!prompt.contains("id: '100'"));
    assert!(!prompt.contains("user_name: Amihere"));
    assert!(!prompt.contains("location: Tarkwa"));
    assert!(!prompt.contains("marks:"));
}

#[test]
fn scalar_set_context_is_added_without_replacing_object_context() {
    let ir = build_ir(json!({
        "type": "template",
        "value": [
            { "type": "literal", "value": "Age: " },
            {
                "type": "memberAccess",
                "object": { "type": "varRef", "value": "ctx" },
                "properties": ["age"]
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "age": 100,
        "id": "100"
    }));
    engine.set_context(json!("secret number: 100"));

    let prompt = engine.generate_prompt(None).expect("prompt should render");

    assert!(prompt.contains("Age: @@age"));
    assert!(!prompt.contains("id: '100'"));
    assert!(!prompt.contains("dynamic_context: 'secret number: 100'"));
}

#[test]
fn conditional_context_is_not_appended_to_system_prompt_when_not_rendered() {
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
    assert!(!prompt.contains("is_vip: false"));
    assert!(!prompt.contains("vip_note: gold-tier"));
    assert!(!prompt.contains("region: EU"));
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
    assert!(prompt.contains("The person is old @@age"));
    assert!(!prompt.contains("age: 25.4"));
    assert!(!prompt.contains("user_name: Amihere"));
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
    assert!(!prompt.contains("age: 18"));
    assert!(!prompt.contains("user_name: Amihere"));
}

#[test]
fn binding_cursor_splits_symbol_bindings_from_injected_context() {
    let ir = build_ir(json!({
        "type": "template",
        "value": [
            { "type": "literal", "value": "User: " },
            {
                "type": "memberAccess",
                "object": { "type": "varRef", "value": "ctx" },
                "properties": ["user_name"]
            }
        ]
    }));
    let engine = AuwgentEngine::new(ir);
    engine.set_context(json!({
        "user_name": "Theo",
        "location": "Tarkwa/Accra",
        "marks": ["A", "B", "C"]
    }));

    let prompt = engine.generate_prompt(None).expect("prompt should render");
    assert!(prompt.contains("User: @@user_name"));
    assert!(prompt.contains("latest [binding] block"));

    let exported: serde_json::Value =
        serde_json::from_str(&engine.export_session().expect("session should export"))
            .expect("exported session should be json");
    let input = exported
        .get("bindingCursor")
        .and_then(|cursor| cursor.get("input"))
        .and_then(serde_json::Value::as_str)
        .expect("binding cursor input should exist");

    assert!(input.contains("[binding]"));
    assert!(input.contains("@@user_name is \"Theo\""));
    assert!(input.contains("[injected_context]"));
    assert!(input.contains("location = \"Tarkwa/Accra\""));
    assert!(input.contains("marks = [\"A\",\"B\",\"C\"]"));
    assert!(!input.contains("@@location"));
    assert!(!input.contains("@@marks"));
}

#[test]
fn export_session_refreshes_stale_imported_system_prompt() {
    let old_session = json!({
        "systemPrompt": "Old prompt",
        "turns": [
            {
                "input": "hello",
                "model_response": "[response_text]hello[/response_text]"
            }
        ],
        "stack": ["TestAgent"],
        "initialInput": null
    })
    .to_string();

    let new_ir = build_named_ir(
        "TestAgent",
        json!({
            "type": "literal",
            "value": "New prompt"
        }),
    );
    let new_engine = AuwgentEngine::new(new_ir);
    new_engine
        .import_session(&old_session)
        .expect("old session should import");

    let exported: serde_json::Value =
        serde_json::from_str(&new_engine.export_session().expect("session should export"))
            .expect("export should be json");

    let system_prompt = exported["systemPrompt"]
        .as_str()
        .expect("system prompt should be string");
    assert!(system_prompt.starts_with("New prompt"));
    assert!(!system_prompt.contains("Old prompt"));
}
