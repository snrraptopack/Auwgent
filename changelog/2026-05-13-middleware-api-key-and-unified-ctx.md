# Middleware API Key & Unified Context Mutation

Date: 2026-05-13

## Summary

**Breaking change:** `onLLMStart` now **only returns `string` or `void`** across all SDKs. The dict/object return type (`MiddlewareLLMStartResult`) has been removed. All mutations go through `ctx` directly.

New capability: middleware can now set `ctx.apiKey` (or `ctx.api_key` in Python) to switch API keys when changing providers via fallback.

## Motivation

When `onError` triggers a fallback to a different provider (e.g. Groq → OpenAI), the new provider requires a different API key. Previously there was no way to change the API key at runtime — it was baked into the driver at registration time.

Additionally, supporting both string returns and dict/object returns from `onLLMStart` created unnecessary complexity and inconsistency across language SDKs. Dart and Rust in particular do not handle union return types ergonomically.

## Changes

### Rust Runtime

- `EventContext` extended with `api_key: Option<String>`
- `LlmStartMiddlewareResult` extended with `model: Option<String>` and `api_key: Option<String>`
- `parse_llm_start_response` extracts `model` and `api_key` from middleware JSON response
- `runtime_loop.rs`: model override now read from top-level `model` field instead of `config.model`
- `ModelDriver` trait: `stream_generate`, `embed`, `embed_batch` now accept `api_key: Option<String>`
- `OpenAIDriver`: uses middleware-provided `api_key` if present, otherwise falls back to registered key
- `GeminiDriver`: uses middleware-provided `api_key` if present, otherwise falls back to registered key
- `runtime_loop.rs`: tracks `provider_api_key`, passes it to `stream_generate`

### TypeScript SDK

- **Removed:** `MiddlewareLLMStartResult` interface
- `onLLMStart` return type simplified to `string | void | Promise<string | void>`
- `MiddlewareContext` gains `apiKey?: string`
- `MiddlewareContext` gains `model?: string` and `apiKey?: string`
- `handleMiddlewareEvent` reads `ctx.model` and `ctx.apiKey`, sends `model` and `api_key` back to Rust

### Python SDK

- **Removed:** `MiddlewareLLMStartResult` TypedDict
- `onLLMStart` return type simplified to `Optional[str]`
- `MiddlewareContext` gains `api_key: Optional[str]`
- `MiddlewareContext` gains `model: Optional[str]` and `api_key: Optional[str]`
- `_handle_middleware_event` reads `ctx["model"]` and `ctx["api_key"]`, sends `model` and `api_key` back to Rust

### Dart SDK

- **Removed:** `MiddlewareLLMStartResult` class
- `onLLMStart` return type simplified to `FutureOr<String?>`
- `MiddlewareContext` gains `String? apiKey`
- `MiddlewareContext` gains `String? model` and `String? apiKey`
- `_handleMiddlewareEvent` reads `ctx.model` and `ctx.apiKey`, sends `model` and `api_key` back to Rust

## Migration Guide

Before (TypeScript):
```ts
async onLLMStart(prompt, ctx) {
  return {
    prompt: prompt + "\n[override]",
    config: { temperature: 0.1 },
    provider: "openai",
  };
}
```

After (TypeScript):
```ts
async onLLMStart(prompt, ctx) {
  ctx.config = { temperature: 0.1 };
  ctx.provider = "openai";
  ctx.model = "gpt-4o";
  return prompt + "\n[override]"; // only the prompt is returned
}
```

Before (Python):
```python
async def onLLMStart(self, prompt, ctx):
    return {
        "prompt": prompt + "\n[override]",
        "config": {"temperature": 0.1},
        "provider": "openai",
    }
```

After (Python):
```python
async def onLLMStart(self, prompt, ctx):
    ctx["config"] = {"temperature": 0.1}
    ctx["provider"] = "openai"
    ctx["model"] = "gpt-4o"
    return prompt + "\n[override]"
```

## Fallback with API Key Example

```ts
middleware({
  name: "Fallback",
  async onError(error, session, ctx) {
    if (String(error).includes("429")) {
      ctx.provider = "openai";
      ctx.model = "gpt-4o";
      ctx.apiKey = process.env.OPENAI_API_KEY;
      return { forceStart: "llm_start" };
    }
    return false;
  },
});
```

## Files Changed

Runtime:
- `ir-runtime/src/runtime/middleware_event.rs`
- `ir-runtime/src/runtime/middleware.rs`
- `ir-runtime/src/runtime/drivers/mod.rs`
- `ir-runtime/src/runtime/drivers/openai.rs`
- `ir-runtime/src/runtime/drivers/gemini.rs`
- `ir-runtime/src/runtime/engine.rs`
- `ir-runtime/src/runtime/engine/runtime_loop.rs`

Target SDKs:
- `targets/typescript/middleware.ts`
- `targets/typescript/auwgent.ts`
- `targets/python/auwgent_sdk/__init__.py`
- `targets/dart/lib/src/middleware.dart`
- `targets/dart/lib/src/typed_auwgent.dart`
