# Quew Positioning

> Status: canonical direction reference. Every future plan should be checked
> against this document: does the feature belong in **agent land** or **tool land**?

## The One-Liner

*Write agent logic in a small typed language that compiles to a resumable
execution graph; run it anywhere — embedded in Rust, a server, or the browser —
and bind any host capability through typed tools.*

## The Mental Model

Quew is to AI agents what Lua is to games and Gleam is to Erlang:

```
┌─────────────────────────────────────────────┐
│  Quew programs (.quew)                       │  ← "agent land": logic, policy,
│  agents, reply-with, middleware, gating      │     composition — resumable, portable
├────────────── ExecutionGraph ────────────────┤  ← the artifact: serializable, WASM-able
├──────────────────────────────────────────────┤
│  Host capability layer (Rust-first)          │  ← "tool land": fetch, db, fs, your
│  NativeRegistry / @@rust builtins            │     crates, user code via SDKs
└──────────────────────────────────────────────┘
```

## What Quew Is

- A **policy and orchestration language**, not a general-purpose language.
- Home for the things that must survive crashes, be type-checked, and stay
  inspectable: control flow, authorization gating, model routing, output
  shaping, retry/fallback, middleware policy.
- Compiled to a portable `ExecutionGraph` artifact.

## What Quew Is Not

- Not a general-purpose programming language.
- Not a systems language. No manual resource management, no raw FFI syntax,
  no arbitrary host access from inside the language.
- Not a replacement for host code. Anything touching the outside world
  (HTTP, databases, filesystems, secrets, platform APIs) is a **host tool**.

## The Two Vocabularies, One Boundary Rule

| | Agent land (.quew source) | Tool land (host) |
|---|---|---|
| Lives in | `.quew` files | Rust (`#[quew_builtin]`), TS/Python/Dart SDK registrations |
| Contains | agents, functions, middleware, types, models, policy | fetch, db drivers, fs, secrets, native APIs |
| Executes as | ExecutionGraph nodes (checkpointable) | opaque registered callables |
| Seen by the graph as | full IR | a typed declaration + implementation pointer |

Rules that follow from this:

1. **Never add I/O syntax to quew.** Stdlib entries (`fetch`, `json`, `io`,
   `net`) are just pre-registered host tools. The `prelude/*.quew` files are
   the declared seam between the two lands.
2. **The boundary is typed.** Host tools are declared with DSL signatures
   (`tool getWeather(city: string): string`); implementations stay opaque.
3. **Every new feature must declare its land.** If it needs to survive a crash
   mid-execution and be replayed deterministically, it belongs in agent land.
   If it touches the outside world, it belongs in tool land.

## Rust Is the Privileged Host, Not the Only One

- **Rust embedding:** a Rust binary embeds the runtime, registers capabilities
  via `#[quew_builtin]` + `inventory`, and runs `.quew` programs with zero FFI cost.
- **Other hosts:** the same ExecutionGraph runs under thin embedders
  (TypeScript / Python / Dart SDKs, WASM runtime).
- **Universal interop = the graph is the contract,** not the Rust API.

## Portability Target

The compiled graph, serialized. From this single decision everything falls out:

- crash resume (checkpoint node state, reload program, continue)
- WASM execution (ship the graph to browser / edge / Cloudflare Workers)
- cross-language embedding (any SDK that can walk the graph)
- tooling (LSP, inspectors, visualizers read the same artifact)

Therefore: `quew-ir`'s `ExecutionGraph` must be serializable or
deterministically reloadable. This is a hard requirement, not an optimization.
See `plans/RESUME_DESIGN.md` (to be written) for the state model.

## Discipline: Keep the Language Small

Every feature added to quew proper is a feature that four SDK targets and a
WASM runtime must reimplement. Growth goes outward into tool land, not upward
into language complexity.

Things we deliberately do not add (for now):

- async/await syntax in quew
- classes / traits / inheritance
- modules with side effects at import time
- raw pointers / unsafe / direct FFI from DSL

## Consequences Already Decided

- Agents are typed callable units; `reply(...) with { ... }` is the model boundary.
- Functions and workflows collapsed into normal callable logic.
- Helpers became agent composition; handoff/trace policy replaces v1 helpers.
- Middleware is DSL-first (interpreted AST), effects recorded for resumability;
  host middleware remains for external integrations.
- Dual-mode block/native protocol decisions carry over from v1 where they serve
  the graph model (see `V2_GRAPH_IR_FINAL.md`).

## Checklist For New Plans

Before approving any plan, answer:

1. Which land does this live in? (If both, split it.)
2. Does it keep the boundary typed?
3. Is its state checkpointable, or does it deliberately opt out?
4. Can every embedder (Rust, TS, Python, Dart, WASM) do it via the graph contract?
5. Does it grow the language? If yes — can it be expressed in tool land instead?
