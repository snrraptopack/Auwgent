# Auwgent Engine: V2 Middleware Architecture & Legacy Deprecation Guide

This guide outlines the critical steps to remove the legacy, individual C-ABI event hooks from the Auwgent Rust Engine and Python/TypeScript SDKs, fully migrating to the cleaner, unified `on_middleware_event` JSON pipeline.

## 1. The Core Architecture Shift
Currently, the engine uses a hybrid of two systems bridging the backend Rust engine to the Python/TypeScript environments:
*   **The Legacy Individual Channels:** Separate NAPI-RS bindings for `on_llm_start`, `on_run_start`, `on_llm_end`, etc.
*   **The V2 Unified Middleware Channel:** A single `on_middleware_event` channel that pushes standard JSON payloads (e.g., `{"type": "llm_start", "prompt": "..."}`) and awaits JSON overrides from the host language plugins.

### Why We Are Removing Legacy
Continuing to maintain both systems leads to bugs where events slip through the cracks. In particular:
*   The Rust engine currently gates certain V2 events inside legacy checks. For example, the `apply_llm_start_middleware` trigger is nested inside an `if let Some(h) = start_handler` block, causing it to never fire when Python SDKs correctly ignore the legacy API.

---

## 2. Deprecation Roadmap: The Rust Engine (`ir-runtime/src/runtime/engine.rs`)

### Step A: Delete the Legacy Handlers
You should completely strip out the legacy lifecycle storage variables and their NAPI/PyO3 bindings:
*   **Remove:** `llm_start_handler`, `llm_end_handler`, `run_start_handler`, `run_complete_handler`, and `error_handler`.
*   *(Note: Keep `on_intent` and `on_intent_partial` for now. Their high-throughput stream characteristics and direct parsing duties might benefit from dedicated channels until WASM adoption is finalized).*

### Step B: Un-Gate the Middleware Events
With the legacy handlers removed, the engine logic will become significantly cleaner. You must ensure that all middleware payloads fire **unconditionally** via `middleware_event_handler`.

**Current Flawed `llm_start` logic:**
```rust
// BAD: Hidden inside start_handler
if let Some(h) = start_handler {
    if let Some(middleware_result) = self.apply_llm_start_middleware(...).await {
         // ...
    }
}
```

**New Clean `llm_start` logic:**
```rust
// GOOD: Fires unconditionally on the unified channel
let context_json = ...;
if let Some(middleware_result) = self.apply_llm_start_middleware(&input_text, &sys_prompt, &self.ir.name).await {
    if let Some(modified) = middleware_result.get("prompt").and_then(|v| v.as_str()) {
        self.session.lock().unwrap().set_input(modified.to_string());
    }
    // ... stack parsing
}
```

---

## 3. Deprecation Roadmap: The Python SDK (`targets/python/auwgent_sdk/__init__.py`)

### Step A: Protect against Protocol Inheritance Overwrites
Python's `Protocol` inheritance creates empty stub methods (like `onRunStart(self) -> None`) even when users don't intend to implement them. The SDK router currently executes these empty stubs, destroying state like the active `session`.

Update the routing loop inside `_handle_middleware_event` to explicitly check if the user returned a valid override, and ignore it if they returned `None`:

```python
# In event_type == "run_start":
try:
    result = await middleware.onRunStart(session, ctx)
    if result is not None:
        session = result
    self._persist_middleware_context(ctx)
except Exception as error: ...
```
*(Apply to any middleware hook that mutates an object down a chain)*

### Step B: Strict None/Null Parsing
In TypeScript, `event.session || {}` safely defaults to an empty object if `event.session` is `null`. In Python, `.get()` respects `None`. Always enforce a defensive check instantly when deserializing the JSON packet from Rust:

```python
session = event.get("session")
if session is None:
    session = {}
session = cast(SessionState, session)
```

## Summary
By deleting the 6+ legacy lifecycle bindings in Rust and strictly enforcing `on_middleware_event` payloads across both ecosystems, you will drastically lighten the FFI bridge. This unified design inherently translates flawlessly to WebAssembly `wasm-bindgen` for your Edge deployment roadmap.
