# Discussion 13: Quew-ifying the Runtime — What Can Be Written in Quew?

**Status:** Open — architectural analysis for V2 runtime design.

---

## Core Insight

In V1, the entire runtime (agent loop, drivers, prompt building, protocol parsing, session management) was written in Rust. For V2, we want to push as much as possible into quew itself. The compiler gets stronger, the runtime gets smaller, and users can customize behavior by writing quew code instead of Rust patches.

The key enabler: **give quew powerful builtins** (`fetch`, JSON manipulation, string scanning) so it can do what previously required Rust.

---

## V1 Runtime Components Audit

| V1 Crate | Responsibility | Can Be Quew? | Blockers / Needs |
|----------|---------------|--------------|------------------|
| `auwgent-runtime-core` | `Value`, `Message`, `Role`, `TokenUsage`, base types | ❌ **Must stay Rust** | These are the runtime's own types |
| `auwgent-session` | `SessionState`, turns, binding cursor, message builders | ⚠️ **Partial** | Needs mutable state / persistent data structures |
| `auwgent-middleware` | Event enums, payload structs, async fire helpers | ⚠️ **Partial** | Needs async event dispatch; quew can hold middleware *logic* |
| `auwgent-native` | `NativeCallableRegistry`, JSON schema for providers | ⚠️ **Partial** | Schema generation could be quew; registry must be Rust |
| `auwgent-schema` | `FlatFieldSpec`, recursive type resolution | ✅ **Yes** | Deterministic tree walking — pure computation |
| `auwgent-evaluator` | IR expression evaluation, prompt rendering with context symbols | ⚠️ **Partial** | `eval_expr` is the "CPU" — must stay Rust; prompt rendering can be quew |
| `auwgent-protocol` | `BlockOrchestrator`, `JsonlEventBuffer`, partial intent parsing | ✅ **Yes, with builtins** | Needs regex/scanning builtins; otherwise pure string logic |
| `auwgent-prompt` | Block protocol prompt generation (binding rules, intent syntax) | ✅ **Yes** | Pure string templating — already works with interpolation |
| `auwgent-drivers` | OpenAI, Gemini HTTP clients | ✅ **Yes, with `fetch`** | HTTP request building + JSON extraction can be quew |
| `auwgent-engine` | Main engine shell, runtime loop, execution, prompt building | ⚠️ **Partial** | Loop orchestration needs async + streaming; body can call quew functions |
| `auwgent-bridge` | FFI facade for C-ABI / WASM | ❌ **Must stay Rust** | FFI is inherently host-language |

---

## The Three Tiers

### Tier 1: Must Stay Rust (The "Kernel")

These are the bedrock that quew code runs on top of. They cannot be quew because quew *is* compiled to IR that these components execute.

1. **Graph executor / `eval_expr`** — walks IR nodes, evaluates `IrExpr` into `Value`
2. **Value representation** — `Value` enum, memory layout, JSON serialization
3. **Native registry dispatch** — `NativeHandler::Sync` trampoline, `inventory` link-time registration
4. **Async runtime integration** — tokio executor, `Stream` handling, WASM compat
5. **FFI bridge** — C-ABI, WASM-bindgen exports

> **Analogy:** This is the "CPU + memory + OS kernel" of the system. Everything else runs on it.

### Tier 2: Can Be Quew With New Builtins

These are currently Rust but could become quew if we add the right primitives.

#### 2a. HTTP / Networking (`fetch` builtin)

**What it enables:** LLM drivers, web tool calls, HTTP-based middleware.

A `fetch(url, options)` builtin that returns `{ status, headers, body }` would let quew make HTTP requests. With JSON parsing builtins, quew can:

```quew
function openai_chat_completion(model: string, messages: string, apiKey: string): string {
    let response = fetch("https://api.openai.com/v1/chat/completions", {
        method: "POST",
        headers: {
            "Authorization": "Bearer " + apiKey,
            "Content-Type": "application/json"
        },
        body: json_stringify({
            model: model,
            messages: messages,
            stream: true
        })
    })
    return response.body
}
```

