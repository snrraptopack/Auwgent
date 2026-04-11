use crate::error::set_last_error;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use tokio::sync::oneshot;

pub type AuwgentToolCallback =
    unsafe extern "C" fn(tool_name: *const c_char, args_json: *const c_char, user_data: *mut c_void) -> *mut c_char;

pub type AuwgentAsyncToolCallback = unsafe extern "C" fn(
    request_id: *const c_char,
    tool_name: *const c_char,
    args_json: *const c_char,
    user_data: *mut c_void,
);

pub type AuwgentFreeCallback =
    unsafe extern "C" fn(value: *mut c_char, user_data: *mut c_void);

#[derive(Clone, Copy)]
pub struct ToolCallbackRegistration {
    pub callback: AuwgentToolCallback,
    pub free_result: Option<AuwgentFreeCallback>,
    pub user_data: *mut c_void,
}

#[derive(Clone, Copy)]
pub struct AsyncToolCallbackRegistration {
    pub callback: AuwgentAsyncToolCallback,
    pub user_data: *mut c_void,
}

pub struct PendingAsyncToolCalls {
    next_id: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<String, String>>>>,
}

// SAFETY: this is an opaque host pointer/callback pair. The host is responsible for
// ensuring the pointer remains valid and thread-safe for the lifetime of the registration.
unsafe impl Send for ToolCallbackRegistration {}
// SAFETY: same reasoning as above; the host opts into cross-thread use by registering it.
unsafe impl Sync for ToolCallbackRegistration {}

// SAFETY: the host provides the callback and opaque pointer and is responsible for keeping
// them valid and safe for cross-thread invocation.
unsafe impl Send for AsyncToolCallbackRegistration {}
// SAFETY: same reasoning as above.
unsafe impl Sync for AsyncToolCallbackRegistration {}

impl PendingAsyncToolCalls {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_request(
        &self,
    ) -> Result<(String, oneshot::Receiver<Result<String, String>>), String> {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_lock()?.insert(request_id.clone(), tx);
        Ok((request_id, rx))
    }

    pub fn complete(&self, request_id: &str, result_json: String) -> Result<(), String> {
        let sender = self
            .pending_lock()?
            .remove(request_id)
            .ok_or_else(|| format!("unknown async tool request id `{request_id}`"))?;
        sender
            .send(Ok(result_json))
            .map_err(|_| format!("async tool request `{request_id}` was already resolved"))
    }

    pub fn fail(&self, request_id: &str, message: String) -> Result<(), String> {
        let sender = self
            .pending_lock()?
            .remove(request_id)
            .ok_or_else(|| format!("unknown async tool request id `{request_id}`"))?;
        sender
            .send(Err(message))
            .map_err(|_| format!("async tool request `{request_id}` was already resolved"))
    }

    pub fn cancel(&self, request_id: &str) -> Result<(), String> {
        self.pending_lock()?.remove(request_id);
        Ok(())
    }

    fn pending_lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, oneshot::Sender<Result<String, String>>>>, String>
    {
        self.pending
            .lock()
            .map_err(|_| "async tool registry lock poisoned".to_string())
    }
}

impl ToolCallbackRegistration {
    pub fn invoke_json(&self, tool_name: &str, args_json: &str) -> Result<String, String> {
        let tool_name_c = CString::new(tool_name)
            .map_err(|_| "tool name contained interior null byte".to_string())?;
        let args_json_c = CString::new(args_json)
            .map_err(|_| "tool args json contained interior null byte".to_string())?;

        let result_ptr = unsafe { (self.callback)(tool_name_c.as_ptr(), args_json_c.as_ptr(), self.user_data) };
        if result_ptr.is_null() {
            return Err(format!("tool callback for `{tool_name}` returned null"));
        }

        let result = (|| {
            let result = unsafe { CStr::from_ptr(result_ptr) }
                .to_str()
                .map_err(|_| format!("tool callback for `{tool_name}` returned invalid UTF-8"))?;
            Ok::<String, String>(result.to_string())
        })();

        if let Some(free_result) = self.free_result {
            unsafe { free_result(result_ptr, self.user_data) };
        }

        result
    }
}

impl AsyncToolCallbackRegistration {
    pub fn invoke(
        &self,
        request_id: &str,
        tool_name: &str,
        args_json: &str,
    ) -> Result<(), String> {
        let request_id_c = CString::new(request_id)
            .map_err(|_| "request id contained interior null byte".to_string())?;
        let tool_name_c = CString::new(tool_name)
            .map_err(|_| "tool name contained interior null byte".to_string())?;
        let args_json_c = CString::new(args_json)
            .map_err(|_| "tool args json contained interior null byte".to_string())?;

        unsafe {
            (self.callback)(
                request_id_c.as_ptr(),
                tool_name_c.as_ptr(),
                args_json_c.as_ptr(),
                self.user_data,
            )
        };

        Ok(())
    }
}

pub fn tool_callback_error(tool_name: &str, message: &str) -> String {
    let msg = format!("tool callback `{tool_name}` failed: {message}");
    set_last_error(msg.clone());
    msg
}
