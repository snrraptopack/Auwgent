use crate::tool_callback::AuwgentFreeCallback;
use ir_runtime::runtime::engine::IntentControl;
use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

pub type AuwgentMiddlewareEventCallback =
    unsafe extern "C" fn(event_json: *const c_char, user_data: *mut c_void) -> *mut c_char;

pub type AuwgentAsyncMiddlewareEventCallback = unsafe extern "C" fn(
    request_id: *const c_char,
    event_json: *const c_char,
    user_data: *mut c_void,
);

#[derive(Clone, Copy)]
pub struct AsyncMiddlewareEventCallbackRegistration {
    pub callback: AuwgentAsyncMiddlewareEventCallback,
    pub user_data: usize,
}

// SAFETY: this carries host-managed function pointers and opaque host state.
unsafe impl Send for AsyncMiddlewareEventCallbackRegistration {}
// SAFETY: host is responsible for ensuring thread-safe use of the callback/user_data.
unsafe impl Sync for AsyncMiddlewareEventCallbackRegistration {}

pub type AuwgentIntentCallback = unsafe extern "C" fn(
    intent_name: *const c_char,
    value_json: *const c_char,
    agent_name: *const c_char,
    user_data: *mut c_void,
) -> *mut c_char;

pub type AuwgentPartialIntentCallback = unsafe extern "C" fn(
    intent_name: *const c_char,
    value_json: *const c_char,
    agent_name: *const c_char,
    user_data: *mut c_void,
);

pub type AuwgentSessionTransformCallback = unsafe extern "C" fn(
    primary_name: *const c_char,
    session_json: *const c_char,
    user_data: *mut c_void,
) -> *mut c_char;

pub type AuwgentSessionNotifyCallback = unsafe extern "C" fn(
    primary_name: *const c_char,
    session_json: *const c_char,
    user_data: *mut c_void,
);

#[derive(Clone, Copy)]
pub struct JsonCallbackRegistration<C> {
    pub callback: C,
    pub free_result: Option<AuwgentFreeCallback>,
    pub user_data: *mut c_void,
}

// SAFETY: this carries host-managed function pointers and opaque host state.
unsafe impl<C> Send for JsonCallbackRegistration<C> {}
// SAFETY: host is responsible for ensuring thread-safe use of the callback/user_data.
unsafe impl<C> Sync for JsonCallbackRegistration<C> {}

fn copy_optional_json_result(
    result_ptr: *mut c_char,
    free_result: Option<AuwgentFreeCallback>,
    user_data: *mut c_void,
) -> Result<Option<String>, String> {
    if result_ptr.is_null() {
        return Ok(None);
    }

    let result = (|| {
        let result = unsafe { CStr::from_ptr(result_ptr) }
            .to_str()
            .map_err(|_| "callback returned invalid UTF-8".to_string())?;
        Ok::<Option<String>, String>(Some(result.to_string()))
    })();

    if let Some(free_result) = free_result {
        unsafe { free_result(result_ptr, user_data) };
    }

    result
}

fn make_cstring(label: &str, value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{label} contained interior null byte"))
}

impl JsonCallbackRegistration<AuwgentMiddlewareEventCallback> {
    pub fn invoke_middleware_event(&self, event_json: &str) -> Result<Option<String>, String> {
        let event_json_c = make_cstring("event json", event_json)?;
        let result_ptr = unsafe { (self.callback)(event_json_c.as_ptr(), self.user_data) };
        copy_optional_json_result(result_ptr, self.free_result, self.user_data)
    }
}

impl JsonCallbackRegistration<AuwgentIntentCallback> {
    pub fn invoke_intent(
        &self,
        intent_name: &str,
        value_json: &str,
        agent_name: &str,
    ) -> Result<Option<String>, String> {
        let intent_name_c = make_cstring("intent name", intent_name)?;
        let value_json_c = make_cstring("intent value json", value_json)?;
        let agent_name_c = make_cstring("agent name", agent_name)?;
        let result_ptr = unsafe {
            (self.callback)(
                intent_name_c.as_ptr(),
                value_json_c.as_ptr(),
                agent_name_c.as_ptr(),
                self.user_data,
            )
        };
        copy_optional_json_result(result_ptr, self.free_result, self.user_data)
    }
}

impl JsonCallbackRegistration<AuwgentPartialIntentCallback> {
    pub fn invoke_partial_intent(
        &self,
        intent_name: &str,
        value_json: &str,
        agent_name: &str,
    ) -> Result<(), String> {
        let intent_name_c = make_cstring("intent name", intent_name)?;
        let value_json_c = make_cstring("intent value json", value_json)?;
        let agent_name_c = make_cstring("agent name", agent_name)?;
        unsafe {
            (self.callback)(
                intent_name_c.as_ptr(),
                value_json_c.as_ptr(),
                agent_name_c.as_ptr(),
                self.user_data,
            )
        };
        Ok(())
    }
}

impl JsonCallbackRegistration<AuwgentSessionTransformCallback> {
    pub fn invoke_session_transform(
        &self,
        primary_name: &str,
        session_json: &str,
    ) -> Result<Option<String>, String> {
        let primary_name_c = make_cstring("primary name", primary_name)?;
        let session_json_c = make_cstring("session json", session_json)?;
        let result_ptr =
            unsafe { (self.callback)(primary_name_c.as_ptr(), session_json_c.as_ptr(), self.user_data) };
        copy_optional_json_result(result_ptr, self.free_result, self.user_data)
    }
}

impl JsonCallbackRegistration<AuwgentSessionNotifyCallback> {
    pub fn invoke_session_notify(&self, primary_name: &str, session_json: &str) -> Result<(), String> {
        let primary_name_c = make_cstring("primary name", primary_name)?;
        let session_json_c = make_cstring("session json", session_json)?;
        unsafe { (self.callback)(primary_name_c.as_ptr(), session_json_c.as_ptr(), self.user_data) };
        Ok(())
    }
}

pub fn parse_intent_control_json(result: Option<String>) -> Result<Option<IntentControl>, String> {
    let Some(result) = result else {
        return Ok(None);
    };

    let val: Value =
        serde_json::from_str(&result).map_err(|err| format!("intent callback returned invalid JSON: {err}"))?;

    Ok(match val {
        Value::Null => None,
        Value::Object(obj) => {
            if obj.get("skip").and_then(Value::as_bool) == Some(true) {
                Some(IntentControl::Skip)
            } else if let Some(result) = obj.get("result") {
                Some(IntentControl::Override {
                    result: result.clone(),
                })
            } else {
                None
            }
        }
        _ => None,
    })
}
