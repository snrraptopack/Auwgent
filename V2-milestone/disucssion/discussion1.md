# Auwgent v2 Architecture

> Design notes from architecture discussion — May 2026

---

## 1. What v2 Is

v2 moves Auwgent from a multi-host interpreted system into a self-contained Rust runtime. The language drives everything. The host is a peripheral.

The central goal:

- Write agent logic like normal program logic
- Compile it into a resumable execution graph
- The runtime survives every crash, not just specific portions
- Host-language glue is optional, not required for every behavior

---

## 2. The Core Architecture Shift

### v1 model

```
Host (TypeScript / Python / Dart)
  → reads JSON IR
  → drives execution loop
  → handles intents
  → crosses FFI on every tool call
```

### v2 model

```
Auwgent Runtime (Rust)
  → compiles .aw source internally
  → owns execution graph
  → owns LLM provider calls
  → owns checkpoint / journal
  → owns middleware (DSL-level)

Host (TypeScript / Python / etc.)
  → registers tools        ← input FFI boundary
  → attaches event handlers ← output FFI boundary
  → transports output to clients
```

The runtime is the OS. The host is a driver.

---

## 3. Two and Only Two FFI Boundaries

### Tools — input boundary

Host registers callable functions the DSL can invoke.

```rust
type ToolFn = Arc<dyn Fn(Vec<Value>)
    -> BoxFuture<'static, Result<Value>> + Send + Sync>;
```

### Events — output boundary

Host attaches handlers to observe or intercept runtime lifecycle points.

```rust
type EventHandler = Arc<dyn Fn(EventPayload)
    -> BoxFuture<'static, Result<EventResponse>> + Send + Sync>;

enum EventResponse {
    Continue,
    Skip { result: Value },
    Override(Value),
    Mutate(EventMutation),
}

struct EventMutation {
    prompt: Option<String>,
    model:  Option<ModelConfig>,
    config: Option<ReplyConfig>,
}
```

Everything else — graph execution, LLM calls, expression evaluation,
checkpointing, middleware — stays internal to the Rust runtime.

---

## 4. What the Host Actually Does

Two responsibilities only:

1. **Attach middleware** — optional, only if interception is needed
2. **Transport output** — take what the runtime produces, deliver to clients

The host API follows the config object + `agent.onIntent` pattern.
Tools are defined as plain async functions and registered via config.
Middleware is an object with named lifecycle hooks.

```typescript
import { GEMINI_API_KEY, GROQ_API_KEY } from "../secrets"
import { auwgent, type AuwgentConfig, type AuwgentMiddleware } from "./main.agent.types"
import { create_todo, read_todo } from "./tools"
import { db } from "./db"
import { type SessionState } from "../types"

// middleware — object with lifecycle hooks
const logger: AuwgentMiddleware = {
    name: "logger",
    onRunStart: async (session, ctx) => {
        // load session from DB, return value becomes the session for this run
        let data = await db.load<SessionState>("data.json", session)
        return data
    },
    onRunComplete: async (session, ctx) => {
        await db.save("data.json", session)
    },
    onError: async (error, session, ctx) => {
        return { swallow: true }
    }
}

const config: AuwgentConfig = {
    apiKeys: { groqApiKey: GROQ_API_KEY || "" },
    tools: { create_todo, read_todo },
    middleware: [logger]
}

const agent = auwgent(config)

// single unified intent handler — narrow inside
agent.onIntent((intent, value, name) => {
    if (intent === "response_text")   { console.log("text", value) }
    if (intent === "response_schema") { console.log(JSON.stringify(value, null, 2)) }
    if (intent === "error")           { console.log(value) }
})

// transport
const session = await agent.run("hello my name is Theo i am 10 I'm from Ghana")
console.log(JSON.stringify(session.turns, null, 2))
```

If you strip middleware and transport, the runtime still executes perfectly.
The host is genuinely optional at the execution level.

---

## 5. Event Lifecycle

### Middleware lifecycle hooks (object pattern)

The host middleware is an object with named lifecycle methods.
This is where DB connections, session loading, and external systems belong —
because those resources live in the host, not in the Rust runtime.

```typescript
const AuwgentMiddleware = {
    name: string

    // fires before run starts — return value becomes the session
    onRunStart?: async (session, ctx) => SessionState

    // fires after run completes — persist session here
    onRunComplete?: async (session, ctx) => void

    // fires on error — return { swallow: true } to suppress
    onError?: async (error, session, ctx) => { swallow?: boolean }
}
```

### Intent events (onIntent pattern)

```typescript
agent.onIntent((intent, value, name) => {
    // intent is a string union, narrow inside
})
```

