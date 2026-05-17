# Discussion 4: Generics Before Extensions

*Started after Plan 5, while deciding whether Plan 6 should implement
`ToolResult`, extension methods, Rust-backed stdlib hooks, and provider
extensibility. The revised decision is to focus Plan 6 only on a robust generic
type system.*

---

## Revised Decision

Plan 6 should not be centered on `ToolResult<T>`.

`ToolResult<T>` is a useful future type, but it should not become a hardcoded
compiler concept. Quew needs a generic system strong enough that `ToolResult<T>`
can be written as an ordinary language or stdlib type later.

The new Plan 6 focus is:

```text
generic types + generic functions + substitution + checking + IR preservation
```

Everything else moves later:

- tool result wrapping
- extension methods
- `self`
- `#rust(...)`
- prelude/stdlib loading
- provider/model extension
- request/IO primitives

---

## Why The Previous Direction Was Too Narrow

The previous direction started from this example:

```quew
type ToolResult<T> = {
    data: T,
    error: string
}

tool delete_user(id: string): bool
```

and asked the compiler to understand that a tool call returns
`ToolResult<bool>`.

That is useful, but it risks making the checker special-case one type before the
generic system itself is mature.

The better foundation is to make all of these work first:

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

function first<A, B>(pair: Pair<A, B>): A {
    return pair.first
}
```

After that, `ToolResult<T>` is just another generic type.

---

## What A Robust Generic System Needs

### Generic Type Declarations

```quew
type Box<T> = {
    value: T
}
```

The compiler must record the ordered type parameters for each type declaration.

### Generic Type Instantiation

```quew
let x: Box<string>
```

The checker must validate arity and substitute `T = string`.

### Multiple Type Parameters

```quew
type Pair<A, B> = {
    first: A,
    second: B
}
```

Substitution must be by parameter identity, not by position after parsing.
Position only defines how call-site arguments bind.

### Generic Functions

```quew
function identity<T>(value: T): T {
    return value
}
```

Generic functions should have their own type parameters. Those parameters are in
scope for the function signature and body.

### Generic Function Calls

```quew
let name = identity("alice")
```

The checker should infer `T = string` for straightforward calls.

When inference is ambiguous, the compiler should emit a clear diagnostic rather
than guessing.

### Generic Field Access

```quew
function getValue<T>(box: Box<T>): T {
    return box.value
}
```

The checker must resolve `box.value` to `T`, and then to the concrete type once
the function is instantiated.

---

## Code Organization Expectations

The implementation should avoid pushing all logic into one large checker file.

Suggested ownership:

```text
quew-types
  Owns generic type algebra and substitution.

quew-scope
  Records declaration metadata and generic parameter scopes.

quew-checker
  Resolves types, instantiates generic calls, checks fields, and reports diagnostics.

quew-ir
  Preserves generic definitions and instantiated type references.
```

Possible checker module split:

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

This split should be practical, not ceremonial. Add modules when they own real
logic and reduce complexity.

---

## Testing Expectations

The generic system should be heavily tested before runtime or stdlib behavior
depends on it.

Required areas:

- generic type declaration parsing
- generic function declaration parsing
- generic type arity diagnostics
- unknown generic parameter diagnostics
- generic substitution in records
- field access on instantiated generic records
- generic function inference
- return type checking with generic returns
- IR preservation of generic definitions

Tests should use neutral fixtures:

```quew
Box<T>
Pair<A, B>
Result<T, E>
identity<T>
first<A, B>
```

`ToolResult<T>` can be used later as a consumer test, but not as the foundation.

---

## Deferred Topics

### ToolResult

Future discussion should decide how host tool calls get wrapped.

Possible direction:

```quew
type ToolResult<T> = {
    data: T,
    error: string
}
```

But this should be layered on top of the generic system, not baked into it.

### Extensions

Future discussion should decide:

```quew
extend string { ... }
extend Result<T, E> { ... }
extend Model { ... }
```

This needs generic receiver matching, `self`, method lookup, and possibly
Rust-backed implementations. It is not Plan 6.

### Providers

Future discussion should decide whether provider builders such as
`gemini("...")` become stdlib/prelude functions instead of hardcoded parser
constructs.

That depends on extensions and stdlib design, so it should not be mixed into the
generic type-system plan.

---

## Current Position

Plan 6 is now:

```text
Build a robust, documented, well-tested generic type system.
```

It is not:

```text
Hardcode ToolResult.
Implement extensions.
Implement providers.
Implement stdlib.
```

Once generics are strong, those later features can be designed cleanly instead
of forcing special cases into the compiler.
