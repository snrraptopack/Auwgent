# Auwgent — Agent Reference

This file is the canonical reference for agents working on the Auwgent codebase.
If something in here contradicts an older analysis, trust this file.

---

## 1. What Auwgent Is

Auwgent is a high-performance DSL and compiler for building agentic AI applications.

- **DSL:** `.agent` files declare agents, helpers, tools, workflows, types, prompts, models, components, and intents.
- **Compiler:** A Rust workspace (11 crates) lexes, parses, type-checks, and lowers `.agent` files into JSON IR (`.agent.json`).
- **Runtime:** A Rust async runtime (`ir-runtime`) executes the IR against LLM providers, handling streaming, tool calling, session state, and middleware.
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

ir-runtime/             # Rust async runtime
  src/
    runtime/
      engine.rs         # Core engine shell + registration
      engine/
        runtime_loop.rs # Main async run loop, middleware coordination
        execution.rs    # Intent dispatch (tools, workflows, helpers)
        execution/      # Sub-modules for tools, workflows, helpers
        prompt.rs       # System prompt generation
        native_schema.rs# JSON Schema generation for native tool calling
      drivers/
        mod.rs          # ModelDriver trait + ModelEvent types
        gemini.rs       # Gemini driver
        openai.rs       # OpenAI driver (also used for Groq/custom)
      session.rs        # SessionState, Turn, Message types
      streaming/        # JSONL event buffer, partial intents
        parser/
          block_orchestrator.rs  # Parses bracket protocol from LLM output
      middleware.rs     # Middleware event firing
      middleware_event.rs # Middleware event type definitions
      helper_runner.rs  # Helper execution with handoff modes
    types.rs            # Re-exports from auwgent-ir-schema
    evaluator.rs        # IR expression evaluation
    flat_args.rs        # Flatten/unflatten for nested IR fields
    schema.rs           # Schema helpers
    intents.rs          # Block protocol prompt generation

targets/
  typescript/           # TypeScript SDK (Bun-based)
  python/               # Python SDK
  dart/                 # Dart SDK
  rust/                 # Rust target SDK (design phase)
  cli/                  # NPM wrapper for native binaries
  wasm-runtime/         # WASM-bindgen wrapper around ir-runtime (NOT minimal)

c-abi/                  # C FFI layer for embedding in other languages
function-parser/        # Standalone bracket-protocol parser crate
extension-vscode/       # VSCode extension
extension-zed/          # Zed extension
tree-sitter-auwgent/    # Tree-sitter grammar
```

---

## 3. Common Misconceptions to Avoid

### 3.1 WASM is not "minimal" or experimental

`targets/wasm-runtime/` is a **first-class production target**. It is a complete `wasm-bindgen` wrapper around `ir-runtime` that exposes:

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

The `ir-runtime` itself has extensive `#[cfg(target_arch = "wasm32")]` conditional compilation to support this target (e.g., `async_trait(?Send)`, `js_sys::Date`, no `tokio::time`).

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

There is a **dual-mode proposal** (`IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md`) to add provider-native tool calling as an opt-in alternative (`toolProtocol: "native"`), but block mode remains the default. Do not treat the block protocol as something to be replaced.

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
3. Generate system prompt
4. Fire `run_start` middleware (can modify session/stack)
5. For each turn:
   - Fire `llm_start` middleware (can modify prompt)
   - Build messages from session history + optional binding block
   - Stream from driver → feed chunks to `BlockOrchestrator`
   - `process_intents()` executes tools/workflows/helpers
   - Fire `llm_end` middleware
   - If actions performed, build `[result]` YAML blocks and continue
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
- `turns: Vec<Turn>` — each turn has `input`, optional `input_parts` (multimedia), and `model_response`
- `stack: Vec<String>` — execution stack (agent names)
- `binding_cursor: Option<BindingCursor>` — runtime binding position

`to_messages_with_bindings()` reconstructs provider messages from turns. Bindings are rendered as user messages but are NOT stored in turns, keeping exported sessions clean.

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

## 8. Native Tool Calling (Dual Mode)

**Status: Design complete, not yet implemented**

See `IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md` for the full proposal.

Summary:
- Block mode (default) uses the bracket protocol
- Native mode (`toolProtocol: "native"`) uses provider-native function calling
- Native mode requires:
  1. Schema generation via `native_schema.rs` (or replacement)
  2. Driver event extensions (`NativeToolCall`, `NativeStructuredOutput`)
  3. Session turn extensions for native assistant turns + tool results
  4. Prompt generation that skips block protocol instructions
  5. Execution adapter that normalizes native calls into the same intent shape

The recommended first code change is **NOT** driver modification. It is making `native_schema.rs` produce a full `NativeCallableRegistry` with focused tests.

---

## 9. Code Style & Patterns

- **Rust edition 2024** in newer crates (check `Cargo.toml`)
- **Span-based errors:** Every AST node carries a `Span { start, end }` for precise diagnostics
- **Interior mutability:** `Arc<Mutex<T>>` is the standard pattern in the engine for cross-async shared state
- **WASM compat:** Always use `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` for platform-specific behavior
- **Error handling:** Compiler uses `Diagnostic` with severity + labels + help. Runtime uses `AuwgentResult<T>` / `AuwgentError`.
- **Tests:** Unit tests live in `#[cfg(test)]` modules inside source files. Integration tests live in `tests/` directories.

---

## 10. Key Design Documents

| Document | Topic | Status |
|----------|-------|--------|
| `MULTIMODAL_INPUT_DESIGN.md` | Compiler-driven multimodal input | Partially implemented |
| `RUST_TARGET_DESIGN.md` | Rust target SDK design | Planned / design phase |
| `IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md` | Dual-mode tool calling | Design complete |
| `middleware_findings.md` | Middleware research | Research |
| `prompt-caching-research.md` | Prompt caching investigation | Research |

---

## 11. When You See These, Know This

- `#[cfg(target_arch = "wasm32")]` — This code path is for the WASM runtime target (browser, Cloudflare Workers, etc.)
- `BlockOrchestrator` — This is the parser for LLM output. It is core to how the runtime works.
- `process_intents()` — This is where tools, workflows, and helpers actually execute.
- `sanitize_model_response()` — Strips orphan text before protocol blocks.
- `JsonValue` in `auwgent-ir-schema` — A `serde_json::Value` newtype that implements `ts_rs::TS`.
- `HandoffKindIR` / `helperHandoff` — Controls helper execution: `Return` (default), `User` (stream to user), `ThenContinue` (stream then resume parent).
- `binding_cursor` — Runtime-only context injection point, not persisted in turns.
