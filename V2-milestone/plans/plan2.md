# Plan 2: Lexer and Token System

## Where We Are

The workspace scaffold is done and verified. 14 crates, clean architecture, tested foundations.

**Implemented and tested (24 passing tests):**
- `quew-errors` — Span, Diagnostic, Severity
- `quew-interner` — ThreadedRodeo wrapper, InternedStr (u32, Copy, thread-safe)
- `quew-source` — SourceId, SourceFile (line/col mapping), SourceMap (thread-safe registry)

**Stubbed (Cargo.toml + single-responsibility doc, no real code yet):**
- `quew-lexer`, `quew-ast`, `quew-parser`
- `quew-types`, `quew-unify`
- `quew-scope`, `quew-resolve`
- `quew-checker`, `quew-ir`, `quew-codegen`, `quew-cli`

---

## What We Are Implementing Next

**`quew-lexer` — the token system.**

Everything downstream (parser, AST, checker) depends on knowing the complete terminal symbol
set. Designing tokens forces commitment to the surface grammar. Once locked, the parser is
written without revisiting token design.

---

## The Goal of This Plan

By the end of this plan, `quew-lexer` should:

1. Define every `TokenKind` variant for the v2 quew grammar
2. Produce a `LexResult` from raw source text via a `lex()` function
3. Skip whitespace, line comments (`//`), and block comments (`/* */`) transparently
4. Recover from unknown characters by emitting `TokenKind::Error` rather than panicking
5. Have a test for every single `TokenKind` variant, plus edge-case tests
6. Have integration tests that lex real `.quew` fixture files end-to-end

---

## The v2 Grammar Surface (what the lexer must cover)

**Evidence source:** every code example in `not.txt` (1196 lines), read top to bottom.

### Top-level declarations

```
agent   function   tool   tools   type   model   let
```

> `tools` (plural) has two distinct usages — same token, different semantics resolved by the
> parser and checker:
>
> **1. Shorthand group** — `tools { getusername():string @desc "..." }` is syntactic sugar for
> writing multiple `tool` declarations. Every entry inside is treated as an individual `tool`.
>
> **2. Progressive disclosure group** — `tools usertools { ... } @desc "..."` creates a named
> group. The model only sees the group name and its `@desc` at first. It can request the
> individual tools inside if needed — the runtime handles the reveal as an internal mechanism.
> This keeps large tool sets out of the model's context until relevant.
>
> The lexer emits one `Tools` token in both cases. The parser distinguishes them by what
> follows: a bare `{` means shorthand; a name + `{` means a named progressive group.


### Annotations (always `@name`, lexed as single tokens)

```
@tool         — @tool or @tool(args) — marks a function as a DSL-native callable tool
@desc         — @desc "string" — description shown to the model
@middleware   — @middleware("name") — declare a DSL middleware function
@middlewares  — @middlewares(Name) — attach middleware(s) to an agent
@context      — @context(Type) — bind a context type to an agent
@native       — force native provider tool-calling mode on an agent
@block        — force block protocol mode on an agent
```

> `@native` / `@block` are SDK-facing protocol selectors. They don't appear in `not.txt`
> but will be used when targeting host SDKs. Include them now to avoid a future token addition.

### Control flow / expressions

```
if   else   return   reply   with   for   in   is   and   or   not
```

- `is` — type discrimination: `if response.data is MyType { }`
- `for` / `in` — loops: `for idx, value in session.turns { }`
- `and` / `or` / `not` — English logical operators (NOT `&&`, `||`, `!`)
- inline conditional: `value if condition else value` — postfix, no `then` keyword

> `then` is NOT a keyword — confirmed not part of the grammar.
> `final` is NOT a keyword — removed, was a hallucination.

### Types

```
string   number   float   bool   void
```

These are the only types the lexer emits in this plan. `Text`, `Image`, `File`, `Audio`,
`Video` are deferred — they are not part of the v2 first milestone.

### Literals

```
IntLiteral        — e.g. 42
FloatLiteral      — e.g. 3.14
StringLiteral     — e.g. "hello" or "text with {var} interpolation"
TripleString      — e.g. """multi-line {var}"""
BoolLiteral       — true / false
NullLiteral       — null
```

