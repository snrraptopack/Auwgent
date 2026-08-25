# Quew Compiler Gap Analysis

> Last updated: 2026-08-22. Verified against the current tree (post-audit fixes).
> Runtime behavior claims are backed by executable fixtures in `q_tests/`.

## Legend
- ✅ Implemented
- ⚠️ Partial / Stub
- ❌ Not implemented

---

## 1. Top-Level Declarations

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| `agent` | ✅ | ✅ | ✅ | ⚠️ (graph walks; no LLM) |
| `function` | ✅ | ✅ | ✅ | ✅ |
| `tool` (host-backed) | ✅ | ✅ | ✅ | ❌ |
| `tools { ... }` group | ✅ | ✅ | ✅ | ❌ |
| `type` | ✅ | ✅ | ✅ | N/A |
| `model` | ✅ | ✅ | ✅ | N/A |
| `extend Type { ... }` | ✅ | ✅ | ✅ | ✅ |
| `@middleware` function | ❌ | ❌ | ❌ | ❌ |
| `@middlewares(...)` on agent | ❌ | ❌ | ❌ | ❌ |

**Gap:** Middleware is tokenized (`@middleware`, `@middlewares`) but never parsed into AST.

---

## 2. Statements

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| `let name = expr` | ✅ | ✅ | ✅ | ✅ |
| `let name: Type = expr` | ✅ | ✅ | ✅ | ✅ |
| `if / else` / `else if` | ✅ | ✅ | ✅ (back-patched branch targets) | ✅ |
| `return expr` | ✅ | ✅ | ✅ | ✅ |
| `return expr with turns` | ✅ | ✅ | ✅ | ❌ |
| `reply(expr) with { ... }` | ✅ | ✅ | ✅ | ❌ |
| `for idx, value in iterable` | ✅ | ✅ | ✅ | ✅ (mutated captures propagate back — verified in `q_tests/for_loop_sum.quew`) |
| `while` | ✅ | ✅ | ✅ | ✅ (verified in `q_tests/while_accumulator.quew`) |
| `break` / `continue` | ✅ | ✅ | ✅ | ✅ |
| `return expr` anywhere | ✅ | ✅ | ✅ (`Return` node) | ✅ short-circuits from any branch depth (`q_tests/early_return_if.quew`) |
| Expression stmt | ✅ | ✅ | ✅ | ✅ |

### Branch execution (structural spans)

Branch nodes carry inclusive `then_span` / `else_span` node ranges recorded by
the lowerer; the executor marks the untaken arm's whole span unreachable.
This replaced edge-chasing, which silently failed to skip multi-statement
branch bodies whose statements had no data dependency chain.

---

## 3. Expressions

| Feature | Status |
|---------|--------|
| Literals (int, float, bool, null) | ✅ |
| Strings `"..."` / `"""..."""` / interpolation | ✅ |
| Binary ops (+, -, *, /, %, ==, !=, <, <=, >, >=, and, or, =) | ✅ |
| Unary `not` | ✅ |
| Call / Provider `gemini("...")` / Member / Array / Object literal | ✅ |
| Postfix if `a if cond else b` | ✅ (rejected in `model:` position with a clear diagnostic) |
| Type check `x is Type` | ⚠️ best-effort for records (`Value::Object` shape check only) |

---

## 4. `reply() with { ... }` Config Block

| Field | Parser | Checker | Lowered | Notes |
|-------|--------|---------|---------|-------|
| `prompt` | ✅ | ✅ | ✅ | |
| `model` / `fallback` | ✅ | ✅ | ✅ | Dynamic selection (`A if c else B`) now a checker **error**, not a lowering panic |
| `retry` / `maxTurn` | ✅ | ✅ | ✅ | |
| `tools` | ✅ | ✅ | ✅ | Bare refs AND prebound calls `tool(ctx.arg)` |
| `builtin` / `agents` | ✅ | ✅ | ✅ | |

---

## 5. Annotations

| Annotation | Lexer | Parser | Checker | IR |
|------------|-------|--------|---------|-----|
| `@native` / `@block` | ✅ | ✅ | ✅ | ✅ |
| `@context(Type)` | ✅ | ✅ | ✅ | ✅ |
| `@desc "..."` | ✅ | ✅ | ✅ | ✅ |
| `@tool(...)` | ✅ | ✅ | ✅ | ✅ |
| `@middleware("name")` | ✅ | ❌ | ❌ | ❌ |
| `@middlewares(...)` | ✅ | ❌ | ❌ | ❌ |

---

## 6. Type System

