# Auwgent Middleware — Current Capabilities & Design Gap Analysis

## 1. What Our Middleware Can Do Today

Our middleware has **six hooks**, all event-based with JSON serialization across FFI:

| Hook | Fires When | Can Mutate | Return Type |
|------|-----------|------------|-------------|
| `onRunStart` | Before run loop | `session` | `{ session }` |
| `onLLMStart` | Before provider call | `prompt`, `stack` | `{ prompt, stack }` |
| `onLLMEnd` | After stream completes | — (observational) | — |
| `onIntent` | When intent is parsed | Execution | `{ skip: true }` or `{ result }` |
| `onRunComplete` | After run terminates | — (observational) | — |
| `onError` | On engine/tool error | Error propagation | `{ swallow: true }` |

The TypeScript layer provides type-safe narrowing and agent targeting. The Rust layer serializes events to JSON, sends across FFI, awaits the response, and applies mutations.

**This design is good for:**
- Prompt injection / RAG (mutate `prompt` in `onLLMStart`)
- Context truncation (mutate `session.turns` in `onRunStart`)
- Intent blocking / mocking (return `skip` or `result` in `onIntent`)
- Observability (log in `onLLMEnd`, `onRunComplete`)
- Error swallowing (return `swallow: true` in `onError`)

## 2. What Our Middleware Cannot Do Today

This is the critical finding: **there is no hook that can intercept or mutate the actual HTTP request to the LLM provider.**

### 2.1 The Provider Call Is a Black Box

In `runtime_loop.rs`, the engine:

1. Evaluates model config from IR → gets `provider_id`, `model_name`, `config_params`
2. Builds messages (block mode or native mode)
3. Injects native tools/output schema into `config_params`
4. Calls `driver.stream_generate(model_name, &messages, config_params)`

At this point, middleware has already run (`onLLMStart`) and could only mutate `prompt` and `stack`. The `config_params` passed to the driver are **locked**.

Inside the driver (e.g., `openai.rs`):

```rust
let response = self
    .client
    .post(&url)
    .bearer_auth(&self.api_key)  // Hardcoded at driver construction
    .json(&body)
    .send()
    .await?;
```

The driver has **no mechanism** to:
- Accept per-request headers
- Accept per-request body mutations beyond the `config` merge
- Accept URL overrides
- Accept auth token overrides

### 2.2 Consequences

Because of this black box, middleware **cannot** implement:

| Feature | Why It Fails |
|---------|-------------|
| **Subscription auth** | No way to inject `Authorization: Bearer <jwt>` per request |
| **Dynamic header injection** | No header interception point exists |
| **Request proxying** | No URL override point exists |
| **Retry with model fallback** | `onError` fires *after* the stream failed; there's no mechanism to retry the same turn with a different driver/model |
| **Provider-specific cache control** | OpenAI/Anthropic cache headers or config fields can't be injected dynamically |
| **Request/response logging at HTTP layer** | `onLLMStart` only sees the prompt string, not the full request body |

## 3. The Design Decision We Need to Make

To enable provider request interception, we need to decide **where** in the middleware surface to add it. There are two distinct approaches:

### Option A: Extend `onLLMStart` with Request Metadata

Add `model`, `provider`, `config`, and `headers` to `onLLMStart` payload and response.

**What changes:**
- `LlmStartPayload` gets new fields: `model`, `provider`, `config`
- `onLLMStart` response can return `{ prompt, stack, config, headers }`
- `config` is deep-merged into the existing config before driver call
- `headers` are threaded through to the driver and applied to the HTTP request
- Driver trait and implementations updated to accept optional headers

**Pros:**
- Single hook handles everything "before the provider call"
- Non-breaking (old middleware returning `{ prompt }` still works)
- Natural mental model: "I'm about to call the LLM, let me adjust the request"

**Cons:**
- `onLLMStart` becomes a "god hook" with many responsibilities
- `headers` are HTTP-specific but the engine abstracts over providers; feels leaky
- Doesn't solve retry/fallback (still no way to re-run a failed turn from middleware)

### Option B: Add a New `onDriverRequest` Hook

Create a separate hook that fires **after** the request is fully built but **before** it is sent.

**What changes:**
- New event type `DriverRequestPayload` with: `provider`, `url`, `method`, `headers`, `body`
- New middleware hook `onDriverRequest`
- Response shape: `{ headers?, body?, url? }`
- Driver must support receiving request overrides

