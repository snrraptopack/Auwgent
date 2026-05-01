use ir_runtime::ComponentDefinition;
/// Integration tests for block protocol
use ir_runtime::runtime::streaming::parser::block_orchestrator::BlockOrchestrator;
use serde_json::json;
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

    orch.write("[response_text]Hello world[/response_text]");
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

    orch.write(
        "[tool_call: fetch_session]\nsession_id: \"sess_123\"\n[/tool_call]\n[tool_call: get_user]\nuser_id: \"usr_456\"\n[/tool_call]",
    );
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
    orch.register_output_shape(
        &json!({
            "session_id": { "type": "string", "optional": false },
            "user": {
                "type": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "optional": false },
                        "name": { "type": "string", "optional": false }
                    }
                },
                "optional": false
            }
        }),
        None,
    );

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write(
        "[schema: Output]\nsession_id: \"sess_123\"\nuser_id: \"usr_456\"\nuser_name: \"Nana\"\n[/schema]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "response_schema");
    assert_eq!(results[0].1["type"], "Output");
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

    orch.write(
        "[workflow_call: process_data]\ninput: \"test\"\nconfig: { timeout: 30 }\n[/workflow]",
    );
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

    orch.write("[helper_call: StoryTeller]\ncity: \"Accra\"\ndays: 3\n[/helper]");
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
[response_text]
Let me fetch that data.
[/response_text]

[tool_call: fetch_session]
session_id: "sess_123"
[/tool_call]
[tool_call: get_user]
user_id: "usr_456"
[/tool_call]

[response_text]
Here's the result.
[/response_text]
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

[tool_call: fetch]
id: "123"
[/tool_call]

Here's the result.
"#;

    orch.write(input);
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 3); // implicit chat + tool + implicit chat
    assert_eq!(results[0].0, "response_text");
    assert!(
        results[0].1["text"]
            .as_str()
            .unwrap()
            .contains("Let me help")
    );
    assert_eq!(results[1].0, "tool_call");
    assert_eq!(results[2].0, "response_text");
    assert!(
        results[2].1["text"]
            .as_str()
            .unwrap()
            .contains("Here's the result")
    );
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

    orch.write("[response_text]\nHello\n[tool_call: fetch]\nid: \"123\"\n[/tool_call]");
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

    orch.write(
        "[response_text]First attempt[/response_text][response_text]Second attempt[/response_text][response_text]Final answer[/response_text]",
    );
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

    orch.write(
        "[custom: ask_user]\nquestion: \"Are you sure?\"\noptions: [\"yes\", \"no\"]\n[/custom]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "ask_user");
    assert_eq!(results[0].1["question"], "Are you sure?");
}

#[test]
fn test_component_block_reconstructs_props_and_action() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("component");
    let component: ComponentDefinition = serde_json::from_value(json!({
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
    }))
    .expect("valid component def");
    orch.register_component_shape(&component, None);

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write(
        "[component: Button, c_id:\"confirm_order_button\"]\nlabel: \"Confirm\"\nvariant: \"primary\"\naction_onclick: delete_user(id: \"usr_123\")\n[/component]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "component");
    assert_eq!(results[0].1["type"], "Button");
    assert_eq!(results[0].1["c_id"], "confirm_order_button");
    assert_eq!(results[0].1["props"]["label"], "Confirm");
    assert_eq!(results[0].1["props"]["variant"], "primary");
    assert_eq!(results[0].1["action"]["onclick"]["name"], "delete_user");
    assert_eq!(results[0].1["action"]["onclick"]["args"]["id"], "usr_123");
}

#[test]
fn test_render_component_resolves_children_from_component_registry() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("component");
    orch.register_intent("render_component");

    let button: ComponentDefinition = serde_json::from_value(json!({
        "name": "Button",
        "props": {
            "label": { "type": "string", "optional": false }
        },
        "action": {
            "onclick": [{ "name": "confirm_order" }]
        }
    }))
    .expect("valid button def");
    let card: ComponentDefinition = serde_json::from_value(json!({
        "name": "Card",
        "props": {
            "title": { "type": "string", "optional": false }
        },
        "children": {
            "kind": "only",
            "components": ["Button"]
        }
    }))
    .expect("valid card def");
    orch.register_component_shape(&button, None);
    orch.register_component_shape(&card, None);

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write(
        "[component: Card, c_id:\"checkout_card\"]\ntitle: \"Checkout\"\nchildren: [\"confirm_btn\"]\n[/component]\n\
         [component: Button, c_id:\"confirm_btn\"]\nlabel: \"Confirm\"\naction_onclick: confirm_order\n[/component]\n\
         [render_component]\nroot: \"checkout_card\"\n[/render_component]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[2].0, "render_component");
    assert_eq!(results[2].1["root"], "checkout_card");
    assert_eq!(results[2].1["tree"]["type"], "Card");
    assert_eq!(results[2].1["tree"]["children"][0]["type"], "Button");
    assert_eq!(
        results[2].1["tree"]["children"][0]["action"]["onclick"]["name"],
        "confirm_order"
    );
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

    orch.write(
        "[schema: Result]\nstatus: \"first\"\n[/schema]\n[schema: Result]\nstatus: \"second\"\n[/schema]\n[schema: Result]\nstatus: \"final\"\n[/schema]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1); // Only last one
    assert_eq!(results[0].0, "response_schema");
    assert_eq!(results[0].1["response"]["status"], "final");
}

#[test]
fn test_tool_call_unflattens_nested_args_from_aliases() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");
    orch.register_tool_shape(
        "create_user",
        &json!({
            "profile": {
                "type": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "optional": false },
                        "contact": {
                            "type": {
                                "type": "object",
                                "properties": {
                                    "email": { "type": "string", "optional": false }
                                }
                            },
                            "optional": false
                        }
                    }
                },
                "optional": false
            }
        }),
        None,
    );

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write(
        "[tool_call: create_user]\nprofile_name: \"Ada\"\nprofile_contact_email: \"ada@test.com\"\n[/tool_call]",
    );
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1["args"]["profile"]["name"], "Ada");
    assert_eq!(
        results[0].1["args"]["profile"]["contact"]["email"],
        "ada@test.com"
    );
}

