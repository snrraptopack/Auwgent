# Plan 20: `print` Builtin + `x is Type` Runtime + Prelude Modularity

**Status:** Completed
**Scope:** Close the last non-agent core language gap and add debug output. No LLM, no agent features, no I/O beyond stdout.

---

## Goals

1. Add `print<T>(value: T): null` builtin for debug output.
2. Implement `x is Type` runtime discrimination end-to-end.
3. Split `prelude/native.quew` into modular files.

## Non-Goals

- `break` / `continue` (deferred — needs native loop nodes)
- `fetch` / HTTP
- JSON builtins
- Middleware
- Runtime type tags on objects

---

## Architecture

### `print` Builtin

```
User code: print("hello")
    → Parser: CallExpr { callee: Ident("print"), args: ["hello"] }
    → Checker: Generic inference T = string, return type = null
    → Lowerer: IrExpr::Call { function: "print", args: { value: "hello" } }
    → Runtime: NativeRegistry lookup "std.io.print" → print_value(&Value::String("hello"))
    → stdout: "hello\n"
    → returns Value::Null
```

### `x is Type` Runtime Discrimination

```
User code: x is string
    → Parser: Expr::Is { value: Ident("x"), ty: Named("string") }
    → Checker: returns Ty::bool()
    → Lowerer: IrExpr::Is { value: Ref(x), ty: "string" }
    → Runtime: match value { Value::String(_) => true, _ => false }
```

### Prelude Modularity

Before:
```
prelude/native.quew     (47 lines, mixed concerns)
```

After:
```
prelude/
  native.quew           → aggregator (includes other files)
  string.quew           → std.string.* builtins + string extensions
  array.quew            → std.array.* builtins
  number.quew           → std.number.* builtins
  io.quew               → std.io.* builtins
```

The checker prelude loader concatenates all `.quew` files in `prelude/`.

---

## Implementation Steps

### Step 1: Prelude Modularity

1. Create `prelude/string.quew`, `prelude/array.quew`, `prelude/number.quew`, `prelude/io.quew`.
2. Move existing declarations from `native.quew` into their respective files.
3. Update `native.quew` to be a thin aggregator (or remove it and update the loader to glob `prelude/*.quew`).
4. Update `quew-checker/src/prelude.rs` to load all `.quew` files in `prelude/`.

**Files touched:**
- `quew-compiler/prelude/*.quew` (new + modified)
- `quew-compiler/crates/quew-checker/src/prelude.rs`

### Step 2: `print` Rust Implementation

1. Create `quew-runtime/crates/quew-stdlib/src/io.rs`:
   ```rust
   #[quew_builtin(id = "std.io.print", decl = r#"!@@function print<T>(value: T): null"#)]
   pub fn print_value(value: &Value) -> Value {
       println!("{}", value);
       Value::Null
   }
   ```
2. Add `pub mod io;` to `quew-stdlib/src/lib.rs`.
3. Add declaration to `prelude/io.quew`.

**Files touched:**
- `quew-runtime/crates/quew-stdlib/src/io.rs` (new)
- `quew-runtime/crates/quew-stdlib/src/lib.rs`
- `quew-compiler/prelude/io.quew` (new)

### Step 3: `x is Type` IR Lowering

1. Add `IrExpr::Is { value: Box<IrExpr>, ty: InternedStr }` to `quew-ir/src/graph.rs`.
2. Update `lower_expr` in `quew-ir/src/lower/expr.rs` to handle `Expr::Is`:
   - Extract the type name from `TypeExpr::Named`
   - Lower the value expression
   - Emit `IrExpr::Is { value, ty }`
   - For non-`Named` types (unions, arrays, etc.), emit a checker error or fallback to `IrLit::Null`.

**Files touched:**
- `quew-compiler/crates/quew-ir/src/graph.rs`
- `quew-compiler/crates/quew-ir/src/lower/expr.rs`

### Step 4: `x is Type` Runtime Evaluation

