use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct PartialIntentState {
    response_text_cursor: HashMap<String, String>,
}

impl PartialIntentState {
    pub fn response_text_delta(
        &mut self,
        agent_name: &str,
        name: &str,
        segment: u64,
        text: &str,
    ) -> String {
        let key = format!("{agent_name}:{name}:{segment}");
        let previous = self
            .response_text_cursor
            .get(&key)
            .cloned()
            .unwrap_or_default();

        let delta = if text.starts_with(&previous) {
            text[previous.len()..].to_string()
        } else {
            text.to_string()
        };

        self.response_text_cursor.insert(key, text.to_string());
        delta
    }

    pub fn clear_turn(&mut self) {
        self.response_text_cursor.clear();
    }
}
