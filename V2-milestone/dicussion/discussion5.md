# Discussion 5: Quew Builtin Annotation System

*Started after Plan 6, after the generic type system was completed. This
discussion turns the notes from `scratch.md` into the next design direction:
letting Quew define language builtins and compiler contracts without hardcoding
magic type names in the compiler.*

---

## Current Position

Plan 6 gave Quew the missing foundation for this work: generic types, generic
functions, substitution, arity checking, and IR preservation.

That means the compiler no longer needs to special-case future types such as:

```quew
ToolResult<T>
Result<T, E>
Box<T>
MiddlewareEvent
WithBlock
Model
Text
Image
```

The next problem is not generics. The next problem is how the compiler learns
which user-visible or stdlib-defined types have special language roles.

For example:

- Which type wraps tool call results?
- Which type defines the shape of `with { ... }`?
- Which argument type must `@middleware` functions accept?
- Which types are builtin and globally available without import?
- Which functions are builtin provider builders?
- Which functions are backed by native Rust code?

The naive answer is to hardcode names like `ToolResult`, `WithBlock`,
`MiddlewareEvent`, `gemini`, and `openai` directly in the compiler.

That is the wrong long-term shape. Every language feature would become a
compiler edit. The better direction is:

```text
The compiler should understand roles and contracts.
Quew or the stdlib should define the names and shapes.
```

---

## Goal

Build a small annotation system that lets Quew define its own builtin surface:

```text
@@type Name              global builtin type
!@@type Name             internal builtin type
@@(keyword, place) type  compiler-role binding
@@function name          global builtin function
!@@function name         internal builtin function
#rust("id")              native Rust implementation binding
extend Type { ... }      methods with implicit self
```

This should let the compiler stop asking:

```text
Is this type named ToolResult?
Is this function named gemini?
Is this block named with?
```

and instead ask:

```text
Which type is bound to (tool, value)?
Which type is bound to (with, body)?
Which function is registered as a builtin provider builder?
Which function has a native Rust implementation id?
```

---

## Core Idea

Quew needs a prelude-like source layer that ships with the compiler.

That prelude is written mostly in Quew, but can expose a small number of native
Rust leaves through explicit bindings.

The compiler loads this builtin source before user source, records builtin
declarations, and uses role bindings to drive special checking.

The important separation:

| Layer | Responsibility |
|---|---|
| Rust compiler | Understands annotations, roles, native binding ids, and safety rules |
| Quew prelude | Defines builtin types, builtin functions, methods, and role bindings |
| User source | Uses the resulting language surface without importing it |

The compiler still owns enforcement. But Quew defines the contracts being
enforced.

---

## `@@type`: Global Builtin Types

`@@type` declares a type that ships globally with the language.

Users do not import these types. They are available the same way primitive types
are available.

```quew
@@type Text = {
    value: string
}

@@type Image = {
    url: string,
    mimeType: string
} | {
    path: string,
    mimeType: string
}

@@type Model = {
    provider: string,
    name: string
}
```

This keeps types like `Text`, `Image`, `Audio`, `Video`, `Model`, and future
runtime-facing types out of hardcoded Rust enum cases unless they truly are
primitive.

The compiler sees them as builtin declarations loaded before user code.

---

## `!@@type`: Internal Builtin Types

`!@@type` declares a builtin type that is visible to the prelude/compiler layer
but hidden from normal user source.

It is useful for composing public builtin contracts out of smaller internal
pieces.

```quew
!@@type OnRunStart = {
    session: string,
    context: string
}

!@@type OnRunFinish = {
    session: string,
    context: string,
    result: string
}
```

The `!@@` marker is a visibility rule, not a different type-system rule. The
type behaves normally, but users cannot name it unless it is exposed through a
public builtin type.

---

## `@@(keyword, place)`: Compiler Role Bindings

This is the most important part of the design.

`@@(keyword, place)` binds a type to a compiler-recognized role.

Each language keyword can expose up to three conceptual slots:

| Place | Meaning |
|---|---|
| `value` | Type wrapper for values produced by that keyword or call family |
| `args` | Required argument type for functions annotated by that keyword |
| `body` | Expected shape for a block owned by that keyword |

The compiler does not care about the bound type's name. It cares about the
binding.

### Tool Return Wrapper

Instead of hardcoding `ToolResult<T>`, the prelude can say:

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

Then a host tool declaration:

```quew
tool getName(): string
```

can be treated by the checker/runtime as producing:

```quew
ToolResult<string>
```

The compiler learned that from `(tool, value)`, not from the name
`ToolResult`.

### Middleware Argument Contract

Instead of hardcoding the required parameter type for middleware functions:

```quew
@@(middleware, args)
type MiddlewareEvent = OnRunStart | OnRunFinish
```