**Pros:**
- Clean separation: `onLLMStart` = prompt/semantic concerns, `onDriverRequest` = transport concerns
- Full visibility into the actual HTTP request
- Can mutate body directly (e.g., inject Anthropic's `cache_control` into message objects)

**Cons:**
- More FFI traffic (another round-trip)
- Exposes HTTP details into middleware, which breaks abstraction for non-HTTP providers
- Requires larger driver refactoring

### Option C: Driver-Level Middleware (Rust-Only)

Instead of FFI-ing the request out to JS, keep request mutation in Rust. Add a `RequestInterceptor` trait in Rust that middleware can register.

**What changes:**
- New Rust trait: `RequestInterceptor` with `intercept(&mut request)`
- TS middleware cannot access this — only Rust-registered interceptors
- Or: add a `register_request_interceptor` FFI method separate from the event-based middleware

**Pros:**
- Zero FFI overhead for request mutation
- Keeps HTTP details out of JS middleware surface
- Fast — no JSON serialization of large request bodies

**Cons:**
- TypeScript users can't write interceptors in JS
- Breaks our "middleware is TS-first" DX model
- Requires users to write Rust or use a separate registration mechanism

## 4. The Retry/Fallback Problem (Separate Concern)

Even with request interception, **retry and fallback middleware are not possible with our current architecture** because:

1. The `run()` loop in `runtime_loop.rs` handles errors by returning `Err(error)`
2. `onError` fires, but the engine is already in an error state
3. There is no mechanism to say "try this same turn again with different config"

Genkit's retry middleware works because their middleware wraps the `model` call directly:

```ts
// Genkit
model: async (req, ctx, next) => {
  for (let i = 0; i < maxRetries; i++) {
    try { return await next(req, ctx); }
    catch (e) { if (!shouldRetry(e)) throw e; }
  }
}
```

Our middleware cannot wrap the model call because:
- We don't have a `next()` function
- The engine loop owns the driver call, not the middleware
- Even if we added `next()`, the FFI round-trip for every chunk would be catastrophic

**If we want retry/fallback, we need to either:**
- Build retry/fallback **into the Rust engine loop** (not middleware), or
- Fundamentally redesign middleware to wrap execution (huge change), or
- Accept that retry/fallback is an engine feature, not a middleware feature

## 5. What We Should Actually Do

### Immediate: Decide on Option A vs B

We need to pick one approach for provider request interception. My recommendation is **Option A (extend `onLLMStart`)** for these reasons:

1. **It solves the subscription auth use case** — the primary goal you mentioned
2. **It's non-breaking** — existing middleware continues to work
3. **It aligns with how `onLLMStart` is already used** — people think of it as "before the LLM call"
4. **It doesn't require a new hook concept** — keeps the surface small
5. **Retry/fallback is a separate problem** — don't let that complexity bleed into this design

The `headers` field does leak HTTP abstraction slightly, but we can name it `providerHeaders` to make it clear it's transport-layer.

### Important: Keep the Scope Tight

We should NOT try to solve all these at once:
- ❌ Pre-built middleware packages (not discussed)
- ❌ Retry/fallback in middleware (architecturally incompatible)
- ❌ `onToolCall` hook (cosmetic, `onIntent` already works)
- ❌ Session serialization optimization (separate concern)
- ✅ `config` + `providerHeaders` in `onLLMStart` response
- ✅ Error middleware returning synthetic response

## 6. Exact API Shape (Option A)

### Rust: `LlmStartPayload`

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmStartPayload {
    pub prompt: String,
    pub model: String,          // NEW
    pub provider: String,       // NEW
    pub config: Value,          // NEW — the config that will be passed to driver
    pub context: EventContext,
}
```

### Rust: Response Parsing

```rust
// In runtime_loop.rs, after apply_llm_start_middleware:
if let Some(middleware_result) = middleware::apply_llm_start_middleware(handler, payload).await
{
    // Existing
    if let Some(modified) = middleware_result.get("prompt").and_then(Value::as_str) { ... }
    if let Some(new_stack) = middleware_result.get("stack").and_then(Value::as_array) { ... }

    // NEW: config override
    if let Some(config_override) = middleware_result.get("config") {
        // Deep-merge config_override into config_params
    }

    // NEW: headers for driver
    let provider_headers = middleware_result.get("providerHeaders").cloned();
}
```

### TypeScript: Middleware Type Update

```ts
interface LLMStartPayload<IR> {
  prompt: string;
  model: string;
  provider: string;
  config: Record<string, any>;
}

interface LLMStartResponse {
  prompt?: string;
  stack?: string[];
  config?: Record<string, any>;      // Deep-merged into driver config
  providerHeaders?: Record<string, string>;  // Injected into HTTP request
}

onLLMStart?: (
  payload: LLMStartPayload<IR>,
  ctx: MiddlewareContext<IR>
) => void | string | LLMStartResponse | Promise<...>;
```

### Driver Signature Change

```rust
pub trait ModelDriver: ModelDriverBounds {
    async fn stream_generate(
        &self,
        model: &str,
        messages: &[Message],
        config: Option<Value>,
        provider_headers: Option<Value>,  // NEW
    ) -> Result<ModelEventStream, String>;
}
```

## 7. Summary

| Capability | Status | Path Forward |
|-----------|--------|--------------|
| Prompt mutation | ✅ Works | `onLLMStart` returns `{ prompt }` |
| Session mutation | ✅ Works | `onRunStart` returns `{ session }` |
| Intent control | ✅ Works | `onIntent` returns `skip`/`result` |
| Header injection | ❌ Missing | Extend `onLLMStart` with `providerHeaders` |
| Config override | ❌ Missing | Extend `onLLMStart` with `config` |
| Subscription auth | ❌ Missing | Use `providerHeaders` in `onLLMStart` |
| Retry/fallback | ❌ Architecturally impossible | Build into engine loop, not middleware |
| HTTP body mutation | ❌ Missing | Out of scope; use `config` override for provider-specific fields |

**The decision we need from you:** Do we proceed with Option A (extend `onLLMStart` with `config` and `providerHeaders`)? Or do you prefer a different approach?
