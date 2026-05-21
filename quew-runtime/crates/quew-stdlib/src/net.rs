use quew_runtime::value::Value;
use quew_macros::quew_builtin;

#[quew_builtin(
    id = "std.net.fetch",
    decl = r#"!@@function fetch(url: string): { data: string, error: string }"#,
)]
pub fn fetch_url(url: &str) -> Value {
    match reqwest::blocking::get(url) {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                match response.text() {
                    Ok(body) => {
                        let mut result = indexmap::IndexMap::new();
                        result.insert("data".to_string(), Value::String(body));
                        result.insert("error".to_string(), Value::String("".to_string()));
                        Value::Object(result)
                    }
                    Err(e) => {
                        let mut result = indexmap::IndexMap::new();
                        result.insert("data".to_string(), Value::String("".to_string()));
                        result.insert(
                            "error".to_string(),
                            Value::String(format!("failed to read response body: {}", e)),
                        );
                        Value::Object(result)
                    }
                }
            } else {
                let mut result = indexmap::IndexMap::new();
                result.insert("data".to_string(), Value::String("".to_string()));
                result.insert(
                    "error".to_string(),
                    Value::String(format!("HTTP {}", status)),
                );
                Value::Object(result)
            }
        }
        Err(e) => {
            let mut result = indexmap::IndexMap::new();
            result.insert("data".to_string(), Value::String("".to_string()));
            result.insert(
                "error".to_string(),
                Value::String(format!("request failed: {}", e)),
            );
            Value::Object(result)
        }
    }
}
