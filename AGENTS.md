# Auwgent — Agent Reference

This file is the canonical reference for agents working on the Auwgent codebase.
If something in here contradicts an older analysis, trust this file.

---

## 1. What Auwgent Is

Auwgent is a high-performance DSL and compiler for building agentic AI applications.

- **DSL:** `.agent` files declare agents, helpers, tools, workflows, types, prompts, models, components, and intents.
- **Compiler:** A Rust workspace (11 crates) lexes, parses, type-checks, and lowers `.agent` files into JSON IR (`.agent.json`).
- **Runtime:** A Rust async runtime workspace (`runtime/crates/`) executes the IR against LLM providers, handling streaming, tool calling, session state, and middleware.
- **Targets:** Generated type stubs + runtime SDKs for TypeScript, Python, Dart, and Rust. A WASM runtime target exists for browser/edge environments.

---

## 2. Project Layout

```
auwgent-compiler/       # Rust workspace — lexer, parser, checker, IR, codegen, CLI, LSP
  crates/
    auwgent-lexer/      # logos-based tokenizer
    auwgent-parser/     # chumsky-based parser with error recovery
    auwgent-ast/        # AST definitions (Span on every node)
    auwgent-checker/    # Type system, validation, workflow type checking
    auwgent-ir/         # AST → AgentIR lowering
    auwgent-ir-schema/  # Typed IR structs (serde + schemars + ts-rs)
    auwgent-codegen/    # Type-stub generators (TS, Python, Dart, Rust)
    auwgent-cli/        # CLI binary (compile, generate, watch)
    auwgent-lsp/        # Language server (tower-lsp)
    auwgent-analysis/   # Symbol tables, import resolution, cross-file analysis
    auwgent-compile/    # Shared validation pipeline (parse → check → lower)
    auwgent-errors/     # Diagnostic types + ariadne rendering

runtime/                # Rust async runtime workspace
  crates/
    auwgent-runtime-core/ # Base types: Message, Role, TokenUsage, FinishReason, callback aliases
    auwgent-session/      # SessionState, Turn, BindingCursor, native message builders
    auwgent-middleware/   # Event enums, payload structs, async fire helpers, parsing functions
    auwgent-native/       # NativeCallableRegistry, IR → JSON Schema for provider-native tool calling
    auwgent-schema/       # FlatFieldSpec, recursive type resolution, output flattening
    auwgent-evaluator/    # IR expression evaluation, prompt rendering with context symbols
    auwgent-protocol/     # BlockOrchestrator, JsonlEventBuffer, partial intent parsing
    auwgent-prompt/       # Block protocol prompt generation (binding rules, intent syntax)
    auwgent-drivers/      # ModelDriver trait + OpenAI and Gemini implementations
    auwgent-engine/       # Engine shell, runtime loop, execution (tools/workflows/helpers), prompt building
    auwgent-bridge/       # EngineBridge FFI facade for C-ABI and target SDKs

targets/
  typescript/           # TypeScript SDK (Bun-based)
  python/               # Python SDK
  dart/                 # Dart SDK
  rust/                 # Rust target SDK (design phase)
  cli/                  # NPM wrapper for native binaries
  wasm-runtime/         # WASM-bindgen wrapper around runtime workspace (NOT minimal)

c-abi/                  # C FFI layer for embedding in other languages
function-parser/        # Standalone bracket-protocol parser crate
extension-vscode/       # VSCode extension
extension-zed/          # Zed extension
tree-sitter-auwgent/    # Tree-sitter grammar
```

---

## 3. Common Misconceptions to Avoid

### 3.1 WASM is not "minimal" or experimental

`targets/wasm-runtime/` is a **first-class production target**. It is a complete `wasm-bindgen` wrapper around the runtime workspace (`auwgent-engine` + `auwgent-bridge`) that exposes:

- Engine construction from IR JSON
- Driver registration (OpenAI, Groq, Gemini, Custom)
- Tool registration with JS callbacks
- Intent handlers (`onIntent`, `onIntentPartial`)
- Sub-engine and middleware hooks
- `run()` returning a JS `Promise`
- Session export/import
- Manual stream chunk writing (`writeChunk`, `endStream`)
- Embedding support

It is built with `wasm-pack --target bundler` and the output is copied into `targets/typescript/wasm-runtime/`. It runs on Cloudflare Workers and in browsers.

The runtime crates have extensive `#[cfg(target_arch = "wasm32")]` conditional compilation to support this target (e.g., `async_trait(?Send)`, `js_sys::Date`, no `tokio::time`).

### 3.2 "Lifecycle" TODOs are not missing features

The DSL has a `use lifecycle { maxTokens, maxMessages }` config block. This is **not a gap** — lifecycle enforcement transitioned into the **middleware layer**. The middleware system handles run lifecycle events (`run_start`, `llm_start`, `llm_end`, `run_complete`, `error`), and lifecycle constraints are enforced there rather than as hardcoded engine rules.

### 3.3 The bracket protocol is intentional, not a workaround

Auwgent uses a custom bracket protocol for LLM output:

```
[tool_call: fetch_user]
id: "123"
[/tool_call]

[response_text]Hello![/response_text]

[schema: Output]
name: "Alice"
[/schema]
```

This is the **primary and intentionally designed** output mode. It offers advantages:
- Works with any model that emits text (no native tool-calling support required)
- Unified protocol for tools, workflows, helpers, components, custom intents, and structured output
- Streaming-friendly partial parsing

Auwgent also supports **native mode** (`@native` annotation or auto-detected from media input) which uses provider-native function calling (OpenAI `tools`, Gemini `functionDeclarations`). See §8 for full dual-mode documentation. Block mode remains the default.

### 3.4 Tests in the DSL are not yet wired to a runner

The DSL parser supports `test` blocks inside agents, but there is **no test runner** yet. The team is still deciding whether tests belong in the compiler, the runtime, or a separate test harness. Do not assume test blocks are executed anywhere.

---

## 4. Compiler Pipeline

```
.agent source
  → auwgent-lexer (logos)           → Vec<Token> + lex errors
  → auwgent-parser (chumsky)        → Model + parse errors
  → auwgent-checker                 → Vec<Diagnostic>
  → auwgent-ir (lowering)           → AgentIR
  → serde_json                      → .agent.json
  → auwgent-codegen                 → type stubs
```

**Key rules:**
- `Text`/`string` input lowers to `null` (default text-only path)
- Media input (`Image`, `File`, `Audio`, `Video`, unions) lowers to strings or union objects
- The IR lowering panics with an "Internal Compiler Error" if the generated JSON fails to deserialize into `AgentIR` — this ensures the IR crate and schema crate stay in sync

---

## 5. Runtime Architecture

### 5.1 Engine (`AuwgentEngine`)

The engine uses interior mutability extensively: `Arc<Mutex<T>>` for shared state across async boundaries.

Key fields:
- `ir: AgentIR` — compiled agent definition
- `session: Mutex<SessionState>` — conversation turns, stack, system prompt
- `orchestrator: Mutex<BlockOrchestrator>` — parses LLM output into intents
- `drivers: Mutex<HashMap<String, Arc<dyn ModelDriver>>>` — LLM providers
- `pending_intents: Mutex<Vec<(String, Value)>>` — queue of parsed intents
- `tools: Mutex<HashMap<String, ToolImplementation>>` — registered tool callbacks

### 5.2 Runtime Loop (`run()`)