**What Rust still owns:** The *streaming* aspect. `fetch` can return a complete response body, but streaming chunks require Rust-level `Stream` handling. A `fetch_stream()` builtin that yields chunks as an async iterator is possible but needs language support for async iterators.

**Recommendation:** Start with a synchronous `fetch()` that returns the full response. Non-streaming drivers (Gemini non-stream, simple HTTP tools) become pure quew. Streaming stays Rust for now.

#### 2b. JSON Manipulation Builtins

**What it enables:** Response parsing, request building, schema generation.

Needed builtins:
- `json_parse(string)` → `Value`
- `json_stringify(value)` → `string`
- `json_get(object, path)` → `Value | null`  (e.g. `json_get(resp, "choices.0.message.content")`)
- `json_set(object, path, value)` → `Value`
- `json_keys(object)` → `string[]`
- `json_type(value)` → `"string" | "number" | ...`

With these, the entire "extract fields from LLM response" pipeline becomes quew code.

#### 2c. String Scanning / Regex Builtins

**What it enables:** Block protocol parsing, intent extraction.

The `BlockOrchestrator` in V1 is a hand-written state machine that scans text for `[tool_call: ...]`. This is deterministic string processing — perfect for quew if we have:

- `string_find(haystack, needle)` → `number | null`
- `string_split(text, delimiter)` → `string[]`
- `string_starts_with(text, prefix)` → `bool`
- `regex_match(text, pattern)` → `{ matched: bool, groups: string[] }`

Or even simpler: expose the existing `function-parser` crate (block scanner / tokenizer) as a builtin:
- `block_scan(text)` → `Block[]`
- `parse_intent(text)` → `{ name: string, args: object }`

#### 2d. Loop Constructs

**What it enables:** The agent loop itself, iteration over arrays.

The agent loop in V1 is literally:
```rust
loop {
    // build messages
    // call LLM
    // stream response
    // parse intents
    // execute tools
    // if no tools, break
}
```

In quew, this could be expressed as **recursion** (already supported!):

```quew
function agent_turn(session: Session, max_turns: number): Session {
    if max_turns <= 0 {
        return session
    }
    let response = llm_call(session.messages)
    let intents = parse_intents(response)
    if intents.is_empty() {
        return session.with_response(response)
    }
    let results = execute_intents(intents)
    let next_session = session.with_results(results)
    return agent_turn(next_session, max_turns - 1)
}
```

Since `FuncCall` already supports recursion in the graph executor, the *loop body* can be quew. The loop *orchestrator* (async streaming, middleware hooks) stays Rust.

**But** `for` loops over arrays are more ergonomic. The parser already parses `for idx, value in iterable`. We need:
1. Lower `ForStmt` to IR (currently emits nothing)
2. Runtime support for array iteration

This is a medium-sized compiler change.

**Recommendation:** Use recursion for the agent loop (works today with minor IR work). Add `for` loops later for ergonomics.

### Tier 3: Can Be Quew Today (Deterministic, Pure)

These need no new language features — just someone to write them in quew.

1. **Prompt templates** — String interpolation already works. System prompt building, tool descriptions, protocol instructions are all string manipulation.
2. **JSON schema generation** — The `auwgent-schema` crate walks type definitions and outputs JSON Schema. This is pure recursive tree walking. Can be a quew function.
3. **Result formatters** — Converting tool outputs to `[result]` YAML blocks or native tool result messages.
4. **Simple middleware logic** — Logging, prompt mutation, header injection. These are pure functions over data.
5. **Type discrimination helpers** — `value is string`, `value is User`. The runtime evaluation is a pattern match; the *logic* can be quew if we expose `Value` introspection builtins.

---

## The Bold Vision: A "Microkernel" Runtime

If we push this direction to its conclusion, the Rust runtime becomes a **thin microkernel**:

