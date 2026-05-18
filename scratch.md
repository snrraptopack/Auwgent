# Quew Annotation System

*How the language defines its own builtins without hardcoding names in the compiler.*

---

## The Problem

The compiler currently has no principled way to know what `ToolResult` is for,
what shape a `with { }` block expects, or what argument a `@middleware` function
must accept. The naive solution is to hardcode those names inside the compiler —
but that means every time the language grows, you are editing the compiler itself
instead of writing Quew. The goal is to move all of that into Quew so the
compiler knows **roles and contracts**, not magic names.

---

## The System at a Glance

```
@@type Name              — global builtin type, ships like a primitive
!@@type Name             — internal builtin type, stdlib/compiler only
@@(keyword, place) type  — concept-bound type, tells the compiler what a type is for
@@function name          — global builtin function
!@@function name         — internal builtin function
#rust("id")              — leaf native binding to a Rust implementation
extend Type { }          — adds methods to any type, implicit self
```

---

## `@@type` — Global Builtin Type

The initial type system ships with `number`, `float`, `string`, and `bool`. The
aim is to extend it — `Image`, `Text`, `Model`, `Audio` and so on — without
making users import anything. When you write `@@type` with a single-word name,
the compiler registers it and ships it with the binary exactly like a primitive.
It is just there, everywhere, no import needed.

```ts
@@type Image = {
    url: string,
    mimeType: string
} | {
    path: string,
    mimeType: string
}

@@type Text = {
    value: string
}

@@type Model = {
    provider: string,
    name: string
}
```

So a user can write `let img: Image = { url: "...", mimeType: "image/png" }` and
the compiler already understands `Image` the same way it understands `string`.

---

## `!@@type` — Internal Builtin Type

Same idea as `@@type` but the type is not accessible to users. It is for
building up internal pieces that feed into public types. Users never see the
name, only the composed result. In practice `!@@` is used more heavily on
functions than on types — but it is available on types for cases like this:

```ts
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

`OnRunStart` and `OnRunFinish` are internal building blocks. The public face of
both of them together is `MiddlewareEvent`, defined below via the concept
binding. The `!@@` marker is a visibility boundary — it does not change how the
type behaves or what its shape is.

---

## `@@(keyword, place)` — Concept-Bound Type

This is where the compiler gets its role-awareness. Instead of hardcoding
`ToolResult` as the magic return-wrapper for tools, or hardcoding that
`@middleware` functions need a specific argument, we bind a type to a compiler
role explicitly using `@@(keyword, place)`.

Every keyword in Quew structurally has three potential attachment points:

| place   | meaning                                                     |
|---------|-------------------------------------------------------------|
| `value` | the type that wraps the return value of calls to `keyword`  |
| `args`  | the type required as an argument for `@keyword` functions   |
| `body`  | the type that defines the shape of the `keyword { }` block  |

These slots are always present on every keyword. `@@(keyword, place)` lets you
bind a global type to any of them. Whether a particular binding makes sense is
a language design question — the compiler does not enforce a fixed matrix of
allowed combinations. It simply applies whatever binding is declared and the
type checker surfaces any inconsistency when the binding is actually used.

In practice some combinations do not make sense and nobody would reach for them.
For example `@@(tool, args)` is expressible but useless — tools each define
their own arguments, so binding a fixed global type to tool args would mean
every tool in the entire program takes the same arguments, which defeats the
purpose. Same with `@@(tool, body)` — tools do not have a meaningful block body
to bind a shape to. These combinations are not illegal, they are just never the
right answer.

### `(tool, value)` — Wrapping tool return values

When a user defines a tool and it returns `string`, the compiler does not hand
back a raw `string` to the caller. It wraps it. But rather than hardcoding the
wrapper type name, we tell the compiler which type plays that role:

```ts
@@(tool, value) type ToolResult<T> = {
    value: T,
    error: string
}
```

Now when a user writes:

```ts
tool hello(): string
```

and calls `hello()`, the compiler knows the caller receives `ToolResult<string>`,
not `string`. The compiler learned this from the binding, not from the name
`ToolResult`.

### `(middleware, args)` — Required argument for middleware functions

We want any function decorated with `@middleware` to compulsorily accept the
middleware event as its argument. Rather than hardcoding that check, we bind
the event type to the role:

```ts
@@(middleware, args) type MiddlewareEvent = OnRunStart | OnRunFinish
```

Now the compiler knows that any `@middleware` function must accept
`MiddlewareEvent`. It validates against the role, not the name. If the user
provides the wrong argument type, the compiler catches it.

```ts
@middleware("name")
function myMiddleware(event: MiddlewareEvent) {
    // event is narrowed to OnRunStart | OnRunFinish
}
```

### `(with, body)` — Shape of the `with { }` block

In agent code we write things like:

```ts
agent One(input: string): string {
    reply(input) with {
        model: gemini()
        prompt: ""
    }
}
```

The `with` block needs a defined shape so the compiler can validate it. Instead
of hardcoding that shape, we declare it:

```ts
@@(with, body) type WithBlock = {
    model: Model,
    prompt: string,
    tools: ...
}
```

When the compiler sees a `with { }` block anywhere it validates the contents
against `WithBlock`. The keyword `with` is bound to this shape through the
annotation, not through any hardcoded handler.

---

## `@@function` and `!@@function` — Builtin Functions

The same public/internal distinction applies to functions. `@@function` ships
globally with the language — no import needed anywhere. `!@@function` is
internal, used only when building the stdlib itself. This is where `!@@` sees
the most use — surfacing Rust-backed functions into the language without
exposing them to users.

```ts
@@function gemini(model: string): Model { ... }
@@function openai(model: string): Model { ... }
@@function groq(model: string): Model { ... }
```

```ts
!@@function string_is_empty(value: string): bool
!@@function string_contains(value: string, substring: string): bool
```

---

## `#rust("id")` — Leaf Native Binding