These are the intents that cross the FFI boundary in v2:

```
response_text    → streaming text chunk from model
response_schema  → structured output from model
tool_call        → before tool executes, can skip or override
tool_result      → after tool returns
error            → something failed
```

These intents are gone in v2 — handled internally by the Rust runtime:

```
workflow_call    ✗  internal
workflow_result  ✗  internal
helper_call      ✗  internal
helper_result    ✗  internal
```

### Ordering

1. DSL middleware runs first (inside VM, no FFI cost)
2. Host middleware lifecycle hooks run second (onRunStart / onRunComplete / onError)
3. Combined effects applied together before next node

---

## 6. No JSON IR Emitted

### Why v1 needed JSON IR

The TypeScript runtime had to read the JSON to know how to drive execution.
It was the contract between compiler and host.

```
v1:  compiler → canonical.agent.json  (host reads, host drives execution)
               → RuntimeTest.ts       (150+ lines, imports JSON, intent handlers)
```

### Why v2 does not need it

The Rust runtime owns execution. Nothing needs to cross a language boundary
at the IR level. The compiler emits native Rust structs internally.

```
v2:  compiler → in-memory graph IR    (Rust structs, runtime reads internally)
               → RuntimeTest.ts       (~40 lines, types only, no JSON import)
```

The JSON IR becomes an internal implementation detail.
The host never sees it. You can change the entire internal IR shape
without breaking any host code.

---

## 7. What the Compiler Emits

```
.aw source
  ↓
Rust runtime starts
  ↓
compiles to in-memory execution graph (native Rust structs)
  ↓
emits RuntimeTest.ts  ← only external artifact
  ↓
ready to serve
```

Binary artifacts (`.awc`) are a future optimization for:
- Distribution without shipping source
- Startup caching for large programs

Not needed for v2 first milestone. Add when the problem is actually felt.

---

## 8. The Generated TypeScript File — v1 vs v2

### v1 generated file (problem)

```typescript
// imports JSON IR into TypeScript
import _importedIR from './canonical.agent.json'

// host implements 12+ intent handlers
export interface AuwgentIntentHandler {
    tool_call?(...)
    tool_result?(...)
    workflow_call?(...)
    helper_call?(...)
    helper_result?(...)
    // ... 8 more
}

// complex generics throughout
export type AuwgentAgent = import("@snrraptopack/auwgent-sdk").TypedAuwgent<
    typeof agentIR,
    AuwgentCustomIntents,
    AuwgentOutput,
    AuwgentTools
>
```

The TypeScript SDK shipped a full execution engine — intent loop,
workflow interpreter, helper orchestration.

### v2 generated file (solution)

No JSON import. No generics complexity. The file is purely types
plus one factory function. The execution engine is entirely in Rust.

```typescript
// Auto-generated types for RuntimeTest
// Do not edit manually

import { createAuwgent as createAuwgentRuntime } from "@snrraptopack/auwgent-sdk"
import type { AuwgentMiddleware as BaseMiddleware } from "@snrraptopack/auwgent-sdk"

// --- Input / Output / Context ---

export type Input = string

export type Output = string

export type AuwgentContext = {
    user_name: string
    age: number
    id: string
}

// --- Tools ---

export type AuwgentTools = {
    get_location: (args: {}) => Promise<string>
    get_marks: (args: { id: string }) => Promise<string>
}

// --- Middleware ---

export type AuwgentMiddleware = BaseMiddleware<AuwgentContext, Output>

// --- Intents visible to host ---

export type AuwgentIntent =
    | "response_text"
    | "response_schema"
    | "tool_call"
    | "tool_result"
    | "error"

// --- Config ---

export type AuwgentConfig = {
    apiKeys:    { groqApiKey: string }
    context?:   AuwgentContext
    tools?:     Partial<AuwgentTools>
    middleware?: AuwgentMiddleware[]
}

// --- Factory ---

export function auwgent(config: AuwgentConfig) {
    return createAuwgentRuntime(config)
}

export { auwgent as createAuwgent }
```

---

## 9. Resumability — Graph vs Journal

> **Reference:** Full graph IR design, node categories, execution state shape, checkpoint rules, and resume algorithm are documented in [`RESUMABLE_GRAPH_IR_PROPOSAL.md`](../../RESUMABLE_GRAPH_IR_PROPOSAL.md).

### The graph (in-memory)

Static blueprint. Loaded once at startup. Shared across all runs.
Never mutated. Always reproducible from source.

### The journal (durable)

Per-run record of what has been completed. Survives crashes.
Backend is a pluggable trait — SQLite embedded by default, Redis or Postgres for production.

