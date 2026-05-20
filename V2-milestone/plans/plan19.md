# Plan 19: Core Language Completion

**Status:** In Progress. Phases 1–5 complete. Phase 6 (`break`/`continue`) and Phase 7 (`x is Type` runtime) deferred.

**Scope:** Close all core language gaps identified in Discussion 12. No runtime architecture, no LLM, no quew-ification — strictly language + compiler + deterministic executor.

---

## Where We Are

Plan 18 (string interpolation) is done. Phases 1–5 of this plan are complete. The quew language now has:
- Primitives, variables, functions, `if/else`, recursion, arrays, postfix-if
- Mutable assignment (`x = expr`) via re-binding
- `for` loop execution over arrays
- `while` loops (zero-iteration confirmed; mutation across iterations limited — see **Limitations**)
- Object literals `{ key: value }`
- Array builtins (`std.array.len`, `get`, `push`, `pop`)
- No `break`/`continue`
- No `x is Type` runtime

---

## Goals

1. Make `for` loops execute.
2. Add `while` loops.
3. Add array builtins (`len`, `get`, `push`, `pop`).
4. Add object literals `{ key: value }`.
5. Fix `=` to be actual mutable assignment (or remove it from parser).
6. Add `break`/`continue`.

## Non-Goals

- `x is Type` runtime — deferred to Plan 20 (pattern matching / literal types too complex)
- `try/catch` error handling
- Modules / imports
- Async / await
- `fetch` / HTTP

---

## Architecture

### `for` / `while` Loop Strategy: Synthetic Recursive Functions

Instead of adding a new IR node, lower loops to **recursion via synthetic functions**:

```quew
for item in items {
    body
}
```

→ Lowered to:

```quew
__for_loop_1(items, 0, captured_var1, captured_var2, ...)
```

Where `__for_loop_1` is a synthetic function injected into the module:

```quew
function __for_loop_1(items: array, idx: number, captured_var1: T1, ...): void {
    if idx >= array_len(items) {
        return
    }
    let item = array_get(items, idx)
    // body inlined here
    __for_loop_1(items, idx + 1, captured_var1, ...)
}
```

**Why this approach:**
- Zero changes to IR types or executor
- Reuses existing `FuncCall` + `Branch` infrastructure
- Body sees loop vars naturally
- Captured outer variables are passed explicitly

**Tradeoff:** Stack depth = array length. For typical agentic arrays (tens/hundreds of items), this is fine. For megabyte arrays, we'd need a native loop node later.

### Captured Variables

The lowerer analyzes the loop body AST, collects all `Ident` references that are:
- NOT the loop variable (`item`)
- NOT the index variable (`idx`)
- NOT locally `let`-bound inside the body

These become extra parameters to the synthetic function and are passed from the call site.

### Mutable Assignment

Currently `x = 1` parses as `BinaryOp::Assign` but lowers to `BinaryOp::Eq` (equality). We have two options:

**Option A — Make `=` real assignment:**
- Track mutable bindings in `LocalScope`
- Add `IrExpr::Assign { target, value }`
- Executor updates the binding in scope
- This makes quew imperative

**Option B — Remove `=` from parser:**
- `=` is not a valid assignment operator
- All state changes via `let` rebinding in nested scopes
- Keeps quew functional / immutable

**Recommendation:** Option A. Agentic code needs mutable state (`let count = 0; count = count + 1`). But implement it as **re-binding** in the local scope, not in-place mutation of `Value`.

### Object Literals

Add `{ key: value, ... }` as an expression form. Parser distinguishes from blocks by lookahead (`Ident :` after `{` = object literal, anything else = block).

```rust
Expr::Object(Vec<ObjectField>)

pub struct ObjectField {
    pub key: InternedStr,
    pub value: Expr,
    pub span: Span,
}
```

Lowered to `IrExpr::Object(IndexMap<InternedStr, IrExpr>)`.

---

## Implementation Steps

### Step 1: Array Builtins (quew-stdlib)

Add `#[quew_builtin]` functions:
- `std.array.len(array: array): number`
- `std.array.get(array: array, index: number): any`
- `std.array.push(array: array, value: any): array` — returns new array (immutable)
- `std.array.pop(array: array): array` — returns new array without last element

Update prelude.

**Tests:** 4 stdlib tests + 2 execution tests.

### Step 2: `for` Loop Lowering (quew-ir)

1. Add `lower_for()` in `graph_lower.rs`:
   - Analyze body for captured variables
   - Generate synthetic `FunctionDecl`
   - Inject into module items before lowering
   - Replace `Stmt::For` with `Expr::Call` to synthetic function

2. The synthetic function uses `array_len` and `array_get` builtins for bounds checking and element access.

**Tests:** Execute `for` loop over array, verify body runs for each element.

