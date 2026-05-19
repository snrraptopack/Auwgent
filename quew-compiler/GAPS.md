# Quew Compiler Gap Analysis

> Comparing `not.txt` (target language design) against current compiler implementation.

## Legend
- ✅ Implemented
- ⚠️ Partial / Stub
- ❌ Not implemented

---

## 1. Top-Level Declarations

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| `agent` | ✅ | ✅ | ✅ | ⚠️ (graph walks, no LLM) |
| `function` | ✅ | ✅ | ✅ | ✅ |
| `tool` (host-backed) | ✅ | ✅ | ✅ | ❌ |
| `tools { ... }` group | ✅ | ✅ | ✅ | ❌ |
| `type` | ✅ | ✅ | ✅ | N/A |
| `model` | ✅ | ✅ | ✅ | N/A |
| `extend Type { ... }` | ✅ | ✅ | ✅ | ✅ |
| `@middleware` function | ❌ | ❌ | ❌ | ❌ |
| `@middlewares(...)` on agent | ❌ | ❌ | ❌ | ❌ |

**Gap:** Middleware is tokenized (`@middleware`, `@middlewares`) but never parsed into AST. The lexer knows them; the parser drops them.

---

## 2. Statements

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| `let name = expr` | ✅ | ✅ | ✅ | ✅ |
| `let name: Type = expr` | ✅ | ✅ | ✅ | ✅ |
| `if / else` | ✅ | ✅ | ✅ | ✅ |
| `return expr` | ✅ | ✅ | ✅ | ✅ |
| `return expr with turns` | ✅ | ✅ | ✅ | ❌ |
| `reply(expr) with { ... }` | ✅ | ✅ | ✅ | ❌ |
| `for idx, value in iterable` | ✅ | ⚠️ | ⚠️ | ❌ |
| `while` | ❌ | ❌ | ❌ | ❌ |
| `break` / `continue` | ❌ | ❌ | ❌ | ❌ |
| Expression stmt | ✅ | ✅ | ✅ | ✅ |

**Gaps:**
- `for` loops parse but the checker treats them as no-ops (empty type) and the lowerer emits nothing useful.
- No `while`, `break`, or `continue`.

---

## 3. Expressions

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| Literals (int, float, bool, null) | ✅ | ✅ | ✅ | ✅ |
| String `"..."` | ✅ | ✅ | ✅ | ✅ |
| String `"""..."""` | ✅ | ✅ | ✅ | ✅ |
| String interpolation `"hello {name}"` | ❌ | ❌ | ❌ | ❌ |
| Identifier | ✅ | ✅ | ✅ | ✅ |
| Binary ops (+, -, ==, and, or, =) | ✅ | ✅ | ✅ | ✅ |
| Unary `not` | ✅ | ✅ | ✅ | ✅ |
| Call `foo()` | ✅ | ✅ | ✅ | ✅ |
| Provider `gemini("...")` | ✅ | ✅ | ✅ | ✅ |
| Member `obj.field` | ✅ | ✅ | ✅ | ✅ |
| Array `[a, b]` | ✅ | ✅ | ✅ | ✅ |
| Postfix if `a if cond else b` | ✅ | ✅ | ✅ | ✅ |
| Type check `x is Type` | ✅ | ⚠️ | ❌ | ❌ |
| Ternary `cond then a else b` | ❌ | ❌ | ❌ | ❌ |

**Gaps:**
- String interpolation is stored raw in the literal; no parser support for extracting `{expr}` segments.
- `x is Type` parses and checks but is NOT lowered to IR (no runtime discrimination).
- `not.txt` uses `then` in expressions like `inputType.data.includes("high") then One(input) else Two(input)` — this is distinct from postfix-if and not parsed.

---

## 4. `reply() with { ... }` Config Block

