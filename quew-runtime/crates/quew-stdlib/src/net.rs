//! `std.net.fetch` — blocking HTTP with method, headers, body, and timeout.
//!
//! Returns a structured `FetchResponse` record:
//! `{ status: number, ok: bool, body: string, error: string }`
//!
//! `error` is empty on success; transport failures set `error`, keep
//! `status = 0`, and leave `body` empty. Non-2xx responses are NOT errors —
//! they populate `status`/`body` normally so callers can branch on `ok`.

use quew_runtime::native::{NativeEntry, NativeError, NativeHandler};
use quew_runtime::value::Value;

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

/// Registered manually (not via `#[quew_builtin]`) because fetch takes an
/// optional second argument, which the fixed-arity macro cannot express.
fn fetch_impl(args: &[Value]) -> Result<Value, NativeError> {
    let Value::String(url) = args.first().unwrap_or(&Value::Null) else {
        return Err(NativeError::new("fetch: first argument must be a string url"));
    };

    let config = match args.get(1) {
        None | Some(Value::Null) => None,
        Some(Value::Object(_)) => args.get(1),
        Some(other) => {
            return Err(NativeError::new(format!(
                "fetch: config must be an object, found {}",
                other.type_name()
            )));
        }
    };
    let config = config.and_then(Value::as_object);

    let str_field = |obj: &indexmap::IndexMap<String, Value>, key: &str| -> Option<String> {
        match obj.get(key) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    };

    let method = config
        .and_then(|c| str_field(c, "method"))
        .unwrap_or_else(|| "GET".to_string())
        .to_uppercase();

    let timeout_seconds = match config.and_then(|c| c.get("timeout_seconds")) {
        Some(Value::Number(n)) if *n > 0 => *n as u64,
        _ => DEFAULT_TIMEOUT_SECONDS,
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|e| NativeError::new(format!("fetch: failed to build HTTP client: {e}")))?;

    let method = reqwest::Method::from_bytes(method.as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut request = client.request(method, url.as_str());

    if let Some(Value::Object(headers)) = config.and_then(|c| c.get("headers")) {
        for (key, value) in headers {
            if let Value::String(v) = value {
                request = request.header(key.as_str(), v.as_str());
            }
        }
    }

    if let Some(body) = config.and_then(|c| str_field(c, "body")) {
        request = request.body(body);
    }

    let response = match request.send() {
        Ok(r) => r,
        Err(e) => return Ok(fetch_error(format!("request failed: {e}"))),
    };

    let status = response.status().as_u16();
    let body = match response.text() {
        Ok(b) => b,
        Err(e) => return Ok(fetch_error(format!("failed to read response body: {e}"))),
    };

    let ok = (200..300).contains(&status);
    Ok(response_record(status, ok, &body, ""))
}

inventory::submit! {
    NativeEntry {
        id: "std.net.fetch",
        handler: NativeHandler::Sync(fetch_impl),
    }
}

fn response_record(status: u16, ok: bool, body: &str, error: &str) -> Value {
    let mut record = indexmap::IndexMap::new();
    record.insert("status".to_string(), Value::Number(status as i64));
    record.insert("ok".to_string(), Value::Bool(ok));
    record.insert("body".to_string(), Value::String(body.to_string()));
    record.insert("error".to_string(), Value::String(error.to_string()));
    Value::Object(record)
}

fn fetch_error(message: impl Into<String>) -> Value {
    response_record(0, false, "", &message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn debug_fetch_error_chain() {
        let args = vec![Value::String("https://example.com".into())];
        let v = fetch_impl(&args).unwrap();
        println!("fetch result: {v}");

        match reqwest::blocking::get("https://example.com") {
            Ok(r) => println!("plain reqwest status: {}", r.status()),
            Err(e) => {
                println!("plain reqwest error: {e}");
                let mut src: Option<&dyn Error> = e.source();
                while let Some(s) = src {
                    println!("  caused by: {s}");
                    src = s.source();
                }
            }
        }
    }
}