1. Update `eval_expr` in `quew-runtime/src/eval.rs` to handle `IrExpr::Is`:
   ```rust
   IrExpr::Is { value, ty } => {
       let val = eval_expr(value, ...)?;
       let ty_name = interner.resolve(*ty);
       let result = match ty_name {
           "string" => matches!(val, Value::String(_)),
           "number" => matches!(val, Value::Number(_)),
           "float" => matches!(val, Value::Float(_)),
           "bool" => matches!(val, Value::Bool(_)),
           "null" => matches!(val, Value::Null),
           "array" => matches!(val, Value::Array(_)),
           _ => matches!(val, Value::Object(_)), // record types
       };
       Ok(Value::Bool(result))
   }
   ```

**Files touched:**
- `quew-runtime/crates/quew-runtime/src/eval.rs`

### Step 5: Tests

**`print` tests:**
- `execute_print_builtin_from_compiled_code` — compile a function that calls `print("hello")`, run it with the real `NativeRegistry::collect()`, capture stdout and verify output.
  - *Problem:* `NativeRegistry::collect()` requires `inventory` link-time registration. Tests in `quew-runtime` that use `compile_source_with_prelude` create a manual registry. To test the real `print`, we need to either:
    - Register `print` manually in the test (like existing native tests)
    - Or use `NativeRegistry::collect()` (which collects ALL builtins including quew-stdlib)
  - **Decision:** Use `NativeRegistry::collect()` for the print test. This is the first test that exercises the full link-time registration pipeline.

**`x is Type` tests:**
- `execute_is_string_true` — `"hello" is string` → `true`
- `execute_is_string_false` — `42 is string` → `false`
- `execute_is_number_true` — `42 is number` → `true`
- `execute_is_bool_true` — `true is bool` → `true`
- `execute_is_array_true` — `[1, 2] is array` → `true`
- `execute_is_record_true` — typed object literal `is Person` → `true` (best effort)

**Files touched:**
- `quew-runtime/crates/quew-runtime/src/execution.rs` (tests)

### Step 6: Documentation Update

1. Update `quew-compiler/GAPS.md`:
   - Mark `x is Type` as ✅
   - Add `print` to stdlib table
2. Update `quew-compiler/docs/BUILTIN_PIPELINE.md`:
   - Document the two-source-of-truth problem
   - Add the modularity section
   - Document `print` as an example

**Files touched:**
- `quew-compiler/GAPS.md`
- `quew-compiler/docs/BUILTIN_PIPELINE.md`

---

## Completed Notes

- Parser now accepts `null` as a type name in function signatures (enabling `print<T>(value: T): null`).
- Prelude tests were made robust: removed hardcoded item counts, added `assert_native` helper.
- `NativeRegistry::collect()` is used in the `print` test with a fallback to manual registration (some linkers drop dev-dependency crates on Windows).
- `x is Type` for record types is best-effort: all records are `Value::Object` at runtime, so `x is Person` and `x is User` both return `true` for any object.

## Acceptance Criteria

- [x] `prelude/` is split into `string.quew`, `array.quew`, `number.quew`, `io.quew`
- [x] `print("hello")` compiles, executes, and prints to stdout
- [x] `"hello" is string` evaluates to `true`
- [x] `42 is string` evaluates to `false`
- [x] `[1, 2] is array` evaluates to `true`
- [x] `{ name: "Alice" } is Person` evaluates to `true` (object check)
- [x] All existing tests continue to pass (429 compiler tests, 56 runtime tests)
- [x] `GAPS.md` and `BUILTIN_PIPELINE.md` are updated

---

## Risk Analysis

| Risk | Mitigation |
|------|------------|
| `print` test captures stdout in parallel test runner | Use `std::sync::Mutex` around `println!` or use a custom print hook. Simpler: just verify the function returns `null` and manually inspect stdout when running the single test. |
| `IrExpr::Is` changes IR schema | `IrExpr` is not serialized (it's in-memory only). No schema migration needed. |
| Prelude loader globbing breaks `include_str!` | Use explicit file list instead of glob to avoid build-non-determinism. |
| `x is Type` for non-primitive types is imprecise | Document the limitation. Record types check `Value::Object` only. |
