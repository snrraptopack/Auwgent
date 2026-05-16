# Plan 5: Parser Finalisation + IR Lowering

## Where We Are

`quew-checker` is complete and locked. **83 tests, 0 failures.**

**Full frontend stack standing:**
```
quew-errors     — Span, Diagnostic, Severity
quew-interner   — ThreadedRodeo, InternedStr
quew-source     — SourceId, SourceFile, SourceMap
quew-lexer      — TokenKind, lex(), LexResult              (73 tests)
quew-ast        — Full AST, every node with Span           (46 tests)
quew-parser     — Full grammar, error recovery             (24 tests)
quew-types      — Ty enum, ToolTy, AgentTy, ProviderKind  (tested)
quew-scope      — SymbolTable, build_symbol_table()        (tested)
quew-unify      — UnifyTable, structural unification       (tested)
quew-checker    — Full semantic pass                       (83 tests)
```

**Crates to fill:**
- `quew-ir` — currently stubbed, no real code

**Skipped:**
- `quew-resolve` — cross-file imports, no `import` syntax yet

---

## Correction from Discussion 1 — No JSON IR File

The first draft of this plan described a JSON file (`file.quew.json`) that the host reads.
That is the **v1 model**. Discussion 1 (section 6) is explicit:

```
v1:  compiler → canonical.agent.json  (host reads, host drives execution)

v2:  compiler → in-memory graph IR    (Rust structs, runtime reads internally)
```

**The host never sees the IR.** It never reads it, never imports it, never drives execution from it.
The compiler emits native Rust structs that the Rust runtime loads internally.
The IR is rebuilt from source on startup — it is deterministic and reproducible.

JSON serialization (`rkyv`, `serde_json`) belongs on the **execution state / journal** — the per-run checkpoint — not on the compiled IR.

Typescript codegen (the generated types file) is **deferred** — it will be ported from v1 when the language is stable. Plan 5 focuses purely on the language pipeline: parser → checker → IR lowering.

---

## The Correct Two-Layer Architecture (discussion1.md §9)

```
┌──────────────────────────────────────────────────────────────────┐
│  Compiled Graph IR  (native Rust structs, in-memory, static)     │
│  Built from source on startup. Shared across all runs.           │
│  Never mutated. Always reproducible.                             │
│  ─ what nodes exist                                              │
│  ─ how they connect (edges + data slots)                         │
│  ─ what each node does (type, config)                            │
│  ─ what needs a checkpoint                                        │
└──────────────────────────────────────────────────────────────────┘
         ↓ loaded once by runtime, never mutated

┌──────────────────────────────────────────────────────────────────┐
│  Journal  (per-run, durable, pluggable backend)                  │
│  SQLite embedded by default. Redis / Postgres for production.    │
│  Keyed by (run_id, node_id).                                     │
│  ─ node status: pending → running → done / failed                │
│  ─ node output: cached result per node                           │
│  ─ active node metadata: transcript, pending tools               │
│  Serialized with `rkyv` — zero-copy, nanosecond write.           │
└──────────────────────────────────────────────────────────────────┘
```

The graph IR is the **blueprint**. The journal is the **bookmark**.
Multiple concurrent runs share one IR. Each run has its own journal record.
A crashed run restarts, reloads the IR from source, reads its journal, and resumes from the first incomplete node.

This also means: **the IR shape can change freely** between compiler versions without breaking any host code. The host only depends on the generated types file, not the IR.

---

## Part A: Parser Finalisation — `return ... with turns`

Before touching the IR, we add `with turns` to the parser. The IR lowerer needs it in the AST.

### What `with turns` means

From `not.txt` (lines 354–380) and `V2_GRAPH_IR_FINAL.md` (section 5.7):

```quew
agent Main(input: Text) {
    if inputType.data.includes("high") {
        return One(input) with turns   // exits Main, child turns visible in parent context
    }
    return Two(input) with turns
}
```

**Without `with turns`** — the parent context records only:
```
{ user: original_input, model: final_output }
```
The child agent's internal tool calls and model turns are invisible.

**With `with turns`** — the parent context carries the child's full turn trace. A cursor marks where the child started. The parent and child share one journal context; there is no nesting.

This is a **context pipeline** directive — not about what the child does, but about how much of the child's transcript is merged into the parent's journal context.

