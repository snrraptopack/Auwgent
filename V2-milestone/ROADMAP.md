# Quew Roadmap

> Status: canonical sequencing document, 2026-08-22.
> Direction reference: `POSITIONING.md`. Current state: `quew-compiler/GAPS.md`.
> Regression fixtures: `quew-compiler/q_tests/` (run via `q_tests/run_q_tests.ps1`).

## Where We Are (post-audit + real-IO slice, 2026-08-25)

The deterministic core is real and tested: lex → parse → check → lower → execute
works for functions, branches (structural span-based skipping), both loop kinds
(with capture write-back), break/continue, **early returns from any branch
depth**, natives dispatch, string interpolation (numbers/bools stringify),
object/array values, **`any` type, honest JSON builtins, and working HTTPS
`fetch` with method/headers/body/timeout**. ~500 tests green across both
workspaces; 13/13 q_tests fixtures.

What is NOT real yet: LLM execution (`Reply`/`HostToolCall`/`AgentCall` are
rejected), checkpoints/resume (`CheckpointPolicy` is IR decoration; the executor
ignores it; `Value` isn't serializable), middleware (lexer-only tokens).

Audit + IO-slice fixes landed since the first audit:

1. Deleted stub crates `quew-analysis`, `quew-resolve`, `quew-codegen`.
2. Branch skipping: prediction → back-patching → **structural arm spans**
   (edge-chasing silently failed on multi-statement branch bodies).
3. `return` is now a real graph node: short-circuits from any nesting depth.
4. Dynamic model selection rejected with a clear checker diagnostic.
5. Prelude auto-registration via `include_dir` — new prelude files need no
   Rust edits.
6. `any` primitive end-to-end (types, IR, runtime, `is` checks).
7. Honest JSON stdlib (`json_parse/json_get` return real values, null on miss).
8. Real `fetch(url, config?)` with rustls TLS, method/headers/body/timeout,
   structured `{ status, ok, body, error }` response.
9. Optional trailing parameters allowed at call sites.
10. String escapes processed by the parser (`\"`, `\n`, ...); interpolation
    accepts numbers/bools/`any`; fixed duplicated-text bug for unterminated
    `{` in strings.
11. Loop iteration caps + recursion depth caps + richer data-ref errors +
    float div-by-zero errors.
12. Runtime re-keys expression-call args from callee signature (masks lowerer's
    self-recursion `arg0` fallback until two-pass registration lands).

## The Ordering Principle

Everything sequenced backwards from the product promise:
**"survive from every crash"** + **the graph is the contract**.
Resume design comes before LLM nodes because the LLM nodes are the hard-to-
checkpoint ones; their state model must be designed first, not retrofitted.

---

## Phase 1 — Resume Design (design doc, then types)

**Gate: nothing else starts until this exists.**

1. Write `V2-milestone/plans/RESUME_DESIGN.md` answering:
   - What exactly is checkpointed at each node boundary (journal of
     `NodeId → Value` outputs + bindings? event-sourced effects?)
   - Mid-`Reply` crash semantics: partial stream, pending tool calls,
     continuation token from provider.
   - Program reload strategy: **decide one** — (a) serde on
     `QuewGraphIR`/`Value`, or (b) deterministic recompile from `.quew` +
     content hash check. Recommendation: (a) for runtime portability
     (WASM/embedders can't recompile), content-hashed.
   - `CheckpointPolicy` enforcement semantics per node kind.
2. Add serde to `Value` and `QuewGraphIR` (feature-gated).
3. Journal type in quew-runtime: append-only `Vec<(NodeId, Value)>`,
   replay = pre-seed `outputs` map. `Execution::run` already skips nodes
   whose output exists — resumption slots into that.

**Exit:** a test that runs half a graph, serializes state, drops the process
state, reconstructs, and finishes — all without LLM nodes.

## Phase 2 — Middleware (parse → check → lower → interpret)

Lexer already emits `@middleware`/`@middlewares`; parser drops them.

1. Parse middleware functions into AST (event param, if-on-event pattern).
2. Checker: validate event API calls (`getPrompt`, `setSession`, `skip`,
   `override`, ...) against a fixed `MiddlewareEvent` shape; decide naming
   (`runStart` vs `run_start`) once, here.
3. Lower to a standalone middleware graph (not the main graph — middleware
   stays out of the execution graph per POSITIONING).
4. Runtime: effect-recording interpreter. Middleware mutates an effect object;
   runtime validates + applies effects; effects are journaled (Phase 1 types).
5. DSL middleware runs first, host middleware second.

**Exit:** `@middleware("prompt-prefix")` mutating a prompt works end-to-end
with a fake driver.

## Phase 3 — LLM Execution Nodes

1. `Reply` executor node: builds messages, calls driver, streams, feeds
   orchestrator/native path, journals at every boundary.
2. `HostToolCall`: registry lookup (host tools registered by embedder),
   checkpoint before/after (tool may have side effects — never re-run silently).
3. `AgentCall` (+ `with turns` mode): sub-graph run, blackbox vs transparent.
4. Drivers: **port v1 `auwgent-drivers`** (OpenAI + Gemini) — do not rewrite;
   streaming, tool-call normalization, retry/fallback logic is battle-tested.
5. Protocol decision (document it): block protocol vs native-first for v2.
   Silent carry-over from v1 is not acceptable.

**Exit:** an agent that replies, calls a host tool, resumes from a simulated
crash mid-stream, and completes.

## Phase 4 — Runtime Hardening

**Status: mostly DONE (2026-08-22).**

1. ✅ Loop iteration caps (100k default) and recursion depth limit (64 default)
   → clean errors, configurable via `ExecutionLimits`.
2. ✅ Richer `resolve_data_ref` errors (names the missing node/field).
3. ⬜ Loop fast path: avoid full body-graph re-walk per iteration
   (~42µs/iter release; target <2µs/iter). Benchmark anchor:
   q_tests/stress_while_100k.quew.
4. ⬜ Honest stdlib errors: stop returning `"parse error..."` strings as success
   values (blocked on error-handling design — see EMBEDDABILITY.md §1.1).
5. ✅ Float/int division consistency; removed latent `unreachable!()`.
6. ✅ Expression-level graph calls now share the executor (limits + depth apply).
7. ✅ Fixed recursive-call argument packaging at runtime (lowerer's self-recursion
   `arg0` fallback masked by positional re-keying). Compiler-side proper fix
   (two-pass definition registration) tracked below.
8. ⬜ Flatten wrapped error chains from deep recursion.
9. ⬜ Compiler: two-pass lowerer definition registration so expression-call arg
   names are correct without runtime re-keying.

## Phase 5 — Surface & Interop

1. `@entry` annotation (replaces reverse-scan heuristic).
2. CLI diagnostics with file:line:col spans (SourceMap exists, unused).
3. Prelude handling: compile prelude once to a cached definitions blob instead
   of re-lowering into every user IR.
4. Codegen reborn as real crate(s): TS first (SDK handoff), snapshot-tested.
5. WASM target: same ExecutionGraph, `wasm-bindgen` wrapper (v1 pattern proven).

---

## Deliberately Deferred (do not pull forward)

- Bytecode compilation of middleware (after syntax stabilizes).
- Tool groups progressive disclosure internals.
- Provider builtin tools (`builtin: [web_search]`) beyond validation.
- Async/await syntax in quew.
- Multi-file modules / import resolution (single-file until SDK pressure demands it).

## Definition Of Done (per phase)

- All existing tests green (`cargo test --workspace` × 2, `q_tests/run_q_tests.ps1`).
- New public items tested (README testing mandate).
- GAPS.md updated in the same change.
- No new language surface without a POSITIONING checklist pass.