### Identifiers

```
Ident             — any identifier that is not a reserved keyword
```

### Punctuation / operators

```
{  }  (  )  [  ]  <  >   ← delimiters / generics
:  ,  .                   ← structure  (no semicolons — language does not use them)
?                         ← optional parameter marker: id?: string
|                         ← union type separator
=  ==  !=                 ← assignment and equality
+  -  *  /  %             ← arithmetic
```

> **`->` is NOT in the grammar.** Return types use `:` — `function foo(): string { }`.
> **No semicolons.** Statement boundaries are newlines and block delimiters `{ }`.
> **`&&`, `||`, `!` are NOT tokens.** Use `and`, `or`, `not`.

### Special tokens

```
At              — standalone @ not matched by any known annotation
Error           — unrecognised character; lexer continues rather than aborting
```

---

## Key Design Decisions

### 1. Annotations as single tokens

`@tool`, `@desc`, `@middleware`, `@middlewares`, `@context`, `@native`, `@block` are each
one token, not `At` + `Ident`. The lexer greedily matches `@context` before falling through
to `At`. Unknown annotations produce `At` + `Ident` — forward compatible.

**Why:** parser rules are simpler. Error messages say "expected `@tool`" not two tokens.

### 2. `and`, `or`, `not` are keywords — no symbol equivalents

Consistent across all 1196 lines of `not.txt`. Do not add `&&`, `||`, or `!`.

### 3. No semicolons

The language is newline-terminated. `;` is not in the token set.

---

## `LexResult` and `lex()` signature

```rust
pub struct LexResult {
    /// Full token stream in source order, including Error tokens.
    /// The lexer never aborts — always returns the complete stream.
    pub tokens: Vec<(TokenKind, Span)>,
    /// Non-fatal errors (unknown chars, unterminated strings, etc.)
    pub errors: Vec<Diagnostic>,
}

pub fn lex(
    source: &str,
    source_id: SourceId,
    interner: &Arc<Interner>,
) -> LexResult
```

- `source_id` embeds which file each `Span` belongs to.
- `interner` interns all `Ident` values — no heap `String` allocations.

---

## Testing Mandate

- Every `TokenKind` variant → at least one unit test
- Both comment styles → `//` single-line, `/* */` block
- Edge cases:
  - Empty input → empty stream, no panic
  - Whitespace-only → empty stream
  - Unterminated string → `Error` token with correct span
  - Unterminated block comment → `Error` token
  - Unknown character (`$`) → `Error`, lexer continues
  - Unicode identifier → `Ident`
  - `tool{` → `Tool` + `LBrace` (no bleed)
  - `@toolfunction` → should NOT match `@tool`
  - `handsome` → `Ident`, not split on embedded `and`/`or`/`not`
  - Triple-quoted string with `{var}` → `TripleString`
  - `null` → `NullLiteral`, not `Ident`
  - `?` → `Question`, not swallowed
- Integration tests (`tests/lex_snippets.rs`):
  - Minimal agent: `agent Hello(input: string) { }`
  - Host-backed tool: `tool getWeather(): string @desc "..."`
  - DSL tool function: `@tool @desc "..." function getWeather(city: string): string { }`
  - Agent with `reply(...)` and `with` block
  - Inline conditional: `let x = a if cond else b`
  - `for ... in` loop
  - Type discrimination: `if x is MyType { }`

---

## What We Are NOT Doing in This Plan

- `Text`, `Image`, `File`, `Audio`, `Video` tokens — deferred
- Parser (plan 3)
- AST nodes (plan 3)
- Type checking (plan 4+)
- Any runtime, VM, or codegen

---

## Definition of Done

- [x] `TokenKind` enum defined with all variants listed above
- [x] `lex()` implemented using `logos 0.16`
- [x] `LexResult` returned from `lex()`
- [x] Every `TokenKind` variant covered by at least one unit test
- [x] All edge cases above have dedicated tests
- [x] Fixture files in `tests/fixtures/valid/` and `tests/fixtures/invalid/`
- [x] `cargo test -p quew-lexer` → 0 failures — **73 tests, 0 failures**