Then:

```quew
@middleware("logger")
function Logger(event: MiddlewareEvent): void {
    ...
}
```

The checker validates `@middleware` functions against the type bound to
`(middleware, args)`.

### `with { }` Block Shape

The `reply(...) with { ... }` block can be described through a role-bound type:

```quew
@@(with, body)
type WithBlock = {
    model: Model,
    prompt: string,
    tools: ToolList?,
    fallback: Model?,
    retry: number?,
    maxTurn: number?
}
```

When the checker sees a `with { }` block, it validates the block against the
type bound to `(with, body)`.

This removes hardcoded field validation from the checker over time. The checker
still performs validation, but the expected shape comes from the builtin
contract.

---

## Invalid Or Useless Bindings

The system should not need a large hardcoded matrix of allowed bindings.

Some bindings are expressible but not useful:

```quew
@@(tool, args) type GlobalToolArgs = { ... }
@@(tool, body) type ToolBody = { ... }
```

These do not make much language sense because tools define their own params and
host-backed tools have no body.

The compiler can start permissive and only reject bindings that are structurally
impossible or duplicate an existing role binding.

Recommended initial rule:

- one binding per `(keyword, place)`;
- duplicate binding is a compiler error;
- unknown keyword/place is a compiler error;
- semantically useless but known bindings can be warnings later.

---

## `@@function` and `!@@function`

Builtin functions follow the same public/internal split.

`@@function` declares a function available globally without import:

```quew
@@function gemini(model: string): Model
@@function openai(model: string): Model
@@function groq(model: string): Model
```

This is the path toward moving provider builders out of hardcoded parser or
checker logic.

`!@@function` declares an internal builtin helper:

```quew
!@@function string_is_empty(value: string): bool
!@@function string_contains(value: string, substring: string): bool
```

User code cannot call internal functions directly. Public builtin methods or
functions can use them.

---

## `#rust("id")`: Native Rust Leaves

Some operations cannot be implemented in Quew itself. They need to call native
runtime code.

Those should be explicit, narrow leaves:

```quew
!@@
#rust("std.string.is_empty")
function string_is_empty(value: string): bool

!@@
#rust("std.string.contains")
function string_contains(value: string, substring: string): bool

!@@
#rust("std.fetch")
function fetch(url: string): ToolResult<string>
```

This gives the runtime a stable native dispatch id while keeping the public
signature in Quew.

Important rule:

```text
#rust is not a general escape hatch for user code.
It is allowed in compiler/prelude-owned builtin source first.
```

User-defined native bindings may be considered later, but they should not be in
the first implementation.

---

## `extend Type { ... }`: Methods And `self`

Extensions are the ergonomic layer on top of builtin functions.

```quew
extend string {
    function isEmpty(): bool {
        string_is_empty(self)
    }

    function contains(substring: string): bool {
        string_contains(self, substring)
    }
}
```

Then user code can write:

```quew
"hello".contains("ll")
```

Extensions should work for builtin and user-defined types:

```quew
extend Text {
    function isEmpty(): bool {
        self.value.isEmpty()
    }
}

extend ToolResult<T> {
    function isOk(): bool {
        self.error == ""
    }
}
```

This depends on the generic work from Plan 6. Generic receiver matching needs
to understand `ToolResult<T>` as a receiver pattern.

---

## Composition Direction

The design composes from the smallest native boundary to the user-facing
language surface:

```text
#rust native leaf
  -> internal builtin function
  -> extension method or public builtin function
  -> builtin type
  -> compiler role binding
  -> user-facing language behavior
```

Example:

```quew
!@@
#rust("std.string.is_empty")
function string_is_empty(value: string): bool

extend string {
    function isEmpty(): bool {
        string_is_empty(self)
    }
}

@@type Text = {
    value: string
}

extend Text {
    function isEmpty(): bool {
        self.value.isEmpty()
    }
}
```

A user writing this:

```quew
message.isEmpty()
```

does not need to know that the final implementation crosses into Rust.

---

## Implementation Shape

This should probably be split into multiple plans rather than implemented as
one large feature.

### Phase 1: Builtin Declaration Parsing

Add syntax support for:

- `@@type`
- `!@@type`
- `@@function`
- `!@@function`
- `@@(keyword, place) type`

This requires lexer/parser/AST support and symbol-table storage.

Do not implement `extend` or `#rust` yet if it makes the first step too large.

### Phase 2: Role Registry

Create a compiler role registry populated from builtin declarations:

```rust
RoleKey {
    keyword: KeywordRole,
    place: RolePlace,
}

RoleBinding {
    role: RoleKey,
    type_name: InternedStr,
    span: Span,
}
```

The checker can then look up:

