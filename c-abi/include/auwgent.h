#ifndef AUWGENT_H
#define AUWGENT_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct auwgent_engine_handle auwgent_engine_handle;

typedef char* (*auwgent_tool_callback)(
    const char* tool_name,
    const char* args_json,
    void* user_data
);

typedef void (*auwgent_async_tool_callback)(
    const char* request_id,
    const char* tool_name,
    const char* args_json,
    void* user_data
);

typedef void (*auwgent_free_callback)(
    char* value,
    void* user_data
);

typedef char* (*auwgent_middleware_event_callback)(
    const char* event_json,
    void* user_data
);

typedef char* (*auwgent_intent_callback)(
    const char* intent_name,
    const char* value_json,
    const char* agent_name,
    void* user_data
);

typedef void (*auwgent_partial_intent_callback)(
    const char* intent_name,
    const char* value_json,
    const char* agent_name,
    void* user_data
);

typedef char* (*auwgent_session_transform_callback)(
    const char* primary_name,
    const char* session_json,
    void* user_data
);

typedef void (*auwgent_session_notify_callback)(
    const char* primary_name,
    const char* session_json,
    void* user_data
);

typedef char* (*auwgent_llm_start_callback)(
    const char* input_json,
    const char* system_prompt,
    const char* context_json,
    void* user_data
);

typedef void (*auwgent_llm_end_callback)(
    const char* raw_response,
    const char* system_prompt,
    void* user_data
);

typedef bool (*auwgent_error_callback)(
    const char* error_json,
    const char* session_json,
    const char* context_json,
    void* user_data
);

/*
 * Strings returned by this library must be freed with auwgent_string_free.
 * Strings returned by host callbacks may be freed by the optional free callback
 * passed during registration.
 */
void auwgent_string_free(char* ptr);
char* auwgent_last_error_message(void);

auwgent_engine_handle* auwgent_engine_new(const char* ir_json);
void auwgent_engine_free(auwgent_engine_handle* handle);

bool auwgent_engine_set_context(
    auwgent_engine_handle* handle,
    const char* context_json
);

bool auwgent_engine_set_gemini_driver(
    auwgent_engine_handle* handle,
    const char* api_key
);

bool auwgent_engine_set_openai_driver(
    auwgent_engine_handle* handle,
    const char* api_key,
    const char* base_url
);

bool auwgent_engine_set_custom_driver(
    auwgent_engine_handle* handle,
    const char* driver_id,
    const char* api_key,
    const char* base_url
);

char* auwgent_engine_generate_prompt(
    auwgent_engine_handle* handle,
    const char* helper_name
);

char* auwgent_engine_export_session(auwgent_engine_handle* handle);

bool auwgent_engine_import_session(
    auwgent_engine_handle* handle,
    const char* session_json
);

bool auwgent_engine_clear_session(auwgent_engine_handle* handle);

bool auwgent_engine_run_text(
    auwgent_engine_handle* handle,
    const char* input_text,
    const char* initial_stack_json
);

bool auwgent_engine_run_json(
    auwgent_engine_handle* handle,
    const char* input_json,
    const char* initial_stack_json
);

char* auwgent_engine_process_intents(auwgent_engine_handle* handle);

bool auwgent_engine_write_chunk(
    auwgent_engine_handle* handle,
    const char* chunk
);

char* auwgent_engine_end_stream(auwgent_engine_handle* handle);
char* auwgent_engine_drain_jsonl(auwgent_engine_handle* handle);
char* auwgent_engine_drain_jsonl_lines(auwgent_engine_handle* handle);
char* auwgent_engine_get_metadata(auwgent_engine_handle* handle);

bool auwgent_engine_clear_listeners(auwgent_engine_handle* handle);

bool auwgent_engine_register_tool_callback(
    auwgent_engine_handle* handle,
    const char* tool_name,
    auwgent_tool_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_register_tool_callback_async(
    auwgent_engine_handle* handle,
    const char* tool_name,
    auwgent_async_tool_callback callback,
    void* user_data
);

bool auwgent_engine_complete_tool_call(
    auwgent_engine_handle* handle,
    const char* request_id,
    const char* result_json
);

bool auwgent_engine_fail_tool_call(
    auwgent_engine_handle* handle,
    const char* request_id,
    const char* error_message
);

bool auwgent_engine_on_middleware_event(
    auwgent_engine_handle* handle,
    auwgent_middleware_event_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_on_intent(
    auwgent_engine_handle* handle,
    auwgent_intent_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_on_intent_partial(
    auwgent_engine_handle* handle,
    auwgent_partial_intent_callback callback,
    void* user_data
);

bool auwgent_engine_on_sub_engine_start(
    auwgent_engine_handle* handle,
    auwgent_session_transform_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_on_sub_engine_complete(
    auwgent_engine_handle* handle,
    auwgent_session_notify_callback callback,
    void* user_data
);

bool auwgent_engine_on_llm_start(
    auwgent_engine_handle* handle,
    auwgent_llm_start_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_on_llm_end(
    auwgent_engine_handle* handle,
    auwgent_llm_end_callback callback,
    void* user_data
);

bool auwgent_engine_on_run_start(
    auwgent_engine_handle* handle,
    auwgent_session_transform_callback callback,
    auwgent_free_callback free_result,
    void* user_data
);

bool auwgent_engine_on_run_complete(
    auwgent_engine_handle* handle,
    auwgent_session_notify_callback callback,
    void* user_data
);

bool auwgent_engine_on_error(
    auwgent_engine_handle* handle,
    auwgent_error_callback callback,
    void* user_data
);

#ifdef __cplusplus
}
#endif

#endif