1. Initialize session stack
2. Evaluate model config (provider, model name, params)
3. Generate system prompt (block mode appends protocol instructions; native mode does not)
4. Fire `run_start` middleware (can modify session/stack)
5. For each turn:
   - Fire `llm_start` middleware (can modify prompt)
   - Read `toolProtocol` from IR (`resolve_tool_protocol()`)
   - **Block mode:** Build messages via `to_messages_with_bindings()`
   - **Native mode:** Build messages via `to_messages_native_openai()`; inject native tools/output schema into driver config
   - Stream from driver
     - Block mode: feed text chunks to `BlockOrchestrator`
     - Native mode: bypass orchestrator; handle `NativeToolCall` / `NativeStructuredOutput` events
   - `process_intents()` executes tools/workflows/helpers
   - Fire `llm_end` middleware
   - If actions performed:
     - Block mode: build `[result]` YAML blocks and continue
     - Native mode: store `NativeToolResult`s in session and start empty continuation turn
   - Handle empty completion retries (max 2, 250ms delay)
6. Fire `run_complete` middleware

### 5.3 Block Orchestrator

`BlockOrchestrator` parses LLM text output into structured intents:

| Block | Intent | Notes |
|-------|--------|-------|
| `[response_text]` | `response_text` | Terminal chat output |
| `[tool_call: name]` | `tool_call` | Executes registered tool |
| `[workflow_call: name]` | `workflow_call` | Executes workflow body |
| `[helper_call: name]` | `helper_call` | Spawns sub-engine |
| `[component: Name, c_id:"id"]` | `component` | UI component instance |
| `[render_component]` | `render_component` | Component tree render |
| `[schema: Name]` | `response_schema` | Structured output |
| `[custom: name]` | `<custom_name>` | User-defined intent |
| `[result]` / `[error]` | (system) | Injected by runtime, not parsed |

**Emission rules:**
- Partial intents fire during streaming for UI updates (deduplicated by payload comparison)
- Final intents only fire when `is_final = true` to prevent duplicate tool calls with partial args
- Terminal intents (`response_schema`) use last-wins strategy

### 5.4 Session State

`SessionState` stores:
- `system_prompt: Option<String>`
- `turns: Vec<Turn>` — each turn has `input`, optional `input_parts` (multimedia), `model_response`, and native-mode fields
- `stack: Vec<String>` — execution stack (agent names)
- `binding_cursor: Option<BindingCursor>` — runtime binding position

**Block mode:** Turns are minimal — `input`, `model_response`, optional `input_parts`.

**Native mode:** Turns may include:
- `protocol: "native"` — marks the turn's protocol
- `nativeAssistantTurn` — assistant's text, `tool_calls`, and `structured_output` (only when tool calls or structured output exist; text-only turns omit this)
- `nativeToolResults` — results linked to provider call IDs for round-trip message reconstruction

Message builders:
- `to_messages_with_bindings()` — block mode; renders binding blocks as user messages; bindings are NOT stored in turns
- `to_messages_native_openai()` — native mode; reconstructs OpenAI-style history with `tool_calls` on assistant messages and `role: "tool"` / `tool_call_id` on result messages

### 5.5 Model Drivers

Trait `ModelDriver` (async via `async-trait`):
- `stream_generate(model, messages, config) -> ModelEventStream`
- `embed(model, text, config) -> Vec<f32>`
- `embed_batch(model, texts, config) -> Vec<Vec<f32>>`

Events: `ContentChunk(String)`, `Usage(TokenUsage)`, `FinishReason(FinishReason)`, `Metadata(ModelMetadata)`

**WASM note:** On `wasm32`, the trait uses `async_trait(?Send)` and `Pin<Box<dyn Stream>>` (without `Send`). Native uses `Send + Sync` bounds.

---

## 6. Middleware System

Middleware is a **first-class, production-ready** feature. It is not a placeholder.

Event types: `RunStart`, `LlmStart`, `LlmEnd`, `Intent`, `RunComplete`, `Error`

`IntentControl` responses:
- `Skip` — prevent the intent from executing
- `Override { result }` — replace the execution result
- `null` / absent — let execution proceed normally

Middleware can modify:
- Session (via `run_start` returning `{ session: ... }`)
- Prompt (via `llm_start` returning `{ prompt: ... }`)
- Stack (via `llm_start` returning `{ stack: [...] }`)

---

## 7. Multimodal Input