Each journal entry maps a `(run_id, node_id)` key to a status (`pending`, `running`, `done`, `failed`, `skipped`) and the node's output envelope.

### Resume flow on crash

```
1. Runtime restarts
2. Reloads .aw source → rebuilds same graph in memory (deterministic)
3. Run comes in with existing run ID
4. Executor checks journal for that run ID
5. Nodes marked done → skip, inject stored result
6. Resume from first incomplete node
```

The graph being in-memory does not affect resumability.
Resumability comes entirely from the journal.

---

## 10. Graph IR — Internal Shape

> **Reference:** Full node type catalogue, edge semantics, JSON examples, and lowering examples for all DSL constructs are in [`RESUMABLE_GRAPH_IR_PROPOSAL.md`](../../RESUMABLE_GRAPH_IR_PROPOSAL.md).

The compiler lowers `.aw` source to a graph of typed nodes. Each node has a stable ID derived deterministically from source position — that ID is the journal key.

Node categories:

- **Deterministic** — `LetBind`, `Condition`, `FuncCall` — replayable, no checkpoint required by default
- **Effectful** — `HostToolCall`, `AgentCall`, `Reply` — checkpoint before and after
- **Boundary** — `Input`, `Context`, `Output` — connect the graph to the outside call

Note: a `FuncCall` that contains stdlib I/O (`fetch`, etc.) is promoted to effectful — the per-node `checkpoint` flag on each node handles this explicitly rather than relying on category alone.

Idempotency rule: before executing any effectful node, the executor checks the journal. If the node ID exists with status `done`, skip execution and inject the stored result.

---

## 11. Middleware Architecture

### DSL middleware (inside runtime, no FFI)

```auwgent
@middleware("prompt-prefix")
function PromptPrefix(event: MiddlewareEvent) {
    if event.on == "llmStart" {
        let prompt = event.getPrompt()
        event.setPrompt(prompt + "\nBe concise.")
    }
}

@middlewares(PromptPrefix)
agent Hello(input: Text): Text {
    reply(input) with {
        prompt: "Answer the user."
        model: Gemini
    }
}
```

Runs as bytecode inside a stack VM. Sub-microsecond. No FFI roundtrip.

### Host middleware (across FFI, for external systems)

The host middleware is a named object with optional lifecycle hooks.
This is the pattern confirmed from v1 real usage — it works and carries forward.

```typescript
const logger: AuwgentMiddleware = {
    name: "logger",

    onRunStart: async (session, ctx) => {
        // DB connection lives in host — correct place for this
        let data = await db.load<SessionState>("data.json", session)
        return data   // return becomes the session for this run
    },

    onRunComplete: async (session, ctx) => {
        await db.save("data.json", session)
    },

    onError: async (error, session, ctx) => {
        return { swallow: true }   // suppress error if needed
    }
}
```

Used for: session persistence, database adapters, secrets, analytics, observability.

### Ordering

1. DSL middleware runs first (local, fast, inside VM)
2. Host middleware lifecycle hooks run second (FFI, external systems)
3. Combined effects applied together before next node

---

## 12. Bytecode VM — For Middleware and Expressions

Expressions and middleware evaluate as bytecode inside a stack VM.
No heap allocation for the common case.

```rust
enum Op {
    Push(Value),
    Load(u8),           // local slot index
    Store(u8),
    GetField(InternedKey),
    CallEvent(EventMethod),
    JumpIfFalse(u16),
    Jump(u16),
    Eq, Add, Not,
    Return,
}

struct Vm {
    stack:  Vec<Value>,
    locals: [Value; 32],  // fixed slots, zero heap allocation
    ip:     usize,
    code:   Vec<Op>,
}
```

The graph executor handles async orchestration (LLM calls, tool calls,
agent composition). The bytecode VM handles synchronous expression
evaluation within each node.

---

## 13. Tooling Stack

| Layer | Approach | Reason |
|---|---|---|
| Lexer + Parser | Chumsky v1 | Familiar from Auwla, handles recursive grammar cleanly |
| String interning | `lasso` | All identifiers become `u32` keys throughout pipeline |
| AST allocation | `bumpalo` | Arena allocation, freed in one call after compilation |
| Graph IR | Native Rust structs | Host never reads it, no need for JSON |
| Checkpoint serialization | `rkyv` | Zero-copy, nanosecond write |
| Async runtime | Tokio | Native async, parallel branch dispatch |
| Middleware VM | Stack VM, fixed slots | Sub-microsecond, no FFI roundtrip |
| Journal backend | Trait object | SQLite embedded, Redis/Postgres for production |
| Host FFI (Node) | `napi-rs` | Existing from v1 |
| Host FFI (Python) | `PyO3` | Existing from v1 |

