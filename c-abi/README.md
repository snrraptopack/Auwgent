# Auwgent C ABI

This crate is the first shared native bridge for experimental language targets.

The goal is simple:

- keep execution, parsing, session state, and streaming logic in Rust
- expose a small, stable C ABI
- let higher-level SDKs in Java, Kotlin, Dart, Go, Swift, C#, and others stay thin

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

## Design Notes

- The ABI is JSON-first at the boundary.
- Rust owns the engine and async runtime.
- Foreign languages hold only opaque handles.
- Returned strings are allocated by Rust and must be freed with `auwgent_string_free`.

## Planned Next Steps

- tool callback registration
- event callback registration
- poll/drain-based event API for languages that do not like callbacks
- optional header generation and per-language wrapper examples
