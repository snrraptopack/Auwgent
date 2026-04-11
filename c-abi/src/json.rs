use crate::ffi_string::nullable_cstr;
use serde_json::Value;
use std::os::raw::c_char;

pub fn parse_optional_json(ptr: *const c_char, name: &str) -> Result<Option<Value>, String> {
    match nullable_cstr(ptr)? {
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|err| format!("failed to parse {name} JSON: {err}")),
        None => Ok(None),
    }
}

pub fn parse_optional_stack(ptr: *const c_char) -> Result<Option<Vec<String>>, String> {
    let Some(raw) = nullable_cstr(ptr)? else {
        return Ok(None);
    };

    serde_json::from_str(&raw).map_err(|err| format!("failed to parse initial stack JSON: {err}"))
}