---

## 14. Build Order for First Milestone

1. Lexer + parser → basic AST (agents, reply, let, if, functions, tools)
2. Type checker → symbol resolution, no inference needed (explicit types)
3. Graph IR lowering → `Reply` and `HostToolCall` nodes first
4. Executor with in-memory journal → prove resumption works
5. Plug in SQLite journal → prove crash recovery works
6. Generate TypeScript types file from graph definitions
7. Middleware bytecode compiler → last, syntax still moving

Binary artifact output (`auwgent build` CLI) comes after the runtime
is stable. Not needed for first milestone.

---

## 15. Open Decisions (Carried from DX Proposal)

**Resolved:**
- Context access uses `ctx.` prefix. Direct field access without `ctx.` is not allowed. The checker rejects any local binding that collides with a context field name.

**Still open:**
- Exact `MiddlewareEvent` type shape
- Whether `reply(...)` is allowed without explicit `with` block
- Whether `llmEnd` can modify output or is read-only
- Whether host middleware sees original event, mutated event, or both
- How much middleware effect history to keep in checkpoints
- Exact built-in provider tool names and validation
- `with turns` exact semantics — trace attachment vs inline child graph vs session merge
- Standard library `fetch` — executes in Rust runtime or delegated to host

---

*Source: architecture discussion, May 15–16 2026*
*Repo: github.com/snrraptopack/auwgent-v2-dx*

---

## 16. Two Tool Kinds — Compiler Tracking and Host Interop

v2 has two syntactically distinct tool kinds. The compiler distinguishes them by shape alone — no ambiguity.

### `tool` — host-backed declaration

```auwgent
tool get_location(): string @desc "Return the current user location"
tool get_marks(id: string): string @desc "Return the user's score"
```

- No body
- Required: name, return type
- Optional: parameter list, `@desc`
- Meaning: the runtime cannot execute this — it must call out to the host

### `@tool function` — DSL-defined tool

```auwgent
@tool
@desc "use this to get weather"
function getWeather(city: string): string {
    let response = fetch<string>("https://api.weather/.../city")
    return response.data
}
```

- Has a body
- Required: `@tool` annotation, name, return type, body
- Optional: `@desc`, parameter list
- Meaning: the runtime executes this internally — the host never sees it

These two forms are unambiguous. The compiler does not need inference to tell them apart. `tool` is always host-backed. `@tool function` is always DSL-defined.

---

### Host Tool Registration — v1 Behavior Preserved

In v1, the host registers tool implementations in the config and the runtime calls back when needed:

```typescript
const config: AuwgentConfig = {
    apiKeys: { groqApiKey: "..." },
    tools: {
        get_location: async (args) => "Accra, Ghana",
        get_marks: async (args) => "A+"
    }
}
```

When the engine needed `get_location`, the call crossed the FFI boundary:

```
Rust runtime → NAPI → TypeScript function executes → NAPI → Rust gets result
```

**This roundtrip is preserved exactly in v2.** The `HostToolCall` graph node triggers the same callback mechanism through `auwgent-bridge`. The host registers host-backed tools the same way. The runtime calls out when it hits a `HostToolCall` node, waits for the result, then continues graph execution.

What changes is not the mechanism — it is what gets registered. In v2, only `tool` declarations appear in the generated types. DSL-defined tools (`@tool function`) are compiled into the graph and executed internally. The host never registers them and never sees them.

---

### What the Compiler Emits Per Tool Kind

| Declaration | Graph node | Generated types | Host registers |
|---|---|---|---|
| `tool name(): T` | `HostToolCall` | ✅ appears in `AuwgentTools` | ✅ must provide implementation |
| `@tool function name(): T { ... }` | Internal `FuncCall` / evaluator node | ❌ not emitted | ❌ runtime owns it |

The codegen pass walks tool definitions and emits into `AuwgentTools` only those with `kind: "host"`. DSL-defined tools are skipped. This keeps the generated file honest — every slot in `AuwgentTools` is something the host is actually responsible for.

---

### Standalone Mode (No Host)

When running the runtime as a standalone binary without any host target, host-backed tools can still be satisfied in two ways:

1. Replace them with DSL-defined alternatives using stdlib (`fetch`, etc.) — no host needed at all
2. Run a companion process the binary calls out to over a local socket — uncommon, for production integrations

For normal development, the expectation is that most tool logic moves into DSL-defined functions using the standard library, reducing the dependency on host implementations.