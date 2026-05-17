# Discussion 5: Builtin Role Bindings, Rust Hooks, and Extensions

*Started after Plan 6 completed the first generic type-system slice. This
discussion is about how Quew-authored declarations become compiler/runtime
builtins without hardcoding names such as `ToolResult`, `Model`, `Text`, or
`Image` into the checker.*

---

## Core Direction

Quew should define important builtin-facing types and functions in Quew itself.
The compiler should know **roles**, not magic names.

Example:

```quew
@builtin("tool.result")
type ToolResult<T> = {
    data: T,
    error: string
}
```

The visible type name is `ToolResult`, but the compiler-facing semantic role is:

```text
tool.result
```

The type could later be renamed:

```quew
@builtin("tool.result")
type HostCallResult<T> = {
    data: T,
    error: string
}
```

and the compiler should still understand the role.

The rule is:

```text
Quew declarations define shape and names.
@builtin(...) attaches compiler/runtime semantics.
```

---

## Builtin Type Roles

Builtin types should be ordinary Quew types with role metadata.

For tool results:

```quew
@builtin("tool.result")
type ToolResult<T> = {
    data: T,
    error: string
}
```

For model values:

```quew
@builtin("type.model")
type Model = {
    provider: string,
    name: string
}
```

For media values, the same idea applies. `Image` should not become a compiler
primitive. It should be a Quew-authored type:

```quew
@builtin("type.image")
type Image = {
    url: string,
    mime: string
}
```

The exact fields are future design, but the important point is that `Image` is
not magic. It is a type whose role is registered.

---

## Builtin Function Roles

Some builtin roles attach to functions.

Example provider function:

```quew
@builtin("provider.gemini")
function gemini(model: string): Model {
    ...
}
```

This means:

- `gemini` is a normal Quew function name.
- its return type is the Quew-defined `Model`.
- the compiler/runtime can recognize that this function fills the
  `provider.gemini` role.

This is how Quew can eventually stop hardcoding provider names in the lexer and
parser. Provider functions become declarations with builtin roles.

This discussion does **not** design IO. It only defines how declarations attach
to builtin roles.

---

## Tool Declarations And Tool Results

Quew currently has two tool forms:

```quew
tool host_search(query: string): SearchResult
```

and:

```quew
@tool
function local_search(query: string): ToolResult<SearchResult> {
    ...
}
```

They should be treated differently at the implementation boundary but should
share the same result role.

### Host-Backed `tool`

A host-backed tool declares its success value:

```quew
tool host_search(query: string): SearchResult
```

When called from Quew, its expression type should be the registered
`tool.result` wrapper:

```quew
host_search("x") // ToolResult<SearchResult>
```

The host returns the success value. The runtime/compiler boundary wraps it into
the `@builtin("tool.result")` type.

### Quew-Backed `@tool function`

A Quew-backed tool function is authored in Quew, so the function can return the
full result shape itself:

```quew
@tool
function local_search(query: string): ToolResult<SearchResult> {
    if query.isEmpty() {
        return {
            data: emptySearchResult(),
            error: "query is empty"
        }
    }

    return {
        data: runSearch(query),
        error: ""
    }
}
```

Because Quew is structurally typed, the return expression does not need to
explicitly construct `ToolResult<SearchResult>` by name. The checker can compare:

```quew
{
    data: SearchResult,
    error: string
}
```

against:

```quew
ToolResult<SearchResult>
```

after generic substitution.

The important rule:

```text
@tool function return type must satisfy the registered tool.result role.
```

It should not be limited to returning only the success value.

---

## `#rust(...)` Is Implementation Binding

`@builtin(...)` and `#rust(...)` solve different problems.

```text
@builtin(...)
  declares the compiler/runtime role of a Quew item

#rust(...)
  declares that a Quew function's implementation lives in native Rust
```

Example:

```quew
#rust("std.string.is_empty")
function text_is_empty(value: Text): bool
```

This means:

- Quew sees a normal typed function.
- The checker can type-check calls to it.
- The compiler/runtime resolves the Quew type `Text` to its Rust
  representation and executes Rust builtin `std.string.is_empty`.
- It is not a host tool.
- It is not exposed to the model as a tool.

Later, method syntax can be layered on top:

```quew
extend Text {
    #rust("std.string.is_empty")
    function isEmpty(): bool
}
```

Then:

```quew
query.isEmpty()
```

can lower to the Rust-backed implementation with `query` as the receiver.

The Rust id names the Rust-side implementation, not the Quew type. If `Text` is
represented as a Rust `String`, then `std.string.is_empty` is valid. Quew does
not require Rust to have a type literally named `Text`.

So there must be a representation-resolution step:

```text
Quew type role/name -> runtime representation -> Rust builtin signature
```

Example:

```text
Text -> string representation -> std.string.is_empty(String) -> bool
```

If a Quew type has no known Rust representation, then a `#rust(...)` binding for
that type should be rejected unless the binding explicitly declares how to lower
the value.

---

## `extend` Targets Quew Types, Including Builtin-Role Types

Extensions should not be thought of as extending compiler primitive types like
`string` as a magic built-in.

The stronger model is:

