use crate::error::set_last_error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub fn into_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(s) => s.into_raw(),
        Err(_) => {
            set_last_error("string contained interior null byte");
            ptr::null_mut()
        }
    }
}

pub fn nullable_cstr(ptr: *const c_char) -> Result<Option<String>, String> {
    if ptr.is_null() {
        return Ok(None);
    }

    // SAFETY: caller promises ptr is a valid NUL-terminated string or null.
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| "invalid UTF-8 string".to_string())?;
    Ok(Some(s.to_string()))
}

pub fn required_cstr(ptr: *const c_char, name: &str) -> Result<String, String> {
    nullable_cstr(ptr)?.ok_or_else(|| format!("{name} was null"))
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: ptr must have been returned by CString::into_raw from this library.
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}
