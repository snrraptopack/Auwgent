# Discussion 7: Switching Native Binding Syntax To `@@rust`

*Started during Plan 11 implementation.*

---

## Why This Discussion Exists

Plan 11 originally followed Discussion 5 and used:

```quew
#rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool
```

During implementation we decided to switch the native binding prefix to:

```quew
@@rust("std.string.is_empty")
!@@function string_is_empty(value: string): bool
```

This discussion records that change so future readers do not treat it as an
accidental syntax drift.

---

## Reason For The Change

`#rust` would introduce a new directive family into the lexer and parser.
Native bindings are not ordinary user syntax; they are trusted builtin metadata.
That makes them closer to existing builtin declaration syntax:

```quew
@@type
!@@type
@@function
!@@function
@@(tool, value) type
```

Using `@@rust("id")` keeps native implementation ids in the builtin annotation
family instead of adding a second prefix system.

The syntax also avoids edge cases around hash-prefixed tokens such as `#rusty`
and keeps the lexer grammar smaller:

```text
@@rust("id") + !@@function ...
```

is easier to distinguish from ordinary source than a general `#name` directive
path.

---

## Decision

Plan 11 should implement:

```quew
@@rust("native.id")
@@function publicName(arg: Type): ReturnType

@@rust("native.internal.id")
!@@function internalName(arg: Type): ReturnType
```

Rules:

- `@@rust("id")` may only annotate builtin function signatures.
- `@@rust("id") function foo(...) { ... }` is invalid.
- `@@rust` stores metadata only; it does not execute native code yet.
- Runtime dispatch remains deferred.
- `extend`, implicit `self`, and method lookup remain deferred to Plan 12.

---

## Impact On Previous Notes

Older discussions and plans mention `#rust("id")`. Treat those references as
the earlier design name for the same feature. From Plan 11 onward, the concrete
syntax is:

```quew
@@rust("id")
```

The concept is unchanged: a trusted builtin function can carry a stable native
Rust implementation id.
