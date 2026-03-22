/// Parser for @@result blocks (system-injected tool/workflow results)
/// Format: name: {"json": "object"}

use serde_json::Value;

pub fn parse_results(input: &str) -> Vec<(String, Value)> {
    let mut results = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split on first colon
        if let Some(colon_pos) = line.find(':') {
            let name = line[..colon_pos].trim().to_string();
            let json_str = line[colon_pos + 1..].trim();

            // Parse as JSON
            if let Ok(value) = serde_json::from_str(json_str) {
                results.push((name, value));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_result() {
        let input = r#"fetch_session: {"data": "test", "status": "ok"}"#;
        let results = parse_results(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "fetch_session");
        assert_eq!(results[0].1["status"], "ok");
    }

    #[test]
    fn test_multiple_results() {
        let input = r#"
fetch_session: {"data": "test"}
get_user: {"name": "Nana", "id": "usr_123"}
        "#;
        let results = parse_results(input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "fetch_session");
        assert_eq!(results[1].0, "get_user");
        assert_eq!(results[1].1["name"], "Nana");
    }
}