From `not_graph.txt` (line 159):
> If you call it **transparent** (`with turns`), the child's nodes are inlined into the parent graph, so they all share one state object. There is no nesting.

### AST change — `quew-ast/src/stmt.rs`

```rust
/// Controls how a delegating `return` merges the child's context into the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMode {
    /// Default: parent journal records only the child's final output.
    Normal,
    /// `with turns`: child nodes are inlined into the parent graph.
    /// Parent journal carries the full child turn trace.
    WithTurns,
}

pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub mode:  ReturnMode,   // NEW — defaults to Normal
    pub span:  Span,
}
```

### Lexer change — `quew-lexer`

Check if `KwTurns` exists. If not, add it. `turns` becomes a reserved keyword — no variable can be named `turns`. This is a deliberate constraint: the word is too semantically loaded in this DSL to allow shadowing.

### Parser change — `quew-parser/src/parse_stmt.rs`

After the return expression, optionally parse `with turns`:

```rust
let return_stmt = just(TokenKind::KwReturn)
    .ignore_then(e.clone().or_not())
    .then(
        just(TokenKind::KwWith)
            .ignore_then(just(TokenKind::KwTurns))
            .to(ReturnMode::WithTurns)
            .or(empty().to(ReturnMode::Normal))
    )
    .map_with(|(value, mode), extra| {
        Stmt::Return(ReturnStmt { value, mode, span: to_span(extra.span()) })
    });
```

### Checker change

`Stmt::Return` already validates the value's type. `mode` is transparent to the checker — it carries no type-level meaning. The checker passes it through to the IR.

### Tests