```quew
@builtin("type.text")
type Text = {
    value: string
}

extend Text {
    #rust("std.string.is_empty")
    function isEmpty(): bool
}
```

or if `Text` remains an alias-like type later, it is still the Quew-defined
`Text` role being extended, not a hidden compiler primitive.

The same applies to other builtin-role types:

```quew
@builtin("type.image")
type Image = {
    url: string,
    mime: string
}

extend Image {
    #rust("std.image.mime_type")
    function mimeType(): string
}
```

So the rule is:

```text
extend attaches methods to Quew types.
Some of those Quew types may also carry @builtin roles.
```

### `self` In Extensions

Extension methods need an implicit receiver.

Inside:

```quew
extend Text {
    #rust("std.string.is_empty")
    function isEmpty(): bool
}
```

the checker should treat the method as if it had:

```quew
self: Text
```

So:

```quew
query.isEmpty()
```

is checked like:

```quew
isEmpty(self = query)
```

and lowered roughly like:

```text
call_rust_builtin(
    id = "std.string.is_empty",
    receiver = query
)
```

For a Quew-authored extension body:

```quew
extend ToolResult<T> {
    function isOk(): bool {
        return self.error == ""
    }
}
```

the checker injects:

```text
self: ToolResult<T>
```

and generic receiver matching binds `T` from the call site:

```quew
let result: ToolResult<SearchResult>
result.isOk()
```

means:

```text
self: ToolResult<SearchResult>
T = SearchResult
return type: bool
```

The method declaration itself does not list `self` as a normal parameter. It is
provided by the `extend` receiver.

### Rust Binding And Receiver Representation

For Rust-backed extension methods, `self` is passed to Rust after representation
resolution.

Example:

```quew
@builtin("type.text")
type Text = {
    value: string
}

extend Text {
    #rust("std.string.is_empty")
    function isEmpty(): bool
}
```

If `Text` is represented by its `value: string` field at the Rust boundary, the
lowering can pass that string to `std.string.is_empty`.

If `Text` is represented as a full object, then `std.string.is_empty` is invalid
unless there is an adapter.

So `#rust` needs two checks:

```text
1. Does the Quew signature type-check?
2. Can the Quew parameter/receiver types lower to the Rust builtin signature?
```

The second check belongs to builtin/stdlib validation, not ordinary user type
checking.

---

## Structural Matching

Since Quew is structurally typed, role validation should use structure where
possible.

For `tool.result`, the compiler can validate:

```text
arity = 1
required field data: T
required field error: string
```

That does not hardcode the type name. It validates the contract of the role.

This means both are valid if structurally compatible:

```quew
@builtin("tool.result")
type ToolResult<T> = {
    data: T,
    error: string
}
```

```quew
@builtin("tool.result")
type HostResult<T> = {
    data: T,
    error: string
}
```

The role is fixed. The name is not.

---

## Compiler Architecture

Keep this modular.

Suggested ownership:

```text
quew-ast
  annotations on type declarations and function declarations

quew-scope
  registers normal symbols
  registers builtin role bindings
  catches duplicate builtin roles

quew-checker
  validates builtin role contracts
  uses builtin role bindings for call typing
  validates @tool function return types against tool.result

quew-ir
  preserves builtin role bindings
  preserves Rust implementation bindings

future runtime
  resolves #rust implementation ids
```

Do not put all builtin-role logic into `quew-checker/src/lib.rs`.

Likely modules:

```text
quew-scope/src/builtins.rs
quew-checker/src/builtins.rs
quew-ir/src/builtins.rs
```

---

## Initial Builtin Roles To Consider

Keep the initial set small.

```text
tool.result
type.model
provider.gemini
```

Potential later roles:

```text
type.text
type.image
type.file
provider.openai
provider.groq
```

Do not add roles until there is a concrete compiler/runtime behavior attached
to them.

Unknown builtin roles should be diagnostics, not ignored:

```quew
@builtin("toool.result")
type X<T> = { data: T, error: string }
```

should produce:

```text
unknown builtin role "toool.result"
```

---

## Open Questions

1. Should `@builtin(...)` be allowed in user code, or only in the compiler-owned
   stdlib/prelude?

2. Should `tool.result` require exactly `data` and `error`, or should the
   runtime support custom field names later?

3. Should `@tool function` require an explicit `ToolResult<T>` return type, or
   is structural compatibility with the role enough?

4. Should builtin provider functions be ordinary Quew bodies, Rust-backed
   bodies, or either?

5. Should `#rust(...)` be allowed only in trusted stdlib code?

6. How should aliases work for builtin-role types?

---

## Current Recommendation

Use three separate concepts:

```text
Quew declaration
  defines names and types

@builtin("role")
  binds a declaration to compiler/runtime semantics

#rust("impl.id")
  binds a function/method implementation to native Rust
```

Then later:

```quew
@builtin("tool.result")
type ToolResult<T> = {
    data: T,
    error: string
}

@builtin("type.model")
type Model = {
    provider: string,
    name: string
}

@builtin("provider.gemini")
function gemini(model: string): Model {
    ...
}

extend Text {
    #rust("std.string.is_empty")
    function isEmpty(): bool
}
```

This keeps the compiler from hardcoding names while still giving it precise
semantic hooks.
