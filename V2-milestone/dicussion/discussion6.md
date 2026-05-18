# Discussion 6: Prelude Source And Quew-Owned Runtime Contracts

*Started after Plan 7 completed builtin type declarations and role bindings.*

---

## Why This Discussion Exists

Plan 7 proved that the compiler can parse and preserve declarations such as:

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

But those declarations still have to live somewhere.

If every test or user program has to write `ToolResult<T>` manually, the system
is not really a language builtin system yet. The next step is to create a
trusted Quew source layer that ships with the compiler and becomes the place
where Quew defines Quew.

This is the transition point from:

```text
Rust compiler hardcodes language contracts
```

to:

```text
Rust compiler loads trusted Quew source that declares language contracts
```

---

## Current Codebase Facts

The `quew-compiler` workspace currently has only Rust crates under
`quew-compiler/crates/`.

There is no location for Quew-written builtin source yet.

Relevant current behavior:

- `quew-scope` now has a role registry.
- `quew-ir` now preserves role bindings.
- The checker still builds one symbol table from one parsed `Module`.
- There is no import system or multi-file merge path in the v2 frontend yet.
- `Text` is still a checker primitive alias in `PrimKeys`.
- `with { ... }` and providers are still hardcoded compatibility paths.

So the next prelude step should be deliberately small: load trusted source,
merge it with user source, and prove role-bound contracts are available without
the user writing them.

---

## Proposed Source Layout

Create a workspace-owned prelude area:

```text
quew-compiler/
  prelude/
    core.quew
    tools.quew
    middleware.quew
    with.quew
    providers.quew        # later
```

This directory is not user source. It is compiler-owned trusted source.

Initial files should stay tiny:

```text
prelude/
  tools.quew              # ToolResult<T> and (tool, value)
```

The Rust crates can embed these files with `include_str!` until a more advanced
packaging story exists.

If packaging constraints make workspace-root embedding awkward later, this can
move into a dedicated crate:

```text
crates/quew-prelude/
  src/lib.rs
  src/tools.quew
```

But the first milestone should keep the file location obvious and easy to edit.

---

## Trusted Prelude Rules

Prelude source is not ordinary user source.

Recommended rules:

- Prelude files may use `@@type`, `!@@type`, and role bindings.
- User source may parse these forms for now, but later phases may restrict them
  to trusted source.
- Prelude diagnostics should point to the prelude file name, not user source.
- User source should not be allowed to override prelude names accidentally.
- Duplicate user/prelude definitions should be errors with clear ownership in
  diagnostics.

For Plan 8, the minimum is:

```text
Prelude first, user second, duplicate names still error.
```

---

## Merge Model

Because imports are not implemented yet, the first prelude merge can be simple:

1. Parse every prelude file into a `Module`.
2. Parse the user source into a `Module`.
3. Create a combined module:

```text
combined.items = prelude.items + user.items
```

4. Run `check(combined)`.
5. Lower `combined` to IR, while still selecting the user's entry agent.

This is not the final module system. It is a practical bridge until
`quew-resolve` owns real imports.

---

## First Prelude Contract

The first useful contract is the one Plan 7 prepared:

```quew
@@(tool, value)
type ToolResult<T> = {
    data: T,
    error: string
}
```

Once this is loaded from the prelude, the checker can use `(tool, value)` to
wrap tool call return types.

Example:

```quew
tool getName(): string

function demo(): string {
    let result = getName()
    return result.data
}
```

The type of `result` should become:

```quew
ToolResult<string>
```

The compiler should not know the name `ToolResult`. It should find the wrapper
through the role binding `(tool, value)`.

---

## What Should Not Move Yet

Do not move everything into the prelude immediately.

Keep these hardcoded for now:

- `Text` as a temporary primitive alias;
- provider builders `gemini`, `openai`, `groq`;
- `with { ... }` field validation;
- `isEmpty` temporary method experiment;
- native Rust binding ids.

The first goal is to prove the source-location and role-consumer path without
destabilizing already-working compiler behavior.

---

## Direction

Plan 8 should combine:

1. Create the prelude source home.
2. Load and merge trusted prelude source before user source.
3. Define `ToolResult<T>` in Quew prelude source.
4. Use the `(tool, value)` role to wrap direct tool-call expression types.

That gives us the first real example of Quew defining one of its own language
contracts.

