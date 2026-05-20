# Discussion 12: Core Language Audit — What's Implemented vs Missing

**Status:** In progress — checking off quew language features against this checklist.

---

## Core Language

| Feature | Status | Notes |
|---------|--------|-------|
| Primitive types (int, float, string, bool, null) | ✅ | `number`, `float`, `string`, `bool`, `null` all parse, check, lower, execute |
| Variables (`let` binding) | ✅ | `let x = expr` and `let x: Type = expr` both work |
| Control flow: `if/else` | ✅ | Fully functional including chained else-if |
| Control flow: `for` loops | ⚠️ | **Parsed into AST** but checker gives `Ty::Error` to loop vars, lowerer emits nothing, runtime can't execute |
| Control flow: `while` | ❌ | Not parsed at all |
| Control flow: `break` / `continue` | ❌ | Not parsed at all |
| Functions | ✅ | Parse, check, lower, execute. Includes recursion, params, return types |
| Arrays (`[a, b, c]`) | ⚠️ | **Parsed, checked, lowered, partially executed**. `Value::Array` exists. But no `array_len`, `array_get`, `array_push` builtins yet |
| Objects / Records (`{ name: "Alice" }`) | ⚠️ | **Record types** (`type User = { name: string }`) fully supported. **Object literals** not parsed — no `{ key: value }` expression syntax |
| Error handling (try/catch, Result) | ❌ | No error handling constructs |
| Modules / imports | ❌ | Single file only. No `import`, `export`, or module system |
| String interpolation | ✅ | `"hello {name}"` — Plan 18, fully functional |
| Type annotations | ✅ | Parameters, return types, let bindings all support type annotations |
| Generics | ✅ | `function identity<T>(x: T): T` and `type Box<T> = { value: T }` both work |
| Extension methods | ✅ | `extend string { function len(): number { ... } }` works |
| Unary `not` | ✅ | Parsed, checked, executed |
| Binary ops (`+`, `-`, `*`, `/`, `%`, `==`, `!=`, `and`, `or`, `=`) | ⚠️ | `+ - * / % == != and or` all work. **`=` is lowered to `==`** (equality), not assignment — IR has no `Assign` binary op |
| Postfix-if `a if cond else b` | ✅ | Fully functional |
| Type check `x is Type` | ⚠️ | **Parsed, checked (blindly returns bool)**, but NOT lowered to IR or executed at runtime |
| Member access `obj.field` | ✅ | Fully functional |
| Function calls `foo()` | ✅ | Fully functional including native builtins |
| Array literal `[a, b]` | ✅ | Parsed, checked, lowered, executed |

---

## Network / I/O Features (Deferred — not core language)

| Feature | Status | Notes |
|---------|--------|-------|
| Async/await | ❌ | Not in language |
| HTTP client (`fetch`) | ❌ | Not in stdlib |
| JSON parse/stringify | ❌ | Not in stdlib |
| String interpolation | ✅ | Done in Plan 18 |

---

## Strictly Core Language — What's Missing (in priority order)

### 1. `for` loop execution ⚠️
- **Parser**: ✅ Already parses `for idx, value in iterable { body }`
- **Checker**: ⚠️ Loops vars get `Ty::Error` instead of element type
- **IR lowerer**: ❌ Returns `None` — emits no nodes
- **Runtime**: ❌ Executor has no loop support
- **Blocker**: Need to decide how to represent loops in IR (new `Loop` node vs lowering to recursion)

### 2. Object/record literals ⚠️
- **Parser**: ❌ No `{ key: value }` expression syntax
- **Checker**: N/A
- **IR lowerer**: N/A
- **Runtime**: `Value::Object` exists but can only be created by native functions
- **Blocker**: Parser grammar needs new expression form

### 3. `while` / `break` / `continue` ❌
- **Parser**: ❌ Not parsed
- **Checker**: N/A
- **IR lowerer**: N/A
- **Runtime**: N/A
- **Blocker**: Needs parser + AST + checker + IR + runtime work

### 4. `x is Type` runtime execution ⚠️
- **Parser**: ✅ Already parses `x is Type`
- **Checker**: ⚠️ Returns `bool` without validating the type
- **IR lowerer**: ❌ Maps to `IrLit::Null`
- **Runtime**: ❌ No `Is` evaluation
- **User decision**: Deferred — too complex for pattern matching/literal types right now

### 5. Array builtins ❌
- `array_len`, `array_get`, `array_push`, `array_pop`, etc.
- Needed for `for` loops and general array manipulation

### 6. Error handling ❌
- No `try/catch`, `Result<T, E>`, `panic`, or `?` operator

### 7. Modules / imports ❌
- Single-file compilation only

---

## Recommendation

The highest-impact next core language feature is **`for` loop execution** because:
1. It's already parsed — zero parser work
2. It's needed for array iteration, which is fundamental
3. It unlocks writing more interesting quew programs

The second priority is **object literals** (`{ name: "Alice" }`) because:
1. Record types exist but can't be constructed in user code
2. Needed for JSON-like data manipulation
3. Relatively self-contained parser addition