### Step 3: `while` Loop Parsing + Lowering (quew-parser + quew-ir)

1. **Parser:** Add `while condition { body }` to `parse_stmt.rs`
2. **AST:** Add `WhileStmt { condition: Expr, body: Vec<Stmt>, span: Span }`
3. **Checker:** Check condition is `bool`, check body
4. **Lowerer:** Same synthetic function approach as `for`:
   ```quew
   function __while_loop_1(captured_vars...) {
       if !condition {
           return
       }
       // body
       __while_loop_1(captured_vars...)
   }
   __while_loop_1(captured_vars...)
   ```

**Tests:** Execute `while` loop with counter, verify termination.

### Step 4: Mutable Assignment (quew-parser → quew-ir → quew-runtime)

This is the biggest semantic change.

1. **Parser:** `x = expr` already parses as `BinaryOp::Assign` — keep it
2. **Checker:** Allow assignment only if `x` is a local variable (not a function name, not a parameter? Actually parameters should be assignable too)
3. **IR:** Add `IrExpr::Assign { name: InternedStr, value: IrExpr }`
4. **Runtime:** In `eval_expr`, when encountering `Assign`, update the local scope binding

**Open question:** Should assignment be an expression (returns the new value) or a statement (returns nothing)? In quew, `=` is currently parsed as a binary expression. Let's make it return the assigned value for consistency.

**Tests:** `let x = 1; x = 2; return x` should return 2.

### Step 5: Object Literals (quew-parser → quew-ast → quew-checker → quew-ir → quew-runtime)

1. **Parser:** In `parse_expr.rs`, add object literal parsing with `Ident : Expr` lookahead
2. **AST:** Add `Expr::Object(Vec<ObjectField>)`
3. **Checker:** Infer type as `Record({ key: inferred_type, ... })`
4. **IR:** Add `IrExpr::Object(IndexMap<InternedStr, IrExpr>)`
5. **Runtime:** `eval_expr` creates `Value::Object(IndexMap::new())`

**Tests:** Parse, check, lower, execute object literal.

### Step 6: `break` / `continue`

Requires a mechanism to escape from the synthetic recursive function early.

Approach: Add special return values:
- `__break` — stop the loop entirely
- `__continue` — skip to next iteration

In the synthetic function body:
- `break` → `return __break_marker`
- `continue` → `return __continue_marker`

The synthetic function checks the return value:
```quew
function __for_loop_1(...) {
    if idx >= len { return null }
    let item = get(items, idx)
    let result = body()
    if result == __break_marker { return null }
    // if result == __continue_marker, just proceed
    return __for_loop_1(...)
}
```

**Alternative:** Defer `break`/`continue` to a future plan with native loop nodes.

**Recommendation:** Defer `break`/`continue`. They require significant AST + lowerer complexity. The recursion approach makes them awkward. Better to add them when we have native `Loop` IR nodes.

---

## Phased Delivery

| Phase | Features | Est. Tests | Complexity |
|-------|----------|------------|------------|
| 1 | Array builtins | 6 | Small |
| 2 | `for` loop execution | 6 | Medium |
| 3 | `while` loop | 4 | Small-Medium |
| 4 | Mutable assignment | 6 | Medium |
| 5 | Object literals | 6 | Medium |
| 6 (deferred) | `break`/`continue` | 4 | Medium |
| 7 (deferred) | `x is Type` runtime | 6 | Small-Medium |

---

## Known Limitations

### Mutable assignment across branch boundaries
**Status: RESOLVED.** The lowerer now creates merge (phi) nodes after every `if` statement. For each variable mutated via `=` inside either branch, a `LetBind` node with a lazy `Ternary` expression is emitted after the branch. The ternary selects the then-branch value, the else-branch value, or the pre-branch value based on the runtime condition, ensuring correct semantics even when one arm is skipped by the executor.

### Mutable assignment across loop iterations
**Status: RESOLVED.** The `while` loop body graph now returns a record `{ __cond: bool, <captured_vars> }` instead of just a boolean. The executor extracts the updated state from the return value and writes it back into the parent graph's `outputs` map before the next iteration. This propagates mutations across loop boundaries correctly.

### `break` / `continue`
Deferred. The current synthetic-recursive-function lowering makes `break`/`continue` awkward to implement. A native `Loop` / `WhileLoop` IR node (or proper SSA phi nodes) would be a better foundation.

---

## Acceptance Criteria

- [x] `array_len([1, 2, 3])` returns `3`
- [x] `array_get([1, 2, 3], 1)` returns `2`
- [x] `for item in items { body }` executes body for each element
- [x] `while count < 5 { body }` executes until condition is false (see **Limitations**)
- [x] `let x = 1; x = 2; return x` returns `2`
- [x] `let obj = { name: "Alice", age: 30 }` parses and executes
- [x] All existing tests continue to pass