When the language needs to reach into Rust — for something the type system
cannot express or that must run natively — we model it as a standard function
signature with a `#rust` binding. This keeps the native boundary explicit and
as small as possible. Everything above that boundary is written in Quew.

Every time we reach out to something in Rust we model it as a function whose
signature lives in Quew and whose implementation lives in Rust:

```ts
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

`#rust` is also allowed directly on an `extend` method when the receiver itself
is the native boundary:

```ts
extend string {
    #rust("std.string.trim")
    function trim(): string
}
```

---

## `extend Type { }` — Methods and `self`

The initial type system gives us `number`, `string`, `bool` and so on, but they
have no methods. We want to be able to write `"hello".contains("ll")` the same
way you would in any modern language, but because this is a new language we
cannot just reach into the host. We need to surface those capabilities
explicitly. `extend` is how we do it — it adds methods to any type, with `self`
as the implicit receiver.

```ts
extend string {
    function isEmpty(): bool {
        string_is_empty(self)
    }

    function contains(substring: string): bool {
        string_contains(self, substring)
    }
}
```

Now `"hello".contains("ll")` works and the compiler knows it. You can extend
custom types too:

```ts
extend Text {
    function isEmpty(): bool {
        self.value.isEmpty()    // delegates to string.isEmpty() above
    }
}
```

```ts
extend Image {
    function isUrl(): bool {
        if self is { url: string, mimeType: string } {
            return true
        }
        return false
    }
}
```

```ts
extend ToolResult<T> {
    function isOk(): bool {
        return self.error == ""
    }
}
```

The `is` keyword checks the shape of a value and narrows the type inside the
branch. So `if self is { url: string, mimeType: string }` is essentially asking
"does this value look like the url variant of Image" — and inside the branch the
compiler knows it does.

---

## Composition Rule

The whole system composes in one direction:

```
#rust at the smallest native boundary
  ↓
extend to build ergonomic methods in Quew
  ↓
@@type to ship as a compiler-known builtin
  ↓
@@(keyword, place) to bind it to a compiler role
```

Full example — `Text.isEmpty()` from the Rust boundary all the way to a
user-facing method call:

```ts
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

A user writing `myText.isEmpty()` never sees any of the layers below. The
composition is invisible to them.
