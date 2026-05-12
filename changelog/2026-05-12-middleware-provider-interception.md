# Middleware Provider Request Interception

Date: 2026-05-12

## Summary

Middleware can now intercept and mutate LLM provider requests before they are sent. This enables:

- **Dynamic auth**: Inject `Authorization` headers per-request (e.g. subscription tokens)
- **Config overrides**: Change temperature, maxTokens, or any provider config at runtime
- **Provider switching**: Route to a different driver (OpenAI → Gemini) on the fly
- **URL override**: Proxy through custom endpoints per-request
- **Retry/fallback**: `onError` can return `{ forceStart: "llm_start" }` to retry the current turn, or `{ forceStart: "run_start" }` to restart the entire run

## API

### `onLLMStart` — mutate the outgoing request

Return an object (previously only `string` prompt replacement was supported):

```ts
middleware({
  name: "auth",
  async onLLMStart(prompt, ctx) {
    ctx.headers = { Authorization: `Bearer ${await getToken()}` };
    // optionally also mutate:
    // ctx.config = { temperature: 0.2 };
    // ctx.provider = "openai";
    // ctx.url = "https://proxy.example.com/v1";
  },
});
```

New fields available on `ctx` during `onLLMStart`:
- `model` — model name for this turn
- `provider` — provider ID (e.g. `"gemini"`, `"openai"`)
- `config` — provider config object (temperature, etc.)
- `url` — custom provider URL if applicable
- `headers` — HTTP headers (readable + writable across middleware chain)

Return type (backward compatible):
- `string` — replace prompt text
- `{ prompt?, stack?, config?, provider?, url?, headers? }` — mutate request fields
- `void` — no changes

### `onError` — swallow or retry

```ts
middleware({
  name: "retry",
  async onError(error, session, ctx) {
    ctx.retryCount = (ctx.retryCount || 0) + 1;
    if (ctx.retryCount > 3) return { swallow: false }; // propagate

    ctx.config = { ...ctx.config, model: "fallback-model" };
    return { forceStart: "llm_start" }; // retry this turn
  },
});
```

Return type (backward compatible):
- `true` / `false` — swallow or propagate
- `{ swallow?: boolean, forceStart?: "llm_start" | "run_start" }` — fine-grained control

### Retry limits

`forceStart` retries are capped at **5 consecutive failures** per run to prevent infinite loops. The counter resets on a successful stream start.

## Rust Runtime Changes

- `EventContext` extended with `model`, `provider`, `config`, `url`, `headers`
- `ModelDriver` trait: `stream_generate`, `embed`, `embed_batch` now accept `headers: Option<Value>`
- `OpenAIDriver` and `GeminiDriver`: apply middleware headers to HTTP requests; skip default `bearer_auth` when `Authorization` header is present
- `runtime_loop.rs`: deep-merge middleware `config`, switch `provider`, inject `url` into driver config, pass `headers` to driver
- `parse_llm_start_response`: handles bare `string` returns for backward compatibility
- `parse_error_response`: extracts `swallow` and `forceStart`
- `force_start_retry_count` on `AuwgentEngine` with `MAX_FORCE_START_RETRIES = 5`
- `SessionState.pop_last_turn_if_empty()` for cleaning up empty turns after retry

## Target SDK Changes

### TypeScript
- `MiddlewareContext` gains `model?`, `provider?`, `config?`, `url?`, `headers?`
- `MiddlewareLLMStartResult` and `MiddlewareErrorResult` types added
- `buildContextFromRuntimeEvent` hydrates all new fields from runtime event
- `handleMiddlewareEvent` serializes new return fields back to Rust

### Python
- `MiddlewareContext` gains `model`, `provider`, `config`, `url`, `headers`
- `MiddlewareLLMStartResult` and `MiddlewareErrorResult` TypedDicts added
- `_build_context_from_runtime_event` hydrates new fields
- `llm_start` and `error` handlers support object returns

### Dart
- `MiddlewareContext` gains `model`, `provider`, `config`, `url`, `headers`
- `MiddlewareLLMStartResult` and `MiddlewareErrorResult` classes added
- `_buildContextFromRuntimeEvent` hydrates new fields
- `_handleMiddlewareEvent` handles object returns for `llm_start` and `error`

### Rust target SDK
- `DeterministicDriver` updated with new `headers` parameter on `ModelDriver` trait methods

## TypeScript Input Type Fix

The TypeScript codegen now explicitly types the `input` field in the generated IR type:

```ts
type VisionIR = Omit<typeof _importedIR, "name" | "workflows" | "helpers" | "input"> & {
  name: "Vision";
  workflows: undefined;
  helpers: undefined;
  input: "image"; // ← explicit literal type
};
```

This fixes `ExtractInputShape<IR>` resolving to `string` instead of the media array type when `_importedIR` comes from a JSON import (which types all JSON strings as `string`, losing literal types).

Before this fix:
```ts
// Error: Argument of type '(AuwgentTextPart | AuwgentImagePart)[]' is not assignable to parameter of type 'string'
await agent.run([input.text("..."), input.image({ url: "..." })]);
```

After this fix, `agent.run()` correctly accepts the `Input` array type for media agents.

## Files Changed

Compiler:
- `auwgent-compiler/crates/auwgent-codegen/src/typescript.rs`

Runtime:
- `ir-runtime/src/runtime/middleware_event.rs`
- `ir-runtime/src/runtime/middleware.rs`
- `ir-runtime/src/runtime/drivers/mod.rs`
- `ir-runtime/src/runtime/drivers/openai.rs`
- `ir-runtime/src/runtime/drivers/gemini.rs`
- `ir-runtime/src/runtime/engine.rs`
- `ir-runtime/src/runtime/engine/runtime_loop.rs`
- `ir-runtime/src/runtime/session.rs`
- `ir-runtime/tests/middleware_provider_interception_test.rs`
- `ir-runtime/tests/native_terminal_intent_test.rs`
- `ir-runtime/tests/multimodal_engine_run_test.rs`

Target SDKs:
- `targets/typescript/middleware.ts`
- `targets/typescript/auwgent.ts`
- `targets/python/auwgent_sdk/__init__.py`
- `targets/dart/lib/src/middleware.dart`
- `targets/dart/lib/src/typed_auwgent.dart`
- `targets/rust/src/lib.rs`

## Verification

```sh
cargo test --manifest-path ir-runtime/Cargo.toml
cargo test --manifest-path auwgent-compiler/Cargo.toml -p auwgent-codegen
cargo check --manifest-path targets/wasm-runtime/Cargo.toml --target wasm32-unknown-unknown
cargo check --manifest-path c-abi/Cargo.toml
cargo check --manifest-path targets/rust/Cargo.toml
```