Primitives, named/record types, optional `?`, unions, generics on types and functions,
agent input/output types, context types — all ✅.

---

## 7. Native / Standard Library

| Feature | Status |
|---------|--------|
| `@@rust("id")` builtin marker + `NativeRegistry` + `#[quew_builtin]` + `inventory` | ✅ |
| string/array/number/io/json builtins | ✅ (json returns real values via `any`) |
| Prelude registration | ✅ automatic — every `.quew` file in `prelude/` is embedded at compile time (`include_dir`), sorted by path; no Rust edits needed for new prelude files |
| Optional parameters | ✅ trailing `param?: T` may be omitted at call sites |
| `any` type | ✅ primitive in the type system; accepts/produces any value; used by JSON builtins |
| `fetch()` HTTP stdlib | ✅ `fetch(url, config?)` → `{ status, ok, body, error }`; method/headers/body/timeout via rustls TLS. Network fixture: `q_tests/net_fetch.quew` |
| JSON builtins | ✅ `json_parse: string → any`, `json_stringify: any → string`, `json_get: (any, path) → any` returning real values |

---

## 8. Runtime Execution

| Feature | Status |
|---------|--------|
| Graph walking (`Execution::run`) | ✅ linear insertion-order walk; correctness depends on lowerer emitting topological order |
| Branch routing + unreachable marking | ✅ (branch targets now back-patched from real lowered node ids) |
| Loops incl. capture write-back | ✅ verified by `q_tests/` |
| Loop iteration caps | ✅ `while` capped at 100k by default (`ExecutionLimits`, configurable) — `q_tests/infinite_loop_capped.quew` |
| Recursion depth cap | ✅ capped at 64 by default; clean `RecursionLimitExceeded` instead of stack overflow |
| Rich data-ref errors | ✅ `MissingOutput` names the missing node/field |
| Float division | ✅ div-by-zero errors for floats too (no silent inf/NaN) |
| Expression-level function calls | ✅ share the executor's depth tracking (was: fresh executor per call, unbounded) |
| Recursive calls (self/cyclic) | ✅ runtime re-keys args from callee signature — fixes lowerer's positional `arg0` fallback for self-recursive bodies |
| `HostToolCall` / `Reply` / `AgentCall` runtime | ❌ rejected with `UnsupportedNode` (LLM-related, deferred) |
| Checkpoint / resume | ❌ `CheckpointPolicy` exists in IR but is **ignored** by the executor; `Value` is not serializable |
| Middleware execution | ❌ |

### Known runtime quality issues
1. **Loop performance:** ~42µs/iteration (100k while iterations ≈ 4s release). Each iteration
   re-walks the body graph and clones outputs maps. Needs a fast path.
2. **Error-as-value anti-pattern** in json/net stdlib hides failures from the type system.
3. **Compiler bug (documented, worked around):** the lowerer names expression-call args via
   `definitions.functions`, which is not yet populated when a function's own body is lowered,
   so self-recursive calls get positional `arg0` names. The runtime re-keys positionally from
   the callee signature, masking it. Proper fix: two-pass definition registration in the
   lowerer.
4. Deeply recursive failures produce one wrapped error message per stack level — flatten.
5. `EvalError::MissingField` reports `NodeId(0)` placeholder for member-access misses.

---

## 9. Workspace Hygiene (post-audit)

- Removed stub crates: `quew-analysis`, `quew-resolve`, `quew-codegen` (doc-comment-only;
  re-add each when actually implemented). `quew-cli` no longer depends on quew-codegen.
- Entry-agent selection scans user items (reverse scan past prepended prelude items).
  Still approximate until an explicit `@entry` annotation lands.
- The prelude module is still merged for both check and lower (required: native bindings
  resolve through it), so prelude definitions are duplicated into every compiled IR — bloat,
  not incorrectness. Roadmap item.
- CLI now prints execution errors in human-readable Display form.

---

## Summary: Biggest Missing Pieces (in roadmap order)

See `V2-milestone/ROADMAP.md` for the full plan. Short version:

1. **Resume design first** — checkpoint format, mid-`Reply` recovery, graph serialization.
   This is the product's core promise and currently has zero implementation.
2. **Middleware parse → check → lower → execute** (lexer tokens already exist).
3. **Reply / HostToolCall / AgentCall executor nodes** + drivers (port v1 `auwgent-drivers`).
4. **Runtime hardening**: loop iteration caps, recursion depth limit, richer data-ref errors.
5. **Loop performance fast path.**
6. plan21 leftovers: `any` type, honest fetch/json error model.