#[test]
fn test_custom_intent_unflattens_nested_fields_from_aliases() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("Thoughts");
    orch.register_custom_intent_shape(
        "Thoughts",
        &json!({
            "trace": {
                "type": {
                    "type": "object",
                    "properties": {
                        "explain": { "type": "string", "optional": false }
                    }
                },
                "optional": false
            }
        }),
        None,
    );

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[custom: Thoughts]\ntrace_explain: \"Need lookup first\"\n[/custom]");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "Thoughts");
    assert_eq!(results[0].1["trace"]["explain"], "Need lookup first");
}

#[test]
fn test_malformed_close_tag_recovers_and_keeps_next_block() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");
    orch.register_intent("response_text");

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[tool_call: fetch]\nid: \"123\"\n[/workflow]\n[response_text]Done[/response_text]");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "tool_call");
    assert_eq!(results[0].1["args"]["id"], "123");
    assert_eq!(results[1].0, "response_text");
    assert_eq!(results[1].1["text"], "Done");
}

#[test]
fn test_partial_response_text_does_not_emit_incomplete_open_tag() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = Arc::clone(&partials);

    orch.on_intent_partial(Arc::new(move |name, value| {
        partials_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[response_text");
    assert!(partials.lock().unwrap().is_empty());

    orch.write("]Hello");

    let partials = partials.lock().unwrap();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].0, "response_text");
    assert_eq!(partials[0].1["text"], "Hello");
}

#[test]
fn test_partial_response_text_does_not_reemit_same_payload_on_close() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = Arc::clone(&partials);

    orch.on_intent_partial(Arc::new(move |name, value| {
        partials_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[response_text]Hello");
    orch.write("[/response_text]");

    let partials = partials.lock().unwrap();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].0, "response_text");
    assert_eq!(partials[0].1["text"], "Hello");
}

#[test]
fn test_tool_call_preserves_integer_numbers() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[tool_call: user_name]\nid: 123\n[/tool_call]");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "tool_call");
    assert_eq!(results[0].1["args"]["id"], 123);
}

#[test]
fn test_response_text_does_not_leak_stray_closing_tag() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_text");

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[response_text]Hello[/response_text][/response_text]");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "response_text");
    assert_eq!(results[0].1["text"], "Hello");
}

#[test]
fn test_malformed_tool_header_is_not_emitted_as_tool_call() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");
    orch.register_intent("response_text");

    let emitted = Arc::new(Mutex::new(Vec::new()));
    let emitted_clone = Arc::clone(&emitted);

    orch.on_intent_ready(Arc::new(move |name, value| {
        emitted_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[tool_call: user_name To get your name][/tool_call][response_text]Hello Theo[/response_text]");
    orch.end();

    let results = emitted.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "response_text");
    assert_eq!(results[0].1["text"], "Hello Theo");
}

#[test]
fn test_partial_tool_call_emits_structured_payload() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");
    orch.register_tool_shape(
        "create_user",
        &json!({
            "name": { "type": "string", "optional": false },
            "email": { "type": "string", "optional": false }
        }),
        None,
    );

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = Arc::clone(&partials);

    orch.on_intent_partial(Arc::new(move |name, value| {
        partials_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[tool_call: create_user]\nname: \"Ama\"\nemail:");

    let partials = partials.lock().unwrap();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].0, "tool_call");
    assert_eq!(partials[0].1["partial"], true);
    assert_eq!(partials[0].1["mode"], "structured");
    assert_eq!(partials[0].1["type"], "create_user");
    assert_eq!(partials[0].1["args"]["name"], "Ama");
    assert_eq!(partials[0].1["args"]["email"]["$state"], "pending");
}

#[test]
fn test_partial_tool_call_does_not_reemit_same_payload_on_close() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("tool_call");

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = Arc::clone(&partials);

    orch.on_intent_partial(Arc::new(move |name, value| {
        partials_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[tool_call: create_user]\nname: \"Ama\"");
    orch.write("\n[/tool_call]");

    let partials = partials.lock().unwrap();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].0, "tool_call");
    assert_eq!(partials[0].1["type"], "create_user");
    assert_eq!(partials[0].1["args"]["name"], "Ama");
}

#[test]
fn test_partial_response_schema_emits_structured_payload() {
    let mut orch = BlockOrchestrator::new();
    orch.register_intent("response_schema");
    orch.register_output_shape(
        &json!({
            "status": { "type": "string", "optional": false },
            "summary": { "type": "string", "optional": false }
        }),
        None,
    );

    let partials = Arc::new(Mutex::new(Vec::new()));
    let partials_clone = Arc::clone(&partials);

    orch.on_intent_partial(Arc::new(move |name, value| {
        partials_clone.lock().unwrap().push((name, value));
    }));

    orch.write("[schema: Output]\nstatus: \"draft\"\nsummary:");

    let partials = partials.lock().unwrap();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].0, "response_schema");
    assert_eq!(partials[0].1["partial"], true);
    assert_eq!(partials[0].1["mode"], "structured");
    assert_eq!(partials[0].1["response"]["status"], "draft");
    assert_eq!(partials[0].1["response"]["summary"]["$state"], "pending");
}
