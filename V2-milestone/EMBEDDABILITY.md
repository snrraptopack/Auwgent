# Quew Embeddability & DX

> Status: direction reference, 2026-08-22. Companion to `POSITIONING.md`.
> This document defines what "feels like a language, not config" means
> concretely, the embedding contract, and the demo that must work.

---

## 1. Config vs Language

Config feels like config because it is **inert** — someone else's engine
interprets your declarations. A language feels like a language when the
author's control flow *is* the program.

Quew already has the right bones: `if`, loops, mutation, functions,
agents-as-callables, `reply(...) with {}`. Three things break the feeling:

### 1.1 No error handling (worst offender)

```quew
// today: defensive string-smelling everywhere
let result = delete_user(id)
if result.error.isEmpty() {
    return "deleted"
}
return result.error
```

Agents live in a world where everything fails — network, rate limits, bad
JSON. Until quew has a first-class failure story (`try`/`else` or a
`Result<T>` type with propagation), every program reads like boilerplate.
This also fixes the dishonest stdlib (`json_parse` returning
`"parse error: ..."` as a *successful string value*).

**Priority: #1 language feature — before anything else in Phase 3+,**
because it touches checker, IR, stdlib, and `Value` all at once.
Design doc: `plans/ERROR_HANDLING.md` (to be written).

### 1.2 No feedback loop

Languages feel alive when save → run → see is instant. Today that is a cargo
build. Requirements:

- `quew run file.quew` sub-second from a cached release binary
- watch mode later; REPL optional

DX is latency.

### 1.3 No inspection

Agent frameworks win or die on debuggability. The graph IR makes this nearly
free and nobody else has it:

```
$ quew trace weather_agent.quew
  0 Input            input = "weather in tokyo?"
  1 LetBind  cond    = input.contains("weather") -> true
  2 Branch     [taken: then]
  3 HostTool getWeather(city) ...
  ...
```

Node-by-node walk with bindings at each step. This does more for the
"this is it" feeling than ten features.

---

## 2. The Embedding Contract

Make the runtime feel like **SQLite**: one artifact, three verbs, zero ceremony.

```
run(graph, input) → stream of events          // execute
register(tool_name, fn, type_signature)       // bind host capabilities
listen(event, handler)                        // middleware
```

Rules:

1. **The embedder never touches IR internals.** Generated stubs keep host
   registrations type-honest against the `.quew` declarations (§4).
2. **Values are boring.** String / number / bool / null / object / array —
   JSON-shaped in every host. Nothing fancy ever crosses the boundary.
3. **Pause is a first-class API**, not just crash recovery:
   `engine.checkpoint()` / `engine.resume(state)`. Same machinery as
   durability. Long-running human-in-the-loop agents become trivial.
4. **Deterministic core + injected capabilities.** Same graph + same inputs +
   same tool results = same execution. Enables replay, debug, time-travel.
5. **Tiny footprint.** Executor stays small enough for WASM/browser and
   in-process Rust embedding. Async only where LLM streaming forces it.

Host languages: Rust first (zero FFI, `#[quew_builtin]` + inventory), then
TypeScript / Python / Dart via thin embedders over the same serialized graph.
Universal interop = the graph is the contract.

---

## 3. Give / Remove

**Give:** error handling, `quew trace`, `@entry`, honest fetch/json errors,
sub-second run loop, module system *only* when multi-file pain is real.

**Remove / resist:** async syntax, classes, inheritance, any I/O primitive in
the language proper. Each one kills an embedder and grows the surface every
SDK target must reimplement.

---

## 4. DX Example — the full picture

One agent file, one host file, generated stubs, checkpoint/resume, trace.
This is the target experience, not current state.

### 4.1 `support_agent.quew`

```quew
type Context = {
    user_id: string
    is_admin: bool
}

tool get_order(order_id: string): Order @desc "Look up an order by id"

tool refund(order_id: string): RefundResult @desc "Refund an order"

// Deterministic policy in the DSL — the model never sees raw refund.
@tool(order_id: string)
@desc "Refund an order after eligibility check"
function safe_refund(is_admin: bool): string {
    if not is_admin {
        return error("only admins can issue refunds")
    }

    let outcome = refund(order_id)

    if outcome.is_err() {
        return error("refund failed: {outcome.unwrap_err()}")
    }

    return "refund {order_id} completed"
}

@context(Context)
agent Support(input: Text): Text {
    let tools = if ctx.is_admin then [get_order, safe_refund] else [get_order]

    reply(input) with {
        prompt: "You are a support agent. Be concise."
        model: gemini("gemini-2.0-flash")
        tools: tools
        fallback: groq("llama-3.3-70b-versatile")
        retry: 2
        maxTurn: 4
    }
}
```

Note what the author wrote: **policy and composition only**. No HTTP, no
retry plumbing, no provider SDK imports, no session management. That is the
"language not config" test: the control flow above is the program.

### 4.2 Embedding in TypeScript

```ts
import { Engine } from "@auwgent/quew";
import { support_agent } from "./support_agent.quew.json"; // generated stubs
import { get_order, refund } from "./host_tools";          // your real code

const engine = await Engine.load(support_agent);

engine.register("get_order", get_order);   // signature checked against .quew
engine.register("refund", refund);

for await const event of engine.run({ text: "where is my order?" }) {
    if (event.kind === "response_text") process.stdout.write(event.data);
}
```

The stub generation is the interop moment that sells it: declare the tool in
quew, implement it in TypeScript, and the compiler keeps both sides honest —
wrong argument types are a compile error, not a runtime surprise.

### 4.3 Pause / resume (human-in-the-loop)

```ts
let state = engine.checkpoint();
saveToDb(ticketId, state);

// ... minutes or days later, maybe in another process ...
const state = await loadFromDb(ticketId);
const engine = await Engine.load(support_agent, { resume: state });
```

Crash recovery is the same machinery with no human involvement.

### 4.4 Inspection

```
$ quew trace support_agent.quew --input "refund my order"
  0 Input          input = "refund my order"
  1 Context        ctx = { user_id: "u_42", is_admin: false }
  2 LetBind  tools = [get_order]
  3 Reply          model=gemini-2.0-flash turn=1
  4 intent tool_call safe_refund(order_id="o_99")   [middleware: intent-guard]
  5 FuncCall safe_refund → error("only admins can issue refunds")
  6 Reply          model=gemini-2.0-flash turn=2
  7 response_text "I'm unable to issue refunds..."
```

Every node, every binding, every middleware intervention — because execution
is a graph walk over journaled outputs, this is a replay, not a log.

---

## 5. Litmus Test — "the sandwich"

The whole strategy compresses into this demo. When all four steps are one
command each and take under five minutes total for a newcomer, quew wins:

1. Write `support_agent.quew` — ~20 lines of policy.
2. Embed in a TypeScript/Rust app — ~5 lines plus the real tool functions.
3. Kill the process mid-run — resume from checkpoint, finish cleanly.
4. `quew trace` the whole thing afterwards and see exactly what happened.

---

## 6. Sequencing Impact (feeds ROADMAP.md)

- **ERROR_HANDLING.md design** — new, gates Phase 3+ (touches checker, IR,
  stdlib, Value).
- `RESUME_DESIGN.md` — unchanged as Phase 1 gate; pause API rides on it.
- `quew trace` — added to Phase 5 (cheap once journal exists in Phase 1).
- Codegen stubs — Phase 5 unchanged, but now explicitly part of the
  embedding contract, not a nice-to-have.
