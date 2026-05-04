use crate::error::{clear_last_error, set_last_error};
use crate::ffi_string::{into_c_string, nullable_cstr, required_cstr};
use crate::host_callback::{
    AsyncMiddlewareEventCallbackRegistration, AuwgentAsyncMiddlewareEventCallback,
    AuwgentIntentCallback, AuwgentMiddlewareEventCallback, AuwgentPartialIntentCallback,
    AuwgentSessionNotifyCallback, AuwgentSessionTransformCallback, JsonCallbackRegistration,
    parse_intent_control_json,
};
use crate::json::{parse_optional_json, parse_optional_stack};
use crate::tool_callback::{
    AsyncToolCallbackRegistration, AuwgentAsyncToolCallback, AuwgentFreeCallback,
    AuwgentRunCompleteCallback, AuwgentToolCallback, PendingAsyncToolCalls,
    RunCompleteCallbackRegistration, ToolCallbackRegistration, tool_callback_error,
};
use ir_runtime::runtime::bridge::EngineBridge;
use serde_json::Value;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::sync::Arc;

pub struct EngineHandle {
    pub bridge: EngineBridge,
    pub pending_async_tools: Arc<PendingAsyncToolCalls>,
    pub pending_async_middleware_events: Arc<PendingAsyncToolCalls>,
}