| Field | Parser | Lowered | Notes |
|-------|--------|---------|-------|
| `prompt` | ✅ | ✅ | |
| `model` | ✅ | ✅ | Supports inline provider OR model alias |
| `fallback` | ✅ | ✅ | |
| `retry` | ✅ | ✅ | |
| `maxTurn` | ✅ | ✅ | |
| `tools` | ✅ | ✅ | Supports bare refs AND prebound calls `tool(ctx.arg)` |
| `builtin` | ✅ | ✅ | |
| `agents` | ✅ | ✅ | |
| `agents: { One, Two }` | ✅ | ✅ | Parsed as array of idents, lowered to `AgentRef` list |
| Dynamic model `model: Gemini if ctx.isVip else Groq` | ⚠️ | ⚠️ | Parses as arbitrary `Expr`, but lowerer panics on non-model exprs |

**Gaps:**
- Dynamic model selection in `with` block (conditional expr) is not validated or lowered correctly.

---

## 5. Annotations

| Annotation | Lexer | Parser | Checker | IR |
|------------|-------|--------|---------|-----|
| `@native` / `@block` | ✅ | ✅ | ✅ | ✅ |
| `@context(Type)` | ✅ | ✅ | ✅ | ✅ |
| `@desc "..."` | ✅ | ✅ | ✅ | ✅ |
| `@tool(...)` | ✅ | ✅ | ✅ | ✅ |
| `@middleware("name")` | ✅ | ❌ | ❌ | ❌ |
| `@middlewares(Name1, Name2)` | ✅ | ❌ | ❌ | ❌ |

**Gap:** Middleware annotations are dead tokens — recognized by lexer, ignored everywhere else.

---

## 6. Type System

| Feature | Status |
|---------|--------|
| Primitives (string, number, float, bool, null, void) | ✅ |
| Named types / records | ✅ |
| Optional `?` | ✅ |
| Union `A \| B` | ✅ |
| Generics on types | ✅ |
| Generics on functions | ✅ |
| Agent return type | ✅ |
| Agent input type | ✅ |
| Context type | ✅ |

---

## 7. Native / Standard Library

| Feature | Status |
|---------|--------|
| `@@rust("id")` builtin marker | ✅ |
| `NativeRegistry` mechanism | ✅ |
| Hardcoded stdlib in runtime | ❌ (removed per `one.txt`) |
| `#[quew_builtin]` proc-macro | ❌ |
| `inventory` link-time registration | ❌ |
| `fetch()` / HTTP stdlib | ❌ |

---

## 8. Runtime Execution

| Feature | Status |
|---------|--------|
| Graph walking (`Execution::run`) | ✅ |
| `Input`, `Context`, `Output`, `LetBind`, `Branch` | ✅ |
| `FuncCall` (user functions, extensions) | ✅ |
| `IrExpr::Call` → native dispatch | ✅ |
| `IrExpr::Call` → graph recursion | ✅ |
| `HostToolCall` | ❌ |
| `Reply` (LLM turn) | ❌ |
| `AgentCall` (sub-agent) | ❌ |
| Checkpoint / resume | ❌ |
| Middleware execution | ❌ |

---

## 9. String & Literal Features

| Feature | Status |
|---------|--------|
| Regular strings `"..."` | ✅ |
| Triple strings `"""..."""` | ✅ |
| Escape sequences | ✅ |
| String interpolation | ❌ |
| Template literals / multiline formatting | ❌ |

---

## Summary: Biggest Missing Pieces

1. **Middleware** — Tokenized but never parsed, checked, or lowered. This is a major feature in `not.txt`.
2. **String interpolation** — Critical for prompt building (`"hello {name}"`).
3. **`while`, `break`, `continue`** — Basic control flow missing.
4. **`x is Type` runtime discrimination** — Parsed but not lowered or executed.
5. **`for` loop** — Parsed but checker treats as no-op; lowerer emits no IR.
6. **Dynamic model in `with` block** — Not properly validated/lowered.
7. **HostToolCall / Reply / AgentCall runtime** — IR nodes exist but executor can't run them.
8. **`#[quew_builtin]` proc-macro** — Future stdlib registration mechanism.
9. **String interpolation lowering** — Even if parsed, would need IR support and runtime evaluation.
