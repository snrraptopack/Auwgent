use auwgent_runtime_core::MessageContent;
use auwgent_session::{
    SessionState, display_input_value, input_parts_value,
};
use serde_json::json;

#[test]
fn structured_input_parts_are_persisted_separately_from_display_input() {
    let parts = vec![
        json!({ "type": "text", "text": "What is in this image?" }),
        json!({
            "type": "image",
            "path": "./photo.png",
            "mimeType": "image/png",
            "detail": "auto"
        }),
    ];
    let mut session = SessionState::new();
    session.start_turn_parts(
        "What is in this image?\n[image: ./photo.png]",
        parts.clone(),
    );

    let exported = session.export().expect("session should export");
    let restored = SessionState::import(&exported).expect("session should import");

    assert!(exported.contains("\"inputParts\""));
    assert_eq!(
        restored.turns[0].input,
        "What is in this image?\n[image: ./photo.png]"
    );
    assert_eq!(restored.turns[0].input_parts, Some(parts.clone()));

    let messages = restored.to_messages();
    assert!(matches!(
        &messages[0].content,
        MessageContent::Parts(restored_parts) if restored_parts == &parts
    ));
}

#[test]
fn input_part_arrays_render_compact_display_text_without_losing_parts() {
    let input = json!([
        { "type": "text", "text": "Describe both images" },
        { "type": "image", "path": "./front.png", "mimeType": "image/png" },
        { "type": "image", "path": "./back.png", "mimeType": "image/png" },
    ]);

    assert_eq!(
        display_input_value(&input),
        "Describe both images\n[image: ./front.png]\n[image: ./back.png]"
    );
    assert_eq!(input_parts_value(&input), input.as_array().cloned());
}
