# Discussion 3: Plan 5 Completion, Readiness, and Runtime Direction

*Written after completing Plan 5.*

---

## What Plan 5 Was Trying To Prove

Plan 5 was the bridge from a validated language frontend into the v2 execution
architecture. The goal was not to build the runtime yet. The goal was to prove
that a checked `.quew` module can lower into a native, in-memory graph that the
future Rust runtime can own directly.

This keeps the v2 rule intact:

```
source -> compiler -> QuewGraphIR in memory -> Rust runtime
```

No JSON IR is emitted for the host. The host remains responsible for tools,
middleware hooks, and transport. It does not drive the program.

---

## What Was Completed

### Parser Finalisation

`return Agent(input) with turns` is now represented in the AST through
`ReturnMode::WithTurns`.

That matters because `with turns` is not a type-system feature. It is an
execution-mode directive for child-agent handoff. The checker validates the
return value as usual; the IR lowers the handoff mode.

### Graph IR Types

`quew-ir` now defines the native graph model:

- `QuewGraphIR`
- `Definitions`
- `AgentGraph`
- `NodeId`
- `IrNode`
- `NodeKind`
- `ReplyConfig`
- `DataRef`
- `Edge`
- `CheckpointPolicy`
- `AgentCallMode`

The important performance improvement from the first draft is already applied:
`AgentGraph.nodes` is `IndexMap<NodeId, IrNode>`, not `Vec<IrNode>`.
This keeps deterministic insertion order and avoids linear node lookup during
execution.

### Static Definitions Lowering

The lowerer now populates static definitions for:

- types
- named and inline models
- host tools
- tool groups
- normal functions
- DSL-defined tool functions
- agents

Tool definitions keep the important v2 distinction:

- `tool name(...)` is host-backed and appears in generated host tool types later.
- `@tool function name(...)` is runtime-owned and should not be registered by the host.

### Protocol Mode: `@native` and `@block`

The v2 IR now preserves agent protocol mode explicitly:

```rust
pub enum ProtocolMode {
    Block,
    Native,
}
```

`@native` lowers to `ProtocolMode::Native`.
`@block` lowers to `ProtocolMode::Block`.
No annotation defaults to block.

This carries forward the v1 behavior where `@native` / `@block` changed the
runtime/SDK execution mode. In v2 this must feed the runtime's `Reply` execution
path rather than becoming a host-visible JSON field.

### Graph Lowering

The compiler now lowers core agent bodies into graph nodes:

- `Input`
- `Context`
- `LetBind`
- `FuncCall`
- `HostToolCall`
- `AgentCall`
- `Reply`
- `Branch`
- `Output`

`return Agent(input)` lowers to `AgentCall { mode: BlackBox }`.
`return Agent(input) with turns` lowers to `AgentCall { mode: WithTurns }`.

`reply(...) with { ... }` lowers into `ReplyConfig`, including:

- prompt
- model
- fallback
- retry
- maxTurn
- tools
- builtin
- agents

### CLI Wiring

The CLI now has the Plan 5 commands:

```text
quew check <file>
quew compile <file>
```

`compile` runs the full frontend and IR lowering, then prints a summary:

```text
compile ok
entry agent: Hello
definitions: ...
graphs: ...
```

It writes no IR file.

---

## Current Test State

The quew workspace passes:

```text
cargo test --workspace
```

Additional Plan 5 tests were added for:

- direct node lookup and insertion order
- type definition lowering
- model lowering
- host tool lowering
- basic source-to-graph reply lowering
- `with turns` lowering
- `@native` protocol lowering

---

## Areas Worth Testing More

The frontend is strong, but before runtime work depends on this graph shape we
should add more grammar and type-lowering tests around the most semantically
important language patterns from `not.txt`.

Recommended additions:

1. Dynamic `with` fields:
   - `let selected = [getWeather]`
   - `tools: selected`
   - `model: selectedModel`

2. Tool prebinding:
   - `tools: [delete_person(ctx.isAdmin)]`
   - multiple host args such as `ctx.user_id, ctx.isAdmin`
   - optional host/model params

3. Context:
   - `@context(Context)` plus `ctx.field`
   - reject invalid context field access
   - ensure context node and `DataRef::field` are generated

4. Branch lowering:
   - `if/else` with reply in each branch
   - `if/else` returning different agents
   - branch edges reference real nodes

5. Function and DSL-tool bodies:
   - normal `function f()`
   - `@tool function f()`
   - ensure host-backed tools and DSL-defined tools are separated in definitions

6. Return shapes and type compatibility:
   - `Text`
   - named object return
   - union return such as `Response | Response2 | Text`

7. Protocol annotations:
   - default block
   - explicit `@block`
   - explicit `@native`
   - later: `@block` plus media type should error when media input lands in quew

These are not blockers for Plan 5 being complete, but they are worth doing
before building too much runtime behavior on top.

---

## What Is Not Plan 5

These are deliberately not finished here:

- graph executor
- scheduler
- journal / checkpoint backend
- provider drivers
- runtime `Reply` loop
- middleware VM
- TypeScript/Python/Dart codegen
- generated SDK files
- binary artifact caching

They belong to Plan 6 and later.

---

## Way Forward

The next plan should start real execution, but it should be scoped tightly.

Recommended Plan 6 goal:

> Execute a `QuewGraphIR` in Rust with an in-memory journal, no provider calls
> at first, proving deterministic nodes, host tool nodes, agent calls, and resume
> keys.

Suggested order:

1. Define runtime value representation.
2. Define journal trait and in-memory journal implementation.
3. Execute boundary and deterministic nodes.
4. Execute host tool calls through a mock registry.
5. Execute `AgentCall` black-box mode.
6. Record node status and output by `(run_id, graph_id, node_id)`.
7. Add resume tests that skip completed nodes.
8. Only then implement `Reply` as the LLM boundary.

`Reply` is the largest node because it owns model streaming, tools exposed to
the model, native/block protocol branching, retry, fallback, and partial output.
It should not be the first executor milestone.

The clean path is:

```
Plan 6A: graph executor + journal, no LLM
Plan 6B: Reply node in block mode
Plan 6C: Reply node in native mode using ProtocolMode
Plan 6D: SDK bridge and generated host types
```

That keeps v2 aligned with the original goal: the Rust runtime owns execution,
the host supplies tools and observes events, and the compiler output stays
native and internal.
