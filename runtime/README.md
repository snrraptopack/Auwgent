# Auwgent Runtime

This workspace contains the async runtime that executes compiled Auwgent agent IR (`.agent.json`) against LLM providers.

## Crate Overview

```
auwgent-runtime-core/   # Base types, errors, callback aliases, JSON utilities
auwgent-session/        # SessionState, Turn, BindingCursor, message builders
auwgent-middleware/     # Event enums, payload structs, async fire helpers
auwgent-native/         # NativeCallableRegistry, IR → JSON Schema conversion
auwgent-schema/         # FlatFieldSpec, recursive type resolution
auwgent-evaluator/      # IR expression evaluation, prompt rendering
auwgent-protocol/       # BlockOrchestrator, JsonlEventBuffer, partial intents
auwgent-prompt/         # Block protocol prompt generation
auwgent-drivers/        # ModelDriver trait, OpenAI & Gemini implementations
auwgent-engine/         # Engine shell, runtime loop, execution, prompt building
auwgent-bridge/         # EngineBridge FFI facade
function-parser/        # Standalone bracket-protocol parser
```

## Dependency Graph

```
auwgent-runtime-core
    ← all other crates

auwgent-evaluator ← auwgent-schema, auwgent-ir-schema
auwgent-engine ← auwgent-evaluator, auwgent-protocol, auwgent-prompt,
                 auwgent-drivers, auwgent-session, auwgent-middleware,
                 auwgent-native, auwgent-runtime-core
auwgent-bridge ← auwgent-engine
```

## Testing

- **Unit tests:** Live in `#[cfg(test)]` modules inside source files.
- **Integration tests:** Live in `tests/` directories within each crate.
- **Cross-language tests:** See `../runtime-tests/` for TypeScript/Python/Dart runners.

Run the full workspace test suite:

```bash
cd runtime
cargo test --workspace
```

## WASM Support

All crates compile for `wasm32-unknown-unknown`. Conditional compilation (`#[cfg(target_arch = "wasm32")]`) is used for:

- `async_trait(?Send)` instead of `async_trait`
- `js_sys::Date` instead of `std::time::SystemTime`
- `wasm-bindgen-futures` instead of `tokio`

Build the WASM target:

```bash
cd targets/wasm-runtime
wasm-pack build --target bundler
```

## Adding a New Crate

1. Create the crate under `crates/`
2. Add it to the `[workspace]` members in `Cargo.toml`
3. Depend on `auwgent-runtime-core` for base types
4. Depend on `auwgent-ir-schema` (workspace) for IR types
5. Add unit tests in `src/lib.rs` and integration tests in `tests/` if needed
