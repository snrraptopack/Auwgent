# Plan 6: Robust Generic Type System

**Status: Complete.**

Plan 6 is a compiler-foundation plan. Its only job is to give Quew a robust,
general-purpose generic type system that future language features can build on.

This plan must not special-case `ToolResult`, providers, stdlib methods, or
extension syntax. Those are later consumers of the generic system.

---

## Goal

Quew should support reusable typed abstractions without hardcoding individual
standard-library shapes into the checker.

Examples this plan should make possible:

```quew
type Box<T> = {
    value: T
}

type Pair<A, B> = {
    first: A,
    second: B
}

function identity<T>(value: T): T {
    return value
}

function pickFirst<A, B>(pair: Pair<A, B>): A {
    return pair.first
}
```

The compiler must understand:

- generic type parameters
- generic function parameters
- generic type instantiation
- generic substitution
- arity validation
- field access after substitution
- direct function return types after generic substitution

---

## Why This Plan Exists

Future Quew features depend on generics:

- result wrappers such as `ToolResult<T>`
- reusable collection types such as `List<T>` or `Map<K, V>`
- generic helper functions
- future extension methods over generic receivers, such as `extend Result<T, E>`
- future provider/model helper types

If generics are weak or ad hoc, every later feature will need special compiler
logic. Plan 6 prevents that by implementing generics as a real type-system
feature first.

---

## Core Decisions

### Generics Are General, Not Stdlib-Specific

The checker should not know that `ToolResult<T>` is special.

This is allowed as an ordinary user or future stdlib type:

```quew
type ToolResult<T> = {
    data: T,
    error: string
}
```

But Plan 6 should test the same behavior using neutral fixtures such as
`Box<T>`, `Pair<A, B>`, and `Result<T, E>`.

### Generic Identity Must Be Preserved

The compiler should preserve enough identity to distinguish:

```quew
Box<string>
Box<number>
Pair<string, bool>
```

Even if a generic record is structurally substituted during checking, IR and
diagnostics should still be able to describe the instantiated type clearly.

### Generic Functions Are Part Of Plan 6

Generic types alone are not enough. Plan 6 should support generic functions:

```quew
function identity<T>(value: T): T {
    return value
}
```

The checker should instantiate function type parameters at call sites.

Initial inference can be conservative:

- infer generic parameters from positional arguments where direct matching is
  obvious
- emit a clear diagnostic when inference is ambiguous
- optionally support explicit type arguments later if the parser design is
  settled

### No Extensions In Plan 6

Do not implement:

- `extend`
- method receivers
- `self`
- `#rust(...)`
- stdlib/prelude loading
- provider/model extensions
- request/IO primitives

Plan 6 should only ensure those features can be built later without replacing
the generic type system.

---

## Implementation Scope

### 1. Type Algebra

`quew-types` should own generic type machinery:

- generic parameters
- generic instances
- function type parameters
- substitution helpers
- generic parameter collection
- arity-safe instantiation helpers
- readable generic type formatting

Keep type algebra out of the checker where possible.

Suggested module split:

```text
quew-types/src/
  lib.rs
  generic.rs
  subst.rs
  display.rs
```

Only split files where it reduces real complexity. Do not create empty modules.

### 2. AST And Parser

Support:

```quew
type Box<T> = { value: T }
type Pair<A, B> = { first: A, second: B }
function identity<T>(value: T): T { return value }
let x: Box<string>
```

Parser tests should cover:

- one type parameter
- multiple type parameters
- generic type usage
- nested generic type usage, if the grammar supports it
- generic function declaration
- invalid generic arity syntax recovery where practical

### 3. Scope

`quew-scope` should record:

- type declaration type parameters
- function declaration type parameters
- which type parameters are in scope while lowering declaration signatures

It should not resolve generic calls or perform substitution.

### 4. Checker

`quew-checker` should be split enough that generic logic does not become a
large block inside `lib.rs`.

Suggested module split:

```text
quew-checker/src/
  lib.rs
  type_resolve.rs
  generics.rs
  infer.rs
  calls.rs
  fields.rs
  diagnostics.rs
```

The checker should implement:

- generic arity validation
- unknown generic parameter diagnostics
- generic type substitution
- field access on instantiated generic records
- generic function instantiation
- return type validation after substitution
- clear diagnostics for failed inference

### 5. IR

`quew-ir` should preserve generic definitions and instantiated types clearly.

It should not flatten away all generic identity just because checking can
substitute fields structurally.

IR tests should assert on Rust structs directly, not snapshot JSON.

---

## Non-Goals

Do not hardcode `ToolResult`.

Do not wrap tool calls in `ToolResult<T>` yet.

Do not implement `extend`.

Do not implement methods.

Do not implement `#rust(...)`.

Do not implement provider extensibility.

Do not implement a real prelude or stdlib.

Do not implement runtime execution.

Do not introduce optional narrowing.

---

## Testing Requirements

Plan 6 should be test-heavy.

Minimum test groups:

- `quew-types`: substitution, parameter collection, instantiated display
- `quew-parser`: generic type declarations, generic function declarations,
  generic type usage
- `quew-scope`: declaration metadata for type and function parameters
- `quew-checker`: arity errors, unknown generic parameters, field substitution,
  generic function call inference, return type substitution
- `quew-ir`: generic definitions and instantiated type references are preserved

Tests should use small neutral examples such as `Box<T>` and `Pair<A, B>`.
`ToolResult<T>` may appear only as a regression-style example after the generic
system itself is proven.

---

## Completion Criteria

Plan 6 is complete when:

- generic type declarations are parsed, scoped, checked, and lowered
- generic function declarations are parsed, scoped, checked, and lowered
- generic arity diagnostics are precise
- generic parameter scope is enforced
- generic substitution works for records and function returns
- field access on instantiated generic records works
- generic function calls instantiate correctly for straightforward cases
- tests are broad enough to protect the design
- full `cargo test --workspace` passes

---

## Future Plans

After Plan 6, separate plans should discuss:

- result wrappers such as `ToolResult<T>`
- whether host tool calls should use a configurable wrapper type
- extension methods and `self`
- Rust-backed stdlib hooks
- provider/model extensibility
- moving hardcoded provider builders into a standard provider layer