fn with_bridge<T>(
    handle: *mut EngineHandle,
    f: impl FnOnce(&EngineBridge) -> Result<T, String>,
) -> Result<T, String> {
    if handle.is_null() {
        return Err("engine handle was null".to_string());
    }

    // SAFETY: null checked above; caller owns a valid handle allocated by this library.
    let handle = unsafe { &*handle };
    f(&handle.bridge)
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_new(ir_json: *const c_char) -> *mut EngineHandle {
    clear_last_error();

    let result: Result<*mut EngineHandle, String> = (|| {
        let ir_json = required_cstr(ir_json, "ir_json")?;
        let bridge = EngineBridge::new(ir_json)?;
        Ok(Box::into_raw(Box::new(EngineHandle {
            bridge,
            pending_async_tools: Arc::new(PendingAsyncToolCalls::new()),
            pending_async_middleware_events: Arc::new(PendingAsyncToolCalls::new()),
        })))
    })();

    match result {
        Ok(handle) => handle,
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_generate_prompt_from_ir(
    ir_json: *const c_char,
    context_json: *const c_char,
    helper_name: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let result: Result<String, String> = (|| {
        let ir_json = required_cstr(ir_json, "ir_json")?;
        let context = parse_optional_json(context_json, "context")?;
        let helper_name = nullable_cstr(helper_name)?;
        EngineBridge::generate_prompt_from_ir(ir_json, context, helper_name)
    })();

    match result {
        Ok(prompt) => into_c_string(prompt),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_free(handle: *mut EngineHandle) {
    if handle.is_null() {
        return;
    }

    // SAFETY: handle must have been created by Box::into_raw in auwgent_engine_new.
    unsafe {
        let _ = Box::from_raw(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_set_context(
    handle: *mut EngineHandle,
    context_json: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let context =
            parse_optional_json(context_json, "context")?.unwrap_or_else(|| serde_json::json!({}));
        bridge.set_context(context);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_set_gemini_driver(
    handle: *mut EngineHandle,
    api_key: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let api_key = required_cstr(api_key, "api_key")?;
        bridge.set_gemini_driver(api_key);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_set_openai_driver(
    handle: *mut EngineHandle,
    api_key: *const c_char,
    base_url: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let api_key = required_cstr(api_key, "api_key")?;
        let base_url = nullable_cstr(base_url)?;
        bridge.set_openai_driver(api_key, base_url);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_set_groq_driver(
    handle: *mut EngineHandle,
    api_key: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let api_key = required_cstr(api_key, "api_key")?;
        bridge.set_groq_driver(api_key);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_set_custom_driver(
    handle: *mut EngineHandle,
    driver_id: *const c_char,
    api_key: *const c_char,
    base_url: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let driver_id = required_cstr(driver_id, "driver_id")?;
        let api_key = required_cstr(api_key, "api_key")?;
        let base_url = required_cstr(base_url, "base_url")?;
        bridge.set_custom_driver(driver_id, api_key, base_url);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_generate_prompt(
    handle: *mut EngineHandle,
    helper_name: *const c_char,
) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let helper_name = nullable_cstr(helper_name)?;
        bridge.generate_prompt(helper_name)
    }) {
        Ok(prompt) => into_c_string(prompt),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_export_session(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| bridge.export_session()) {
        Ok(session) => into_c_string(session),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_import_session(
    handle: *mut EngineHandle,
    session_json: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let session_json = required_cstr(session_json, "session_json")?;
        bridge.import_session(session_json)
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_clear_session(handle: *mut EngineHandle) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        bridge.clear_session();
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_run_text(
    handle: *mut EngineHandle,
    input_text: *const c_char,
    initial_stack_json: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let input = nullable_cstr(input_text)?.map(Value::String);
        let initial_stack = parse_optional_stack(initial_stack_json)?;
        bridge
            .rt
            .block_on(bridge.run_async(input, initial_stack))
            .map(|_| ())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_run_json(
    handle: *mut EngineHandle,
    input_json: *const c_char,
    initial_stack_json: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let input = parse_optional_json(input_json, "input")?;
        let initial_stack = parse_optional_stack(initial_stack_json)?;
        bridge
            .rt
            .block_on(bridge.run_async(input, initial_stack))
            .map(|_| ())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_run_text_async(
    handle: *mut EngineHandle,
    input_text: *const c_char,
    initial_stack_json: *const c_char,
    on_complete: Option<AuwgentRunCompleteCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let result: Result<(), String> = (|| {
        let input = nullable_cstr(input_text)?.map(Value::String);
        let initial_stack = parse_optional_stack(initial_stack_json)?;
        let on_complete = on_complete.ok_or_else(|| "on_complete callback was null".to_string())?;
        let registration = RunCompleteCallbackRegistration {
            callback: on_complete,
            user_data,
        };

        let handle_ref = unsafe { &*handle };
        let bridge = handle_ref.bridge.clone();
        let rt = bridge.rt.clone();

        rt.spawn(async move {
            match bridge.run_async(input, initial_stack).await {
                Ok(_) => registration.invoke_success(),
                Err(err) => registration.invoke_error(&err),
            }
        });

        Ok(())
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_run_json_async(
    handle: *mut EngineHandle,
    input_json: *const c_char,
    initial_stack_json: *const c_char,
    on_complete: Option<AuwgentRunCompleteCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let result: Result<(), String> = (|| {
        let input = parse_optional_json(input_json, "input")?;
        let initial_stack = parse_optional_stack(initial_stack_json)?;
        let on_complete = on_complete.ok_or_else(|| "on_complete callback was null".to_string())?;
        let registration = RunCompleteCallbackRegistration {
            callback: on_complete,
            user_data,
        };

        let handle_ref = unsafe { &*handle };
        let bridge = handle_ref.bridge.clone();
        let rt = bridge.rt.clone();

        rt.spawn(async move {
            match bridge.run_async(input, initial_stack).await {
                Ok(_) => registration.invoke_success(),
                Err(err) => registration.invoke_error(&err),
            }
        });

        Ok(())
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_process_intents(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        bridge.rt.block_on(bridge.process_intents_async())
    }) {
        Ok(json) => into_c_string(json),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_write_chunk(
    handle: *mut EngineHandle,
    chunk: *const c_char,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let chunk = required_cstr(chunk, "chunk")?;
        bridge.write_chunk(chunk);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_end_stream(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| bridge.end_stream()) {
        Ok(json) => into_c_string(json),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_drain_jsonl(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| Ok(bridge.drain_structured_output_jsonl())) {
        Ok(jsonl) => into_c_string(jsonl),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_drain_jsonl_lines(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        bridge.drain_structured_output_jsonl_lines()
    }) {
        Ok(json) => into_c_string(json),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_get_metadata(handle: *mut EngineHandle) -> *mut c_char {
    clear_last_error();
    match with_bridge(handle, |bridge| bridge.get_metadata()) {
        Ok(json) => into_c_string(json),
        Err(err) => {
            set_last_error(err);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_clear_listeners(handle: *mut EngineHandle) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        bridge.clear_listeners();
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_register_tool_callback(
    handle: *mut EngineHandle,
    tool_name: *const c_char,
    callback: Option<AuwgentToolCallback>,
    free_result: Option<AuwgentFreeCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let tool_name = required_cstr(tool_name, "tool_name")?;
        let callback = callback.ok_or_else(|| "tool callback was null".to_string())?;
        let registration = ToolCallbackRegistration {
            callback,
            free_result,
            user_data,
        };

        let tool_name_for_callback = tool_name.clone();
        let implementation: ir_runtime::runtime::engine::ToolImplementation =
            Arc::new(move |args: Value| {
                let registration = registration;
                let tool_name = tool_name_for_callback.clone();
                Box::pin(async move {
                    let args_json = serde_json::to_string(&args).map_err(|err| {
                        tool_callback_error(&tool_name, &format!("failed to serialize args: {err}"))
                    })?;
                    let result_json = registration
                        .invoke_json(&tool_name, &args_json)
                        .map_err(|err| tool_callback_error(&tool_name, &err))?;
                    serde_json::from_str(&result_json).map_err(|err| {
                        tool_callback_error(&tool_name, &format!("returned invalid JSON: {err}"))
                    })
                })
            });

        bridge.register_tool(&tool_name, implementation);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_register_tool_callback_async(
    handle: *mut EngineHandle,
    tool_name: *const c_char,
    callback: Option<AuwgentAsyncToolCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let handle_ref = unsafe { &*handle };
    let bridge = &handle_ref.bridge;
    let pending_async_tools = handle_ref.pending_async_tools.clone();

    let result: Result<(), String> = (|| {
        let tool_name = required_cstr(tool_name, "tool_name")?;
        let callback = callback.ok_or_else(|| "async tool callback was null".to_string())?;
        let registration = AsyncToolCallbackRegistration {
            callback,
            user_data,
        };

        let tool_name_for_callback = tool_name.clone();
        let implementation: ir_runtime::runtime::engine::ToolImplementation =
            Arc::new(move |args: Value| {
                let registration = registration;
                let pending_async_tools = pending_async_tools.clone();
                let tool_name = tool_name_for_callback.clone();
                Box::pin(async move {
                    let args_json = serde_json::to_string(&args).map_err(|err| {
                        tool_callback_error(&tool_name, &format!("failed to serialize args: {err}"))
                    })?;

                    let (request_id, receiver) = pending_async_tools
                        .create_request()
                        .map_err(|err| tool_callback_error(&tool_name, &err))?;

                    if let Err(err) = registration.invoke(&request_id, &tool_name, &args_json) {
                        let _ = pending_async_tools.cancel(&request_id);
                        return Err(tool_callback_error(&tool_name, &err));
                    }

                    let result_json = receiver.await.map_err(|_| {
                        tool_callback_error(
                            &tool_name,
                            &format!("async tool request `{request_id}` was dropped"),
                        )
                    })?;

                    let result_json =
                        result_json.map_err(|err| tool_callback_error(&tool_name, &err))?;

                    serde_json::from_str(&result_json).map_err(|err| {
                        tool_callback_error(&tool_name, &format!("returned invalid JSON: {err}"))
                    })
                })
            });

        bridge.register_tool(&tool_name, implementation);
        Ok(())
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_complete_tool_call(
    handle: *mut EngineHandle,
    request_id: *const c_char,
    result_json: *const c_char,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let handle_ref = unsafe { &*handle };
    let result: Result<(), String> = (|| {
        let request_id = required_cstr(request_id, "request_id")?;
        let result_json = required_cstr(result_json, "result_json")?;
        handle_ref
            .pending_async_tools
            .complete(&request_id, result_json)
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_fail_tool_call(
    handle: *mut EngineHandle,
    request_id: *const c_char,
    error_message: *const c_char,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let handle_ref = unsafe { &*handle };
    let result: Result<(), String> = (|| {
        let request_id = required_cstr(request_id, "request_id")?;
        let error_message = required_cstr(error_message, "error_message")?;
        handle_ref
            .pending_async_tools
            .fail(&request_id, error_message)
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_middleware_event(
    handle: *mut EngineHandle,
    callback: Option<AuwgentMiddlewareEventCallback>,
    free_result: Option<AuwgentFreeCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let callback = callback.ok_or_else(|| "middleware callback was null".to_string())?;
        let registration = JsonCallbackRegistration {
            callback,
            free_result,
            user_data,
        };

        let handler: ir_runtime::runtime::engine::AsyncMiddlewareEventCallback =
            Arc::new(move |event_json: String| {
                let registration = registration;
                Box::pin(async move {
                    registration
                        .invoke_middleware_event(&event_json)
                        .ok()
                        .flatten()
                })
            });

        bridge.on_middleware_event(handler);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_middleware_event_async(
    handle: *mut EngineHandle,
    callback: Option<AuwgentAsyncMiddlewareEventCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let result: Result<(), String> = (|| {
        let callback = callback.ok_or_else(|| "async middleware callback was null".to_string())?;
        let registration = AsyncMiddlewareEventCallbackRegistration {
            callback,
            user_data: user_data as usize,
        };
        let handle_ref = unsafe { &*handle };
        let bridge = &handle_ref.bridge;
        let pending_async_middleware_events = handle_ref.pending_async_middleware_events.clone();

        let handler: ir_runtime::runtime::engine::AsyncMiddlewareEventCallback =
            Arc::new(move |event_json: String| {
                let pending_async_middleware_events = pending_async_middleware_events.clone();
                let registration = registration;
                Box::pin(async move {
                    let (request_id, receiver) =
                        pending_async_middleware_events.create_request().ok()?;

                    let request_id_c = std::ffi::CString::new(request_id.clone()).ok()?.into_raw();
                    let event_json_c = std::ffi::CString::new(event_json).ok()?.into_raw();

                    unsafe {
                        (registration.callback)(
                            request_id_c,
                            event_json_c,
                            registration.user_data as *mut c_void,
                        );
                    }

                    receiver.await.ok()?.ok()
                })
            });

        bridge.on_middleware_event(handler);
        Ok(())
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_complete_middleware_event(
    handle: *mut EngineHandle,
    request_id: *const c_char,
    result_json: *const c_char,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let result: Result<(), String> = (|| {
        let request_id = required_cstr(request_id, "request_id")?;
        let result_json = nullable_cstr(result_json)?.unwrap_or_else(|| "null".to_string());
        let handle_ref = unsafe { &*handle };
        handle_ref
            .pending_async_middleware_events
            .complete(&request_id, result_json)
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_fail_middleware_event(
    handle: *mut EngineHandle,
    request_id: *const c_char,
    error_message: *const c_char,
) -> bool {
    clear_last_error();

    if handle.is_null() {
        set_last_error("engine handle was null".to_string());
        return false;
    }

    let result: Result<(), String> = (|| {
        let request_id = required_cstr(request_id, "request_id")?;
        let error_message =
            nullable_cstr(error_message)?.unwrap_or_else(|| "middleware callback failed".to_string());
        let handle_ref = unsafe { &*handle };
        handle_ref
            .pending_async_middleware_events
            .fail(&request_id, error_message)
    })();

    match result {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_intent(
    handle: *mut EngineHandle,
    callback: Option<AuwgentIntentCallback>,
    free_result: Option<AuwgentFreeCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let callback = callback.ok_or_else(|| "intent callback was null".to_string())?;
        let registration = JsonCallbackRegistration {
            callback,
            free_result,
            user_data,
        };

        let handler: ir_runtime::runtime::engine::AsyncIntentCallback =
            Arc::new(move |name: String, value: Value, agent: String| {
                let registration = registration;
                Box::pin(async move {
                    let value_json = match serde_json::to_string(&value) {
                        Ok(json) => json,
                        Err(_) => return None,
                    };
                    let result = registration
                        .invoke_intent(&name, &value_json, &agent)
                        .ok()?;
                    parse_intent_control_json(result).ok().flatten()
                })
            });

        bridge.on_intent(handler);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_intent_partial(
    handle: *mut EngineHandle,
    callback: Option<AuwgentPartialIntentCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let callback = callback.ok_or_else(|| "partial intent callback was null".to_string())?;
        let registration = JsonCallbackRegistration {
            callback,
            free_result: None,
            user_data,
        };

        let handler: Arc<dyn Fn(String, Value, String) + Send + Sync> =
            Arc::new(move |name: String, value: Value, agent: String| {
                let value_json = match serde_json::to_string(&value) {
                    Ok(json) => json,
                    Err(_) => return,
                };
                let _ = registration.invoke_partial_intent(&name, &value_json, &agent);
            });

        bridge.on_intent_partial(handler);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_sub_engine_start(
    handle: *mut EngineHandle,
    callback: Option<AuwgentSessionTransformCallback>,
    free_result: Option<AuwgentFreeCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let callback = callback.ok_or_else(|| "sub engine start callback was null".to_string())?;
        let registration = JsonCallbackRegistration {
            callback,
            free_result,
            user_data,
        };

        let handler: ir_runtime::runtime::engine::AsyncSessionPreloadCallback =
            Arc::new(move |name: String, session_json: String| {
                let registration = registration;
                Box::pin(async move {
                    registration
                        .invoke_session_transform(&name, &session_json)
                        .ok()
                        .flatten()
                })
            });

        bridge.on_sub_engine_start(handler);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn auwgent_engine_on_sub_engine_complete(
    handle: *mut EngineHandle,
    callback: Option<AuwgentSessionNotifyCallback>,
    user_data: *mut c_void,
) -> bool {
    clear_last_error();
    match with_bridge(handle, |bridge| {
        let callback =
            callback.ok_or_else(|| "sub engine complete callback was null".to_string())?;
        let registration = JsonCallbackRegistration {
            callback,
            free_result: None,
            user_data,
        };

        let handler: ir_runtime::runtime::engine::SessionSaveCallback =
            Arc::new(move |name: String, session_json: String| {
                let registration = registration;
                Box::pin(async move {
                    let _ = registration.invoke_session_notify(&name, &session_json);
                })
            });

        bridge.on_sub_engine_complete(handler);
        Ok(())
    }) {
        Ok(()) => true,
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}
