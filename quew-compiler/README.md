# quew-compiler

The **quew** compiler is the v2 rewrite of the Auwgent DSL compiler. It is a clean-slate, modular
Rust workspace — 14 independently publishable crates — designed to compile `.quew` files into an
in-memory execution graph consumed by the quew runtime.

---

## Project Goals

- **Single responsibility per crate** — every crate does exactly one thing. If you can describe a
  crate's job in more than one sentence without using "and", it should be split.
- **Sound structural type system** — union discrimination, shape checking, and assignment
  compatibility are first-class. No inference hacks.
- **Resumable execution model** — the compiler emits an `ExecutionGraph` (not JSON) that the quew
  runtime can checkpoint and resume across process restarts.
- **Parallel-ready from day one** — `lasso::ThreadedRodeo` is used throughout so multi-file
  parallel compilation can be added without signature-breaking refactors.
- **Aggressively tested** — see the [Testing Mandate](#testing-mandate) below.
- **Rust 2024 edition throughout** — all workspace members use `edition = "2024"`.
- **Zero dead code** — every public item is used. Every private helper has a test.

---

## Workspace Layout

```
quew-compiler/
├── Cargo.toml                ← workspace root; all shared deps declared here
├── README.md                 ← this file
│
└── crates/
    │
    │   ── LAYER 0: absolute foundations (no quew-* deps) ──────────────────
    ├── quew-errors/          ← Span, Diagnostic, Severity — pure types, no rendering
    ├── quew-interner/        ← Interner (ThreadedRodeo), InternedStr — string interning only
    │
    │   ── LAYER 1: source files ───────────────────────────────────────────
    ├── quew-source/          ← SourceId, SourceFile, SourceMap — file registry + span→file mapping
    │
    │   ── LAYER 2: surface syntax ─────────────────────────────────────────
    ├── quew-lexer/           ← TokenKind, Token, lex() — tokenization only
    ├── quew-ast/             ← AST node structs/enums — data only, zero logic
    ├── quew-parser/          ← chumsky 0.13 parser — tokens → AST
    │
    │   ── LAYER 3: type system ─────────────────────────────────────────────
    ├── quew-types/           ← Ty enum — type representation only, no algorithms
    ├── quew-unify/           ← UnifyTable, TyVar — type variable unification (wraps ena)
    │
    │   ── LAYER 4: name resolution ──────────────────────────────────────────
    ├── quew-scope/           ← Scope, SymbolTable — single-file symbol binding only
    ├── quew-resolve/         ← ModuleGraph, ImportResolver — cross-file resolution + cycle detection
    │
    │   ── LAYER 5: semantic analysis ───────────────────────────────────────
    ├── quew-checker/         ← full type checking pass; orchestrates layers 3 + 4
    │
    │   ── LAYER 6: lowering + output ────────────────────────────────────────
    ├── quew-ir/              ← AST → ExecutionGraph lowering
    ├── quew-codegen/         ← type-stub emitters; targets are internal modules:
    │                             src/ts/      ← TypeScript .d.ts
    │                             src/python/  ← Python .pyi
    │                             src/dart/    ← Dart abstract classes
    │                             src/rust/    ← Rust traits + structs
    │
    │   ── LAYER 7: user-facing binary ───────────────────────────────────────
    └── quew-cli/             ← binary: quew check | compile | generate | watch
```

> **Runtime note:** The quew runtime lives in a separate workspace. The compiler exposes
> `quew-ir`'s `ExecutionGraph` type as the handoff point. When the runtime workspace is created,
> `quew-ir-types` will be extracted as a shared crate.

---

## Single Responsibility Rule

Each crate must pass the **one-sentence test**: you must be able to describe what the crate does
in a single sentence that contains no "and".

| Crate | Its one job |
|-------|-------------|
| `quew-errors` | Defines the types that represent compiler diagnostics and source spans |
| `quew-interner` | Interns strings into `u32`-sized handles that are safe to copy across threads |
| `quew-source` | Tracks which source file each span belongs to and maps byte offsets to line/column |
| `quew-lexer` | Transforms raw source text into a flat token stream |
| `quew-ast` | Defines the data structures that represent a parsed quew program |
| `quew-parser` | Transforms a token stream into an AST, recovering from syntax errors |
| `quew-types` | Defines the `Ty` type — the in-memory representation of a quew type |
| `quew-unify` | Maintains a unification table for type variables during inference |
| `quew-scope` | Builds a symbol table by walking the AST of a single file |
| `quew-resolve` | Resolves cross-file imports by building and querying a module dependency graph |
| `quew-checker` | Validates the AST against the type system and emits diagnostics for violations |
| `quew-ir` | Lowers a type-checked AST into an in-memory `ExecutionGraph` |
| `quew-codegen` | Emits target-language type stubs from an `ExecutionGraph` |
| `quew-cli` | Provides the `quew` command-line binary that runs the compiler pipeline |

If a crate starts accumulating a second job, extract a new crate before merging.

---

## Testing Mandate

> **Testing is not optional. It is part of the definition of done.**
> A feature without tests is not finished. A bug without a regression test is not fixed.

Every crate in this workspace follows these rules without exception:

### Rule 1 — Test files mirror source files

For every `src/foo.rs` there must be a corresponding `tests/foo_tests.rs` (or a
`#[cfg(test)] mod tests` block inside `src/foo.rs` for unit tests). Both are required:

```
crates/quew-lexer/
  src/
    lib.rs          ← #[cfg(test)] mod tests { ... } for unit tests
    token.rs        ← #[cfg(test)] mod tests { ... }
  tests/
    lex_full.rs     ← integration tests: real .quew snippets → token streams
    fixtures/
      valid/        ← .quew files expected to lex without errors
      invalid/      ← .quew files expected to produce specific lex errors
```

### Rule 2 — Every public item must be tested

- Every `pub fn` → at least one happy-path test and one error-path test.
- Every `pub struct` constructor → tested.
- Every `pub enum` variant → hit by at least one test.
- Every `impl` block method → tested in isolation.

**There are no exceptions.** If you add a public function without a test, the PR is rejected.

### Rule 3 — Test edge cases explicitly

For any function that takes a collection, always test:
- Empty input (`[]`, `""`, `None`)
- Single-element input
- Large/stress input (where applicable)

For any function that returns `Result` or `Option`:
- Test the `Ok`/`Some` path
- Test the `Err`/`None` path
- Test every distinct error variant

### Rule 4 — Snapshot tests for all string output

Any function that produces rendered output — diagnostics, generated code, IR dumps — must have a
snapshot test using [`insta`](https://crates.io/crates/insta):

```rust
#[test]
fn diagnostic_renders_correctly() {
    let d = Diagnostic::error("undefined variable `x`", span);
    insta::assert_snapshot!(render_to_string(&d));
}
```

Run `cargo insta review` to approve any intentional output changes. Unreviewed snapshot diffs
block CI.

### Rule 5 — Integration tests use real `.quew` fixture files

Every crate that processes `.quew` source (lexer, parser, checker, IR) must have a `tests/fixtures/`
directory with committed `.quew` files:

- `tests/fixtures/valid/` — programs that must compile without errors
- `tests/fixtures/invalid/` — programs that must produce specific, named diagnostics

Fixture files are regression anchors. If a change causes a fixture to produce different output,
it must be intentional and reviewed.

### Rule 6 — Thread-safety tests for concurrent paths

Any crate that touches `quew-interner` or any `Arc<T>` must have at least one test that spawns
multiple threads and verifies there are no data races or panics.

### Coverage targets per crate

| Crate | Unit test focus | Integration test focus |
|-------|----------------|------------------------|
| `quew-errors` | Span arithmetic, Diagnostic builder chain | — |
| `quew-interner` | Intern/resolve round-trip, `InternedStr` copy semantics | Concurrent intern stress (8+ threads) |
| `quew-source` | SourceId uniqueness, byte-offset → line/col mapping | Multi-file SourceMap |
| `quew-lexer` | Every `TokenKind` variant, whitespace/comment skipping | Full `.quew` snippets, error recovery |
| `quew-ast` | Node construction, Span propagation | — (tested via parser) |
| `quew-parser` | Every grammar production, error recovery path | ~20 canonical `.quew` fixture programs |
| `quew-types` | `Ty` construction, equality, display | — |
| `quew-unify` | Unification of base types, union members, cycle detection | Inference scenario fixtures |
| `quew-scope` | Scope nesting, shadowing, use-before-define | AST → symbol table fixtures |
| `quew-resolve` | Import path resolution, cycle detection | Multi-file project fixtures |
| `quew-checker` | Every diagnostic code triggered at least once | ~20 semantic error fixture programs |
| `quew-ir` | Every node type in the graph | Parse → lower → graph structure assertions |
| `quew-codegen` | Each target module independently | Snapshot tests of generated `.d.ts`, `.pyi`, `.dart` |
| `quew-cli` | `--help` exit code, unknown flag handling | End-to-end compile of fixture `.quew` file |

---

## Dependency Decisions

All versions are pinned to the latest stable release at workspace creation (2026-05).
Never use pre-release versions (`alpha`, `beta`, `rc`) in any crate.

| Crate | Version | Used by | Rationale |
|-------|---------|---------|-----------|
| `logos` | `0.16.1` | `quew-lexer` | DFA lexer with macro-driven token definitions |
| `chumsky` | `0.13.0` | `quew-parser` | PEG combinator parser with best-in-class error recovery |
| `ariadne` | `0.6.0` | `quew-errors` | Beautiful terminal diagnostic rendering |
| `lasso` | `0.7.3` | `quew-interner` | String interning; `ThreadedRodeo` for parallel safety |
| `indexmap` | `2.14.0` | `quew-types`, `quew-scope`, `quew-resolve` | Deterministic insertion-ordered maps |
| `ena` | `0.14.4` | `quew-unify` | Union-find for type variable unification (rustc's impl) |
| `serde` + `serde_json` | `1.0` | `quew-ir`, `quew-cli` | IR serialization |
| `clap` | `4` | `quew-cli` | CLI argument parsing |
| `rayon` | `1` | `quew-resolve` | Parallel multi-file analysis |
| `insta` | `1` | all (dev) | Snapshot testing |

### Why `ThreadedRodeo` over `Rodeo`?

`Rodeo` uses `&mut self` to intern — every function in the lexer, parser, checker, and scope crates
that calls `interner.intern(s)` would need `&mut Interner`. Adding parallel compilation later would
require changing every one of those signatures.

`ThreadedRodeo` uses `&self` for both intern and resolve. We pay a DashMap overhead on writes, but
reads — which dominate during type checking — have zero contention. All signatures stay stable.

**Rule:** pass `Arc<Interner>` everywhere. Never pass `&mut Interner`.

---

## Crate Dependency Graph

Dependencies flow strictly downward through layers. No upward or sideways dependencies.

```
Layer 7   quew-cli
               │
Layer 6   quew-codegen      quew-ir
               │                 │
               └────────────┬────┘
                            │
Layer 5              quew-checker
                    /      │      \
Layer 4    quew-scope  quew-resolve  (quew-unify via checker)
              │              │
Layer 3    quew-types    quew-unify
              │
Layer 2    quew-ast ← quew-parser ← quew-lexer
              │
Layer 1    quew-source
              │
Layer 0    quew-errors    quew-interner
```

---

## Internal Layout of `quew-codegen`

The codegen crate emits type stubs for multiple target languages. Each target is an internal
module with its own source subdirectory and test fixtures:

```
quew-codegen/
  src/
    lib.rs           ← CodeWriter, CodegenTarget trait, public API
    ts/
      mod.rs         ← TypeScript .d.ts emitter
    python/
      mod.rs         ← Python .pyi emitter
    dart/
      mod.rs         ← Dart abstract class emitter
    rust/
      mod.rs         ← Rust trait + struct emitter
  tests/
    ts_codegen.rs    ← snapshot tests for TypeScript output
    python_codegen.rs
    dart_codegen.rs
    rust_codegen.rs
    fixtures/
      ts/            ← expected .d.ts output files
      python/        ← expected .pyi output files
      dart/
      rust/
```

---

## Naming Convention

| Concept | Format | Example |
|---------|--------|---------|
| Workspace | `quew-compiler` | — |
| Crate name | `quew-<noun>` | `quew-lexer` |
| Rust module path | `quew_<noun>::Type` | `quew_types::Ty` |
| DSL source file | `*.quew` | `my_agent.quew` |
| IR graph file (future) | `*.quew.ir` | `my_agent.quew.ir` |

---

## Getting Started

```bash
# Check all crates (fast, no codegen)
cargo check --workspace

# Run every test in the workspace
cargo test --workspace

# Run tests for a single crate
cargo test -p quew-interner

# Run the CLI
cargo run -p quew-cli -- --help

# Review snapshot changes after intentional output edits
cargo insta review
```

---

## Versioning

All crates start at `0.1.0` and are versioned independently. The workspace `Cargo.lock` governs
compatibility between crates. When a crate is published to crates.io, it must have:
- A `CHANGELOG.md` entry for the release
- All public items documented with `///` doc comments
- Zero `#[allow(dead_code)]` attributes

---

## Contributing

Read the design documents before changing architecture:
- `V2-milestone/disucssion/discussion1.md` — architecture decisions, interop, dual-tool model
- `RESUMABLE_GRAPH_IR_PROPOSAL.md` — execution graph IR and resumability semantics
