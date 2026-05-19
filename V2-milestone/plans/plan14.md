# Plan 14: Compiler Cleanup — Fix `lower_expr` Args and `CheckResult` Default

**Status:** Completed. Derived from Discussion 9, Section 10.1 and 10.2.

**Scope:** Compiler-only. No runtime work. Unblocks Phase 1 of the Graph IR Runtime Executor.

---

## Where We Are

Plan 13 completed the compiler pipeline:
- Checker resolves extension methods and populates `ResolvedExpressionMap`
- IR lowerer consumes the sidecar and emits `FuncCall` nodes for extension methods
- Function and extension method bodies are lowered into `AgentGraph`
- All 414+ tests pass

**Two cleanup issues remain** that block deterministic execution of compiled graphs:

1. **`lower_expr` silently drops arguments** for regular function calls (e.g. `max(a, b)`)
2. **`pub use quew_scope::SymbolTable`** was added as a public re-export solely to let tests construct dummy `CheckResult`s

---

## Issue 1: `lower_expr` Drops Call Arguments

### The Bug

In `quew-ir/src/lower/expr.rs`, when `lower_expr` encounters a plain function call (`Expr::Call` with `Expr::Ident` callee), it produces:

```rust
let function = match call.callee.as_ref() {
    Expr::Ident(ident) => ident.name,
    other => panic!(...),
};
let _ = call;
IrExpr::Call {
    function,
    args: Default::default(), // ← ALL ARGUMENTS DISCARDED
}
```

This is a pre-existing bug that affects **every regular function call inside an expression**.

### Impact

At runtime, a call like `custom_string_is_empty(self)` inside an extension method body evaluates to `IrExpr::Call { function: "custom_string_is_empty", args: {} }`. The expression evaluator will pass zero arguments to the function, causing a runtime arity mismatch or incorrect behavior.

Extension method calls (which we fixed in Plan 13) correctly populate `args`. Plain function calls do not.

### Fix

Populate `args` by looking up parameter names from `definitions.functions`:

```rust
let mut args = IndexMap::new();
if let Some(func) = definitions.functions.get(&function) {
    for (idx, (param_name, _)) in func.params.iter().enumerate() {
        if let Some(arg) = call.args.get(idx) {
            args.insert(*param_name, lower_expr(arg, check, definitions, interner, ctx));
        }
    }
} else {
    // Fallback for tool calls or unresolved calls: positional arg0, arg1, ...
    for (idx, arg) in call.args.iter().enumerate() {
        let name = interner.intern(&format!("arg{idx}"));
        args.insert(name, lower_expr(arg, check, definitions, interner, ctx));
    }
}
IrExpr::Call { function, args }
```

This mirrors the argument handling already implemented for extension method calls in the same function.

---

## Issue 2: `CheckResult` Needs a Clean Default

### The Problem

To make `defs.rs` tests compile after Plan 13 added `resolved: ResolvedExpressionMap` to `CheckResult`, a public re-export was added:

```rust
// quew-checker/src/lib.rs
pub use quew_scope::SymbolTable;
```

This leaks an internal scope type into the public `quew-checker` API solely for test convenience.

### Fix

Implement `Default` for `CheckResult` inside `quew-checker`:

```rust
// quew-checker/src/lib.rs
impl Default for CheckResult {
    fn default() -> Self {
        Self {
            symbol_table: SymbolTable::default(),
            diagnostics: Vec::new(),
            resolved: ResolvedExpressionMap::default(),
        }
    }
}
```

Then:
1. Remove `pub use quew_scope::SymbolTable;` from `quew-checker/src/lib.rs`
2. Update all test sites in `quew-ir/src/lower/defs.rs` to use `CheckResult::default()` instead of manually constructing the struct

---

## Implementation Steps

### Step 1: Implement `Default` for `CheckResult`
- Add `impl Default for CheckResult` in `quew-checker/src/lib.rs`
- Run `cargo check` to verify it compiles

### Step 2: Remove `pub use quew_scope::SymbolTable`
- Delete the re-export line
- Update `defs.rs` tests to use `CheckResult::default()`
- Run `cargo test` to verify all tests pass

### Step 3: Fix `lower_expr` argument mapping
- In `quew-ir/src/lower/expr.rs`, replace the `let _ = call; args: Default::default()` block with parameter-aware argument lowering
- Use `definitions.functions.get(&function)` to look up param names
- Fall back to `arg0`, `arg1`, ... for calls where the function is not in `definitions.functions`

### Step 4: Add regression test
- Add a test in `quew-ir/src/lower.rs` that compiles a `.quew` source with a regular function call that has explicit arguments, then inspects the IR to assert the `args` map is non-empty

---

## Test Plan

1. **Existing tests:** `cargo test` must pass with 0 failures (414+ tests)
2. **New regression test:** A function call with arguments produces an `IrExpr::Call` with populated `args`
3. **Manual verification:** `cargo run --bin quew -- compile test.quew` still compiles successfully

---

## Acceptance Criteria

- [X] `CheckResult::default()` exists and works
- [X] `pub use quew_scope::SymbolTable;` is removed
- [X] `lower_expr` maps call arguments by parameter name for known functions
- [X] Regression test exists for regular function call argument lowering
- [X] All existing tests pass
- [X] No compiler warnings introduced

---

## Non-Goals

- Runtime execution (that's Plan 15+)
- WASM compatibility fixes (that's a runtime concern, deferred)
- Changing how extension method calls lower (already working)
- JSON serialization of IR