```
┌─────────────────────────────────────────────┐
│  quew-engine (Rust microkernel)             │
│  ├─ Async I/O loop (tokio)                  │
│  ├─ Stream chunk router                     │
│  ├─ Middleware hook dispatcher              │
│  ├─ Session persistence (export/import)     │
│  └─ Calls into quew-compiled graphs         │
├─────────────────────────────────────────────┤
│  quew-compiled IR graphs (user + stdlib)    │
│  ├─ prompt_build(agent, session)            │
│  ├─ driver_openai_request(model, messages)  │
│  ├─ driver_gemini_request(model, messages)  │
│  ├─ block_protocol_parse(chunk)             │
│  ├─ tool_result_format(result)              │
│  ├─ middleware_logger(event)                │
│  └─ agent_loop_body(session, turn_count)    │
├─────────────────────────────────────────────┤
│  quew-runtime-core (Rust "CPU")             │
│  ├─ eval_expr                               │
│  ├─ Execution::run                          │
│  ├─ NativeRegistry                          │
│  └─ Value + JSON serde                      │
└─────────────────────────────────────────────┘
```

### What changes in the compiler?

To support this, the compiler needs to output not just per-function graphs, but also **"system graphs"** that the engine can call by name:

```json
{
  "graphs": {
    "agent:Chatbot": { ... },
    "function:greet": { ... },
    "__prompt_builder:Chatbot": { ... },
    "__driver_request:openai": { ... },
    "__protocol_parser:block": { ... }
  }
}
```

The `__` prefixed graphs are generated by the compiler from special declarations or conventions. The engine knows to call `__prompt_builder:Chatbot` before each LLM turn.

### What builtins do we need?

| Builtin | Status | Enables |
|---------|--------|---------|
| `fetch(url, options)` | ❌ Not started | HTTP drivers, web tools |
| `json_parse(s)` / `json_stringify(v)` | ❌ Not started | All response/request handling |
| `json_get(obj, path)` | ❌ Not started | Field extraction from LLM responses |
| `block_scan(text)` | ❌ Not started | Block protocol parsing |
| `regex_match(text, pattern)` | ❌ Not started | General text parsing |
| `string_find` / `string_split` | ✅ `contains`, `starts_with` exist | More needed |
| `array_len` / `array_get` / `array_push` | ❌ Not started | Session manipulation, iteration |
| `value_type(value)` | ❌ Not started | Runtime type introspection |
| `timestamp()` / `random()` | ❌ Not started | General utility |

---

## Immediate Next Steps (Recommended Order)

1. **Add `fetch` builtin** — This is the biggest unlock. With `fetch` + JSON builtins, LLM drivers become quew code.
2. **Add JSON builtins** (`json_parse`, `json_stringify`, `json_get`) — Required companion to `fetch`.
3. **Make `for` loops executable** — Already parsed, needs IR lowering + runtime support.
4. **Add `while` parsing** — Simple parser addition, same IR lowering as `for`.
5. **Add `value_type` / `is` runtime** — Enables type discrimination in quew.
6. **Add `array_*` builtins** — Mutable array operations for session building.

These six items transform quew from a "toy DSL" into a language capable of writing its own runtime logic.

---

## What Stays Rust No Matter What?

Even in the most aggressive quew-ification scenario, these remain Rust:

1. **The executor itself** — something has to walk IR nodes
2. **Async runtime** — tokio / WASM event loop
3. **reqwest integration** — the actual socket I/O (even if `fetch` is the interface)
4. **WASM bindings** — `wasm-bindgen`, `js-sys`
5. **C-ABI / FFI** — embedding in other languages
6. **The compiler** — self-hosting is a multi-year project

---

## Summary

| Component | V1 (all Rust) | V2 (target) |
|-----------|--------------|-------------|
| Agent loop body | Rust | **Quew** (recursion) |
| LLM driver (HTTP) | Rust | **Quew** (`fetch` builtin) |
| Prompt building | Rust | **Quew** (interpolation) |
| Block protocol parsing | Rust | **Quew** (scanning builtins) |
| JSON schema gen | Rust | **Quew** (tree walking) |
| Session state | Rust | **Rust** (needs persistence) |
| Stream orchestration | Rust | **Rust** (needs async) |
| Middleware dispatch | Rust | **Rust** (async hooks) |
| Graph execution | Rust | **Rust** (the "CPU") |
| Value / IR types | Rust | **Rust** (foundation) |

The V2 runtime should be a **thin Rust orchestrator** that calls into **quew-compiled logic** for all deterministic work. This is the architectural shift that justifies building the compiler in the first place.