**Status: Partially implemented**

The DSL supports:
```auwgent
input: Text
input: Image
input: File
input: Audio
input: Video
input: Image | File
```

Implemented:
- Lexer tokens (`ImageType`, `FileType`, `AudioType`, `VideoType`)
- AST type expressions
- Checker rules (root input validation)
- IR lowering (`Text` → `null`, media → strings/unions)
- TypeScript codegen (`TextPart`, `ImagePart`, etc., `input` builders)
- Runtime session stores `input_parts: Option<Vec<Value>>`

Not yet implemented:
- Provider driver normalization (converting parts to OpenAI/Gemini native formats)
- Full target SDK support beyond TypeScript
- Media transport policy (inline vs upload vs URL)

---

## 8. Dual Mode: Block vs Native Protocol

Auwgent supports two execution modes for agentic tool calling, selected automatically by the compiler based on input/output types or explicitly via annotations.

### 8.1 Block Mode (`toolProtocol: "block"`) — Default

The LLM outputs structured text using the bracket protocol:

```
[tool_call: fetch_user]
id: "123"
[/tool_call]

[response_text]Hello![/response_text]
```

The `BlockOrchestrator` parses this into intents. Block mode:
- Works with any text-emitting model (no native function-calling support required)
- Uses a single unified protocol for tools, workflows, helpers, and structured output
- Streaming-friendly with partial parsing

### 8.2 Native Mode (`toolProtocol: "native"`)

The LLM uses provider-native function calling (OpenAI `tools`, Gemini `functionDeclarations`). The runtime:
- Injects tool schemas and output schemas into the provider request config
- Receives `NativeToolCall` / `NativeStructuredOutput` events from the driver
- Maps provider function names (`tool_search`) to intents via prefix-based routing (`tool_` → `tool_call`, `workflow_` → `workflow_call`, `helper_` → `helper_call`)
- Reconstructs message history with OpenAI-style `tool_calls` / `role: "tool"` / `tool_call_id` for round-trip correctness

Native mode is triggered automatically when the agent uses non-text input/output (`Image`, `File`, `Audio`, `Video`), or explicitly with `@native`.

### 8.3 How the Compiler Decides

| Source | Resulting `toolProtocol` |
|--------|--------------------------|
| `@native` annotation | `"native"` |
| `@block` annotation | `"block"` |
| Media input/output (`Image`, `File`, etc.) with no annotation | `"native"` (auto-detect) |
| Text-only with no annotation | `"block"` (default) |

```auwgent
// Explicit native
@native
agent Vision {
    input: Image
    default config { ... }
}

// Explicit block
@block
agent Chatbot {
    input: Text
    default config { ... }
}

// Auto-detects to native (Image is media)
agent Auto {
    input: Image | Text
    default config { ... }
}
```

**Compile-time error:** `@block` + media types produces a checker error:
```
Agent is annotated with @block but uses non-text input/output types that require native mode
```

### 8.4 Runtime Branching

The runtime reads `toolProtocol` from the IR once per turn:

```rust
let is_native = self.resolve_tool_protocol() == "native";
```

This single boolean branches **every** protocol-sensitive path in the loop:

| Path | Block mode | Native mode |
|------|-----------|-------------|
| **Message building** | `to_messages_with_bindings()` (includes `[result]` blocks) | `to_messages_native_openai()` (OpenAI-style `tool_calls` + `role: "tool"`) |
| **System prompt** | Appends block protocol instructions | Appends nothing (model learns from provider-native declarations) |
| **Config injection** | No extra config | Injects `auwgent_native_tools` + `auwgent_native_output_schema` (OpenAI skips output schema when tools are present to avoid API errors) |
| **Text streaming** | Feeds chunks to `BlockOrchestrator` | Bypasses orchestrator; raw text accumulates directly |
| **Tool calls** | Parsed from `[tool_call: ...]` blocks | Received as `NativeToolCall` events; normalized to `tool_call` / `workflow_call` / `helper_call` intents |
| **Structured output** | Parsed from `[schema: ...]` blocks | Received as `NativeStructuredOutput` events; emitted as `response_schema` intent |
| **Result continuation** | Builds `[result]` YAML blocks for next turn | Stores `NativeToolResult` in session; starts empty turn |
| **Response sanitization** | Strips orphan text via `BlockScanner` | Preserves raw text as-is |

