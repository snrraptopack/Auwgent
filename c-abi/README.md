# Auwgent C ABI

This crate is the first shared native bridge for experimental language targets.

The goal is simple:

- keep execution, parsing, session state, and streaming logic in Rust
- expose a small, stable C ABI
- let higher-level SDKs in Java, Kotlin, Dart, Go, Swift, C#, and others stay thin

The canonical native contract lives in:

- `include/auwgent.h`

## Initial Surface

The first ABI version exposes:

- engine creation from IR JSON
- engine destruction
- context set from JSON
- prompt generation
- sync `run` wrappers for text or JSON input
- session export/import
- streaming chunk write/end
- structured JSONL drain
- Rust-owned string allocation/free
- thread-local last error retrieval
- C tool callback registration
- middleware event callback registration
- intent callback registration
- partial intent callback registration

## Design Notes

- The ABI is JSON-first at the boundary.
- Rust owns the engine and async runtime.
- Foreign languages hold only opaque handles.
- Returned strings are allocated by Rust and must be freed with `auwgent_string_free`.

## Planned Next Steps

- event callback registration
- poll/drain-based event API for languages that do not like callbacks
- optional header generation and per-language wrapper examples

## Tool Callback Shape

The current tool callback API is:

- register a tool by name
- Rust passes `(tool_name, args_json, user_data)` to the host callback
- the host returns a JSON string for the tool result
- the host may optionally provide a free callback for the returned string buffer

This keeps the foreign SDK thin:

- host language does not need to understand Rust types
- Rust still owns execution flow and tool result parsing
- the host only turns native values into JSON and back

## Host Hook Shape

The ABI now supports three host-facing hook registrations that mirror the existing SDK model:

- middleware event callback
- intent callback
- partial intent callback

These all use JSON strings at the boundary.

- middleware callback:
  - input: event JSON
  - output: optional response JSON
- intent callback:
  - input: intent name, value JSON, agent name
  - output: optional control JSON like `{ "skip": true }` or `{ "result": ... }`
- partial intent callback:
  - input: intent name, value JSON, agent name
  - output: none

Additional lifecycle hooks now supported:

- sub-engine start
- sub-engine complete
- llm start
- llm end
- run start
- run complete
- error callback
