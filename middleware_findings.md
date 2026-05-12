# ir-runtime Middleware Findings & Architecture Review

## Overview
The middleware architecture in `ir-runtime` serves as a bridge between the core Rust engine and host environments (Node.js, WASM, Dart). It allows the host application to intercept, log, or mutate the engine's state during critical lifecycle phases.

### Key Lifecycle Events Tracked:
1. `RunStart` / `RunComplete`
2. `LlmStart` / `LlmEnd`
3. `Intent` (Tools, workflows, components)
4. `Error`

### Communication Protocol
- Uses stringified JSON (`Value` -> `String` -> FFI -> `String` -> `Value`).
- Ensures that FFI boundaries (like Rust to Dart or WASM) do not need complex native struct bindings.

---

## My Worries & Potential Areas for Improvement

Here are the potential bottlenecks and architectural limitations identified in the current implementation, which we can address later:

### 1. Serialization Overhead (JSON Bloat)
On events like `RunStart` and `RunComplete`, the *entire* `session` state is exported to JSON, sent across the FFI boundary, and parsed back. 
- **The Worry**: As a conversation grows over multiple turns, serializing and deserializing the entire session history on every run could introduce significant latency spikes, especially in performance-sensitive environments like Dart or WASM.
- **Potential Solution**: Introduce partial session syncing, diff-based state updates, or make the full session payload "opt-in" (only sent if explicitly requested by the middleware).


### 3. Async Overhead for Passive Events
Every middleware event triggers an `async` callback.
- **The Worry**: While async is necessary for state-mutating events (like bypassing an `Intent` or altering a `prompt`), it adds unnecessary thread-scheduling and cross-boundary async overhead for purely passive events (like `LlmEnd` or `RunComplete` telemetry).
- **Potential Solution**: Split the callback architecture. Provide an async hook for execution-blocking mutations, and a separate, zero-blocking synchronous "fire-and-forget" hook for telemetry.

### 4. Error Swallowing Fallbacks
The error middleware allows the host to intercept an error and return `{"swallow": true}` to silently recover.
- **The Worry**: While swallowing an error prevents a hard crash, there isn't a clear mechanism to provide a fallback/synthetic response to the user when an error is swallowed, which could leave the agent in a confusing state.
- **Potential Solution**: Allow the error middleware to optionally return a fallback string or tool result alongside the swallow command to gracefully continue the conversation.