### 8.5 Session State Differences

Block-mode turns are minimal:
```json
{
  "input": "hello",
  "model_response": "[response_text]Hi![/response_text]"
}
```

Native-mode turns carry extra state only when needed:
```json
{
  "input": "hello",
  "model_response": "Hello!",
  "protocol": "native",
  "nativeAssistantTurn": {
    "text_content": "I'll search for that.",
    "tool_calls": [
      {
        "id": "call_abc",
        "provider_name": "tool_search",
        "canonical_name": "search",
        "action_kind": "tool_call",
        "arguments": { "query": "cats" }
      }
    ],
    "structured_output": null
  },
  "nativeToolResults": [
    {
      "call_id": "call_abc",
      "provider_name": "tool_search",
      "canonical_name": "search",
      "action_kind": "tool_call",
      "arguments": { "query": "cats" },
      "result": { "results": ["cat1", "cat2"] }
    }
  ]
}
```

**Design note:** `nativeAssistantTurn` is only populated when there are tool calls or structured output. Text-only native turns store the response in `model_response` like block mode, keeping the common case clean.

### 8.6 Key Files

| File | Role |
|------|------|
| `auwgent-lexer/src/lib.rs` | `@native` / `@block` tokens (`AtNative`, `AtBlock`) |
| `auwgent-parser/src/toplevel.rs` | Parses annotation before `agent` keyword |
| `auwgent-checker/src/lib.rs` | Validates `@block` + media = error |
| `auwgent-ir/src/lib.rs` | Sets `"toolProtocol"` in model config JSON |
| `runtime/crates/auwgent-engine/src/engine.rs` | `resolve_tool_protocol()` reads from IR |
| `runtime/crates/auwgent-engine/src/engine/prompt.rs` | Branches prompt generation on protocol |
| `runtime/crates/auwgent-engine/src/engine/runtime_loop.rs` | Full runtime branching (messages, config, events, results) |
| `runtime/crates/auwgent-native/src/schema.rs` | IR types → JSON Schema for provider-native declarations |
| `runtime/crates/auwgent-native/src/registry.rs` | `NativeCallableRegistry` with prefix-based routing |
| `runtime/crates/auwgent-session/src/state.rs` | `NativeAssistantTurn`, `NativeToolResult`, `to_messages_native_openai()` |
| `runtime/crates/auwgent-drivers/src/openai.rs` | Handles `tool_calls` / `tool_call_id` on messages |
| `runtime/crates/auwgent-drivers/src/gemini.rs` | Handles `functionCall` / `functionResponse` in contents |

---

## 9. Code Style & Patterns

- **Rust edition 2024** in newer crates (check `Cargo.toml`)
- **Span-based errors:** Every AST node carries a `Span { start, end }` for precise diagnostics
- **Interior mutability:** `Arc<Mutex<T>>` is the standard pattern in the engine for cross-async shared state
- **WASM compat:** Always use `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` for platform-specific behavior
- **Error handling:** Compiler uses `Diagnostic` with severity + labels + help. Runtime uses `AuwgentResult<T>` / `AuwgentError`.
- **Tests:** Unit tests live in `#[cfg(test)]` modules inside source files. Integration tests live in `tests/` directories.

---

## 10. Runtime Tests (Cross-Language)

Real LLM integration tests live in `runtime-tests/` and exercise the full FFI + SDK stack. Each target language has a test runner that executes the same canonical scenarios against a compiled `.agent.json`.

### 10.1 Test Runners

