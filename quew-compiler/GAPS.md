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
| `for idx, value in iterable` | ✅ | ✅ | ✅ | ✅ |
| `while` | ✅ | ✅ | ✅ | ✅ |
| `break` / `continue` | ❌ | ❌ | ❌ | ❌ |
| Expression stmt | ✅ | ✅ | ✅ | ✅ |

**Gaps:**
- No `break` or `continue`.

---

## 3. Expressions

| Feature | Parser | Checker | IR Lower | Runtime |
|---------|--------|---------|----------|---------|
| Literals (int, float, bool, null) | ✅ | ✅ | ✅ | ✅ |
| String `"..."` | ✅ | ✅ | ✅ | ✅ |
| String `"""..."""` | ✅ | ✅ | ✅ | ✅ |
| String interpolation `"hello {name}"` | ✅ | ✅ | ✅ | ✅ |
| Identifier | ✅ | ✅ | ✅ | ✅ |
| Binary ops (+, -, *, /, %, ==, !=, <, <=, >, >=, and, or, =) | ✅ | ✅ | ✅ | ✅ |
| Unary `not` | ✅ | ✅ | ✅ | ✅ |
| Call `foo()` | ✅ | ✅ | ✅ | ✅ |
| Provider `gemini("...")` | ✅ | ✅ | ✅ | ✅ |
| Member `obj.field` | ✅ | ✅ | ✅ | ✅ |
| Array `[a, b]` | ✅ | ✅ | ✅ | ✅ |
| Object literal `{ k: v }` | ✅ | ✅ | ✅ | ✅ |
| Postfix if `a if cond else b` | ✅ | ✅ | ✅ | ✅ |
| Type check `x is Type` | ✅ | ⚠️ | ✅ | ✅ |

**Gaps:**
- `x is Type` is fully functional for primitives and records (best-effort for records — checks `Value::Object` only).

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
| `#[quew_builtin]` proc-macro | ✅ |
| `inventory` link-time registration | ✅ |
| `print<T>(value: T): null` | ✅ |
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
| `HostToolCall` | ❌ (LLM-related, deferred) |
| `Reply` (LLM turn) | ❌ (LLM-related, deferred) |
| `AgentCall` (sub-agent) | ❌ (LLM-related, deferred) |
| Checkpoint / resume | ❌ |
| Middleware execution | ❌ |

---

## 9. String & Literal Features

| Feature | Status |
|---------|--------|
| Regular strings `"..."` | ✅ |
| Triple strings `"""..."""` | ✅ |
| Escape sequences | ✅ |
| String interpolation | ✅ |
| Template literals / multiline formatting | ❌ (deferred) |

---

## Summary: Biggest Missing Pieces

1. **Middleware** — Tokenized but never parsed, checked, or lowered. This is a major feature in `not.txt`.
2. **`break` / `continue`** — Loop control flow missing.
3. **Dynamic model in `with` block** — Not properly validated/lowered.
4. **HostToolCall / Reply / AgentCall runtime** — IR nodes exist but executor can't run them (LLM-related, deferred).
5. **`fetch()` builtin** — Not started; needed for HTTP drivers in quew.
6. **JSON builtins** (`json_parse`, `json_stringify`, `json_get`) — Not started; needed for response/request handling.