- `(tool, value)`
- `(middleware, args)`
- `(with, body)`

and apply those contracts.

### Phase 3: Prelude Loading

Add a builtin Quew source file or embedded string loaded before user source.

The first prelude can define:

```quew
@@type Text = { value: string }
@@type Model = { provider: string, name: string }

@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

This phase should prove that user code can reference prelude types without
importing them.

### Phase 4: Builtin Function Registry

Add public/internal builtin functions:

```quew
@@function gemini(model: string): Model
@@function openai(model: string): Model
@@function groq(model: string): Model
```

This is the path toward removing hardcoded provider call expression handling.

### Phase 5: Native Rust Bindings

Add `#rust("id")` for builtin/prelude functions.

The runtime or evaluator maps native ids to Rust implementations.

### Phase 6: Extensions

Add:

- `extend Type { ... }`
- implicit `self`
- method lookup
- generic receiver matching

Extensions should be built on top of the generic system, not before it.

---

## What This Should Not Do Yet

This discussion should not turn into a runtime execution plan.

Do not mix in:

- graph executor implementation;
- journal/checkpoint implementation;
- provider driver changes;
- TypeScript/Python/Dart SDK generation;
- full stdlib design;
- user-defined native plugins.

The immediate goal is compiler representation and checking for builtin
contracts.

---

## Open Questions

1. Should builtin declarations live in a real `.quew` file, an embedded Rust
   string, or both?
2. Should `@@type` and `@@function` be legal in user source, or only in trusted
   prelude/compiler source?
3. Should `!@@` be a prefix token or parsed as `!` plus `@@`?
4. Should role bindings allow user-defined keywords later, or only compiler
   known keywords?
5. Should duplicate role bindings always be errors, including across imports?
6. Should `with { }` validation move fully to `(with, body)` immediately, or
   should the current hardcoded checker stay as a fallback during migration?
7. Should provider builders become ordinary builtin functions immediately, or
   after extension/native binding support exists?
8. How should internal builtin names appear in diagnostics, if at all?

---

## Recommended Next Plan

The next plan should be narrow:

```text
Plan 7: Builtin declarations and role bindings.
```

Suggested scope:

1. Add lexer/parser/AST support for `@@type`, `!@@type`, and
   `@@(keyword, place) type`.
2. Record builtin visibility and type params in scope metadata.
3. Build a role binding registry.
4. Load a tiny embedded prelude before user source.
5. Prove `ToolResult<T>` can be role-bound as `(tool, value)`.
6. Prove `WithBlock` can be role-bound as `(with, body)`.
7. Add tests around duplicate role bindings and unknown role keys.

Explicitly defer:

- `@@function`;
- `#rust`;
- `extend`;
- provider migration;
- runtime execution.

That keeps the next step aligned with the Plan 6 lesson: build the foundation
first, then layer language features on top.

---

## Codebase Fit After Inspection

The current `quew-compiler` shape affects how this should be implemented.

Important existing facts:

- `quew-lexer` currently tokenizes normal annotations through one
  `Annotation(AnnotationKind)` token for `@name`. It does not have tokens for
  `@@`, `!@@`, or `#rust`.
- `quew-ast::Item` currently has only ordinary source items: `Agent`,
  `Function`, `Tool`, `Tools`, `Type`, `Model`, and top-level `Let`.
- `FunctionDecl` and `TypeDecl` already carry `type_params`, so generic builtin
  types and functions can reuse the Plan 6 generic machinery.
- `quew-scope::Symbol` already has `kind`, `ty`, `def_span`, and
  `type_params`, but it has no builtin visibility flag and no role registry.
- `quew-checker::keys::PrimKeys` still treats `Text` as a primitive alias for
  `string`. Moving `Text` to a prelude type will require care and should not be
  bundled into the first role-binding step unless tests are updated around it.
- `reply(...) with { ... }` validation currently lives in
  `quew-checker::check_with_block()` and is driven by hardcoded
  `WellKnownKeys`.
- Provider builders (`gemini`, `openai`, `groq`) are currently dedicated lexer,
  AST, parser, checker, and IR concepts. Moving them to `@@function` is a later
  migration, not the first step.
- `quew-ir::Definitions` already preserves generic type and function metadata,
  but it has no place for builtin visibility, role bindings, or native Rust
  binding ids.

Because of that, Plan 7 should avoid trying to remove hardcoded provider
builders, replace all primitive aliases, or implement methods immediately.

The safest next slice is:

```text
parse builtin type declarations
record builtin visibility
record role bindings
prove the checker can read the role registry
keep existing hardcoded validation as a compatibility fallback
```

That lets the compiler grow the new contract system without destabilizing the
working parser, checker, and IR lowering pipeline.