| Language | Runner | Scenarios | Status |
|----------|--------|-----------|--------|
| TypeScript | `runtime-tests/typescript/test-runner.ts` | 19 | Implemented |
| Python | `runtime-tests/python/test_runner.py` | 19 | Implemented |
| Dart | `runtime-tests/dart/test_runner.dart` | 19 | Implemented |

### 10.2 Scenario Coverage

1. Basic chat (block mode)
2. Tool call with no arguments
3. Tool call with arguments
4. Workflow execution
5. Helper with `Return` handoff
6. Helper with `User` handoff
7. Custom intent emission
8. Middleware lifecycle (all hooks fire in order)
9. Session export/import persistence
10. Error swallowing via middleware
11. Streaming partial intents
12. Middleware state sharing across hooks
13. Middleware prompt mutation
14. Middleware config/header mutation
15. Middleware stack mutation
16. Middleware intent override
17. Middleware intent skip
18. Middleware session mutation
19. Fallback on rate limit (`forceStart: "llm_start"`)

### 10.3 Middleware Testing Patterns

All test runners use a consistent pattern:
- **Setup function** returns `(agent, middleware_log)`
- **Middleware classes** inherit from the language-specific `Middleware` base class
- **Log arrays** capture hook execution order and mutations for assertions
- **4-second delays** between scenarios to avoid rate limits
- **ASCII-only output** in Python to avoid Windows `cp1252` encoding issues

### 10.4 Unified Results

All test results are collected into a single log file:
- `runtime-tests/test-results.json` — structured JSON with per-scenario, per-language results
- `runtime-tests/test-results.md` — human-readable markdown summary

### 10.5 Provider & Rate Limits

Tests use **Groq** (`llama-3.3-70b-versatile`) by default. The Groq TPD (tokens per day) limit is 100K. When the limit is hit, scenario 19 (fallback middleware) demonstrates switching to an alternative provider via `onError` → `forceStart: "llm_start"`.

### 10.6 Key SDK Changes from Testing

- **TypeScript SDK:** `onLLMStart` simplified to string-only return; `ctx.model`, `ctx.apiKey`, `ctx.config`, `ctx.provider`, `ctx.url`, `ctx.headers` mutations are read after all middleware run
- **Python SDK:** `Middleware` converted from `Protocol` to `ABC` with default no-op implementations; `onError` accepts `bool | dict`; `onLLMStart` simplified to `Optional[str]` return
- **Dart SDK:** `onLLMStart` simplified to `FutureOr<String?>`; `MiddlewareLLMStartResult` removed across all SDKs
- **Rust Runtime:** `ModelDriver` trait accepts `api_key: Option<String>` for per-request key override; `EventContext` carries `model` and `api_key` fields. Runtime now lives in `runtime/crates/` as an 11-crate workspace.

---

## 11. Key Design Documents

| Document | Topic | Status |
|----------|-------|--------|
| `MULTIMODAL_INPUT_DESIGN.md` | Compiler-driven multimodal input | Partially implemented |
| `RUST_TARGET_DESIGN.md` | Rust target SDK design | Planned / design phase |
| `IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md` | Dual-mode tool calling | **Implemented** — see §8 |
| `middleware_findings.md` | Middleware research | Research |
| `prompt-caching-research.md` | Prompt caching investigation | Research |

---

## 12. When You See These, Know This

- `#[cfg(target_arch = "wasm32")]` — This code path is for the WASM runtime target (browser, Cloudflare Workers, etc.)
- `BlockOrchestrator` — This is the parser for LLM output. It is core to how the runtime works.
- `process_intents()` — This is where tools, workflows, and helpers actually execute.
- `sanitize_model_response()` — Strips orphan text before protocol blocks.
- `JsonValue` in `auwgent-ir-schema` — A `serde_json::Value` newtype that implements `ts_rs::TS`.
- `HandoffKindIR` / `helperHandoff` — Controls helper execution: `Return` (default), `User` (stream to user), `ThenContinue` (stream then resume parent).
- `binding_cursor` — Runtime-only context injection point, not persisted in turns.