- `return Agent(x) with turns` → `ReturnMode::WithTurns`
- `return Agent(x)` → `ReturnMode::Normal`
- `return expr with turns` on a non-agent value → checker does not error (it's an IR concern); the IR lowerer decides what to do with a non-agent `with turns`

---

## Part B: IR Lowering — `quew-ir`

### Single Responsibility

`quew-ir` takes a `CheckResult` and emits a `QuewGraphIR`.
`QuewGraphIR` is a native Rust struct — no JSON file, no external serialization format.
The runtime loads it directly.

It must **not**:
- Serialize to JSON (that would make it a v1-style external artifact)
- Run user code
- Contact any external service
- Mutate execution state

The only output is the in-memory `QuewGraphIR` struct.

### What the Compiler Actually Emits (per discussion1.md §7)

```
.quew source
  ↓
quew-ir lowers AST → QuewGraphIR (native Rust struct, in-memory)
  ↓
quew-codegen walks QuewGraphIR → emits RuntimeTest.ts  ← the ONLY file written to disk
  ↓
Runtime starts with QuewGraphIR loaded
```

The TypeScript file is ~40 lines of types and one factory function. No JSON import. No intent loop.

### Crate Dependencies

```toml
[dependencies]
quew-ast      = { path = "../quew-ast" }
quew-checker  = { path = "../quew-checker" }
quew-types    = { path = "../quew-types" }
quew-interner = { path = "../quew-interner" }
indexmap      = "2"
# No serde here — IR is in-memory Rust structs.
# Journal serialization belongs in the runtime crate, not here.
```

---

### IR Type Definitions

The types use `InternedStr` throughout — no heap `String` for names.
`IndexMap` preserves insertion order (important for deterministic node IDs).

```rust
/// The complete compiled program for one `.quew` source file.
/// This is a native Rust struct. It is never serialized to disk by the compiler.
/// The runtime holds it in memory and shares it across all concurrent runs.
#[derive(Debug, Clone)]
pub struct QuewGraphIR {
    pub program:     ProgramMeta,
    pub definitions: Definitions,
    /// Key: graph id string like "agent:Main" or "function:sanitize"
    pub graphs:      IndexMap<InternedStr, AgentGraph>,
}

#[derive(Debug, Clone)]
pub struct ProgramMeta {
    pub name:        InternedStr,
    pub entry_agent: InternedStr,
}

#[derive(Debug, Clone)]
pub struct Definitions {
    pub types:     IndexMap<InternedStr, TypeDef>,
    pub models:    IndexMap<InternedStr, ModelDef>,
    pub tools:     IndexMap<InternedStr, ToolDef>,
    pub functions: IndexMap<InternedStr, FunctionDef>,
    pub agents:    IndexMap<InternedStr, AgentDef>,
}
```

#### Node Types

Node categories follow discussion1.md §10:
- **Boundary** — `Input`, `Context`, `Output`
- **Deterministic** — `LetBind`, `FuncCall` (pure), `Branch` — replayable, no checkpoint by default
- **Effectful** — `HostToolCall`, `AgentCall`, `Reply` — always checkpointed

```rust
#[derive(Debug, Clone)]
pub struct AgentGraph {
    pub graph_id:    InternedStr,   // "agent:Main", "function:sanitize"
    pub entry_node:  NodeId,
    pub return_node: NodeId,
    pub nodes:       Vec<IrNode>,
    pub edges:       Vec<Edge>,
}

/// A stable, deterministic node identifier.
/// Format: "nN" where N is the sequential index within this graph.
/// The journal uses (graph_id, node_id) as its checkpoint key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone)]
pub struct IrNode {
    pub id:         NodeId,
    pub kind:       NodeKind,
    /// Whether the journal must checkpoint before/after this node.
    pub checkpoint: CheckpointPolicy,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    // ── Boundary ──────────────────────────────────────────────────────────────
    Input   { input_ty: IrType },
    Context { context_ty: InternedStr },
    Output  { value: DataRef },

    // ── Deterministic ─────────────────────────────────────────────────────────
    /// `let x = expr` — pure expression evaluation
    LetBind { name: InternedStr, value: IrExpr },
    /// `if cond { } else { }` — branches to sub-graph entry nodes
    Branch  { condition: DataRef, then_node: NodeId, else_node: Option<NodeId> },
    /// Pure function call — inlined or referenced sub-graph
    FuncCall { function: InternedStr, args: IndexMap<InternedStr, DataRef> },

    // ── Effectful ─────────────────────────────────────────────────────────────
    /// `tool x()` in agent code — calls out to the host FFI
    HostToolCall { tool: InternedStr, args: IndexMap<InternedStr, DataRef> },
    /// `reply(input) with { ... }` — the LLM boundary; owns the full conversation loop
    Reply   { message: DataRef, config: ReplyConfig },
    /// `return Agent(input)` or `return Agent(input) with turns`
    AgentCall { agent: InternedStr, args: IndexMap<InternedStr, DataRef>, mode: AgentCallMode },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCallMode {
    /// Parent journal records only the child's final output.
    BlackBox,
    /// Child nodes inlined into parent graph. Full turn trace in parent journal.
    WithTurns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointPolicy {
    /// Journal must save before and after this node. Required for all effectful nodes.
    Required,
    /// Journal may save (useful for debugging). Default for deterministic nodes.
    Optional,
    /// Never save. Used for boundary nodes and trivial expressions.
    Never,
}
```

#### Reply Config

```rust
#[derive(Debug, Clone)]
pub struct ReplyConfig {
    pub prompt:   IrPrompt,
    pub model:    ModelRef,
    pub fallback: Option<ModelRef>,
    pub retry:    Option<u32>,
    pub max_turn: Option<u32>,
    pub tools:    Vec<ToolRef>,
    pub builtin:  Vec<InternedStr>,
}

#[derive(Debug, Clone)]
pub enum IrPrompt {
    Literal(InternedStr),
    // Future: template with interpolation slots
}

#[derive(Debug, Clone)]
pub enum ModelRef {
    Named(InternedStr),     // `model: Gemini` — refers to a named model in definitions
    Inline(ModelDef),       // `model: gemini("gemini-pro")` — anonymous inline
}

#[derive(Debug, Clone)]
pub struct ToolRef {
    pub name:      InternedStr,
    /// Pre-bound host args from `delete_person(ctx.isAdmin)` → host_args["isAdmin"]
    pub host_args: IndexMap<InternedStr, DataRef>,
}

/// A reference to data produced by a prior node in the same graph.
#[derive(Debug, Clone)]
pub struct DataRef {
    pub node: NodeId,
    /// Field name if the node output is a record type. None for scalar outputs.
    pub slot: Option<InternedStr>,
}
```

#### Data Flow Edge

```rust
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: NodeId,
    pub to:   NodeId,
    pub slot: InternedStr, // which input slot of `to` receives this value
}
```

---

### Lowering Rules (AST → IR Nodes)

| DSL construct | IR representation |
|---|---|
| Agent input param | `Input` boundary node |
| `@context(Type)` annotation | `Context` boundary node |
| `let x = pure_expr` | `LetBind` deterministic node |
| `let x = f(args)` where `f` is internal function | `FuncCall` deterministic node |
| `let x = tool(args)` in agent code | `HostToolCall` effectful node |
| `if cond { } else { }` | `Branch` deterministic node; branches reference sub-entry nodes |
| `reply(input) with { }` | `Reply` effectful node; full config lowered |
| `return expr` (non-agent) | `Output` boundary node |
| `return Agent(input)` | `AgentCall { mode: BlackBox }` effectful node |
| `return Agent(input) with turns` | `AgentCall { mode: WithTurns }` effectful node |
| `tool x()` top-level decl | `ToolDef { kind: Host }` in definitions |
| `@tool function f() { }` | `ToolDef { kind: Dsl }` in definitions; body → `graphs["function:f"]` |
| `function f() { }` (non-tool) | `FunctionDef` in definitions; body → `graphs["function:f"]` |
| Inline `gemini("...")` | Anonymous `ModelDef` interned in definitions |
| Named `model M = { }` | Named `ModelDef` in definitions |
| `type T = { }` | `TypeDef` in definitions |

### `reply` config lowering detail

The `with { }` block fields lower as follows:

| With field | IR field | Rule |
|---|---|---|
| `model: gemini("...")` | `config.model = ModelRef::Inline(...)` | Provider call → anonymous inline |
| `model: MyModel` | `config.model = ModelRef::Named("MyModel")` | Ident → named ref |
| `fallback: groq("...")` | `config.fallback = Some(...)` | Same rules as model |
| `prompt: "..."` | `config.prompt = IrPrompt::Literal(...)` | String literal |
| `retry: 3` | `config.retry = Some(3)` | Number literal |
| `maxTurn: 5` | `config.max_turn = Some(5)` | Number literal |
| `tools: [getWeather]` | `config.tools = [ToolRef { name: "getWeather", host_args: {} }]` | Bare ref |
| `tools: [delete_person(ctx.isAdmin)]` | `config.tools = [ToolRef { name: "delete_person", host_args: { "isAdmin": DataRef { node: ctx_node, slot: "isAdmin" } } }]` | Pre-bound call |
| `builtin: [web_search]` | `config.builtin = ["web_search"]` | String idents |

---

### Generated TypeScript File (the ONLY disk artifact)

The codegen pass (`quew-codegen`) walks `definitions` from the `QuewGraphIR` and emits:

```typescript
// Auto-generated — do not edit
// Source: hello.quew

import { createAuwgent } from "@snrraptopack/auwgent-sdk"
import type { AuwgentMiddleware as BaseMiddleware } from "@snrraptopack/auwgent-sdk"

export type Input   = string
export type Output  = string

export type AuwgentContext = {
    isAdmin: boolean
    userId:  string
}

export type AuwgentTools = {
    getWeather:  (args: { city: string })  => Promise<string>
    get_marks:   (args: { id: string })    => Promise<string>
}

// DSL-defined tools (@tool function) do NOT appear here — runtime owns them

export type AuwgentMiddleware = BaseMiddleware<AuwgentContext, Output>

export type AuwgentIntent =
    | "response_text"
    | "response_schema"
    | "tool_call"
    | "tool_result"
    | "error"

export type AuwgentConfig = {
    apiKeys:    { geminiApiKey?: string; groqApiKey?: string }
    context?:   AuwgentContext
    tools?:     Partial<AuwgentTools>
    middleware?: AuwgentMiddleware[]
}

export function auwgent(config: AuwgentConfig) {
    return createAuwgent(config)
}
```

Rules:
- Only `tool` (host-backed) declarations appear in `AuwgentTools` — `@tool function` is invisible
- Context type comes from `@context(Type)` annotation on the entry agent
- API keys are inferred from which providers appear in `definitions.models`
- No JSON IR import. No intent handler list. ~40 lines.

---

## Plan 5 Execution Order

### Step 1: Add `with turns` to the parser

1. Add `KwTurns` to `quew-lexer` (reserved keyword)
2. Add `ReturnMode` enum to `quew-ast/src/stmt.rs`
3. Add `mode: ReturnMode` field to `ReturnStmt`
4. Update `parse_stmt.rs` to parse optional `with turns` suffix
5. Update `quew-checker` — forward `mode` through `Stmt::Return` without semantic validation
6. Update all existing tests that pattern-match on `ReturnStmt` to include `mode`
7. New parser tests: `return Agent(x) with turns` → `WithTurns`; `return x` → `Normal`

### Step 2: Define `quew-ir` types

1. Scaffold `Cargo.toml` (no `serde` dep — IR is not serialized)
2. Define `QuewGraphIR`, `Definitions`, `AgentGraph`, `IrNode`, `NodeKind`, `ReplyConfig`, `DataRef`, `Edge`, `AgentCallMode`, `CheckpointPolicy`
3. Define `IrType` — a simplified representation of `Ty` as lowered into the graph
4. Add unit tests for type construction (can the structs be built correctly)

### Step 3: Implement lowering

1. `lower(module, check_result) -> QuewGraphIR` — top-level entry point
2. `lower_definitions(items)` — populate types, models, tools, functions, agents
3. `lower_graph(agent_decl, symbol_table)` — produce one `AgentGraph` per agent
4. `lower_stmt(stmt, ctx)` — emit nodes per statement; `ctx` carries the node counter and data slot map
5. `lower_reply_config(with_block, ctx)` — maps `with` block fields to `ReplyConfig`
6. `lower_tool_list(array_expr, ctx)` — maps tool array elements to `Vec<ToolRef>` with `host_args`
7. `lower_expr(expr, ctx) -> DataRef` — maps expressions to data refs or emits inline `LetBind` nodes

### Step 4: Wire into `quew-cli`

1. `quew compile <file.quew>` → lex → parse → check → lower → print summary (node count, edge count, definitions)
2. Non-zero exit if any diagnostics have `Severity::Error`
3. Print diagnostics to stderr with source context (ariadne)
4. `quew check <file.quew>` — runs only through the checker, no IR lowering

> The CLI does not write any output file in this plan. It validates that the full pipeline
> runs end-to-end and the IR is produced without panicking. TypeScript codegen is a separate plan.

---

## Deliberately Deferred

| Item | Reason |
|---|---|
| TypeScript codegen | Ported from v1 when language is stable — not needed to prove the compiler |
| Python / Dart codegen | After TypeScript codegen is settled |
| Journal / execution state | Runtime concern — Plan 6+ |
| Graph executor / scheduler | Runtime concern — Plan 6+ |
| Cross-file imports | No `import` syntax yet |
| Generics | No syntax yet |
| `for` loop nodes | Design open: inline unroll vs loop node |
| DSL middleware bytecode VM | Syntax still moving; Plan 7+ |
| Binary artifact (`.awc`) | Add when the problem is actually felt |
| v1 `AgentIR` compatibility bridge | After v2 IR is stable |

---

## Test Strategy

All IR lowering tests follow:
```
source string → lex → parse → check (0 errors) → lower → assert on QuewGraphIR struct fields
```

No snapshot JSON files. We assert on the Rust struct directly — specific node kinds, counts, edges, and config field values.

**Coverage targets:**
- Basic agent with one `reply` → exactly 3 nodes: `Input`, `Reply`, `Output`
- Agent with `@context` → `Input`, `Context`, `Reply`, `Output`
- Agent with `let x = f(args)` before `reply` → `FuncCall` node before `Reply`
- Agent with `if/else` → `Branch` node; then/else branches reference correct sub-nodes
- `return One(input)` → `AgentCall { mode: BlackBox }`
- `return One(input) with turns` → `AgentCall { mode: WithTurns }`
- Inline `gemini("gemini-pro")` → anonymous `ModelDef` in definitions
- Named `model Gemini` → named `ModelDef` in definitions
- `tools: [getWeather]` → `ToolRef { host_args: {} }`
- `tools: [delete_person(ctx.isAdmin)]` → `ToolRef { host_args: { "isAdmin": DataRef { ... } } }`
- Every node id in a graph is unique
- Every edge references existing node ids
- `entry_node` and `return_node` exist in the node list
- Only `tool` (host) declarations in codegen output — `@tool function` absent
