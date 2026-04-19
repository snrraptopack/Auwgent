# auwgent-sdk-rust

Native Rust target for Auwgent.

This crate wraps `ir-runtime` directly, so it does not need an FFI bridge.
Generated Rust bindings should compose around `TypedAuwgent`, `AuwgentConfig`,
`AuwgentApiKeys`, and the middleware/tool registration traits defined here.
