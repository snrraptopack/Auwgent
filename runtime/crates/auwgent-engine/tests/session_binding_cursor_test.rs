use auwgent_session::SessionState;

#[test]
fn binding_cursor_moves_to_result_turn_during_agentic_result_loop() {
    let mut session = SessionState::new();
    session.set_system_prompt("static system");
    session.start_turn("hello");
    session.set_model_response("[tool_call: get_marks]\n[/tool_call]");
    session.start_turn("[result]\nname: get_marks\nargs: {}\nresult: A\n[/result]");

    assert_eq!(session.binding_cursor_turn_index(), Some(1));

    let messages = session
        .to_messages_with_bindings(Some("[binding]\n@@id is \"100\"\n[/binding]".to_string()));

    assert_eq!(messages[0].content, "static system");
    assert_eq!(messages[1].content, "hello");
    assert_eq!(messages[2].content, "[tool_call: get_marks]\n[/tool_call]");
    assert_eq!(
        messages[3].content,
        "[binding]\n@@id is \"100\"\n[/binding]"
    );
    assert_eq!(
        messages[4].content,
        "[result]\nname: get_marks\nargs: {}\nresult: A\n[/result]"
    );
}

#[test]
fn binding_cursor_moves_when_new_external_user_turn_is_added() {
    let mut session = SessionState::new();
    session.set_system_prompt("static system");
    session.start_turn("hello");
    session.set_model_response("[response_text]done[/response_text]");
    session.start_turn("what is my name?");

    assert_eq!(session.binding_cursor_turn_index(), Some(1));

    let messages = session.to_messages_with_bindings(Some(
        "[binding]\n@@user_name is \"Amihere\"\n[/binding]".to_string(),
    ));

    assert_eq!(messages[1].content, "hello");
    assert_eq!(messages[2].content, "[response_text]done[/response_text]");
    assert_eq!(
        messages[3].content,
        "[binding]\n@@user_name is \"Amihere\"\n[/binding]"
    );
    assert_eq!(messages[4].content, "what is my name?");
}

#[test]
fn binding_cursor_uses_latest_context_before_result_continuation() {
    let mut session = SessionState::new();
    session.set_system_prompt("static system");
    session.start_turn("add 10 to my account");
    session.set_model_response("[tool_call: add_fund]\namount: 10\n[/tool_call]");
    session.start_turn(
        "[result]\nname: add_fund\nargs:\n  amount: 10\nresult:\n  balance: 30.00\n[/result]",
    );

    let messages = session.to_messages_with_bindings(Some(
        "[binding]\n@@balance is \"30.00\"\n[/binding]".to_string(),
    ));

    assert_eq!(messages[1].content, "add 10 to my account");
    assert_eq!(
        messages[3].content,
        "[binding]\n@@balance is \"30.00\"\n[/binding]"
    );
    assert_eq!(
        messages[4].content,
        "[result]\nname: add_fund\nargs:\n  amount: 10\nresult:\n  balance: 30.00\n[/result]"
    );
}
