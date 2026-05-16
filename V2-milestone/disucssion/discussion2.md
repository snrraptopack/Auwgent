# Discussion 2: Plans 1–4 Retrospective — What We Have, What We Can Do, What Comes Next

*Written before entering Plan 5 (IR Lowering).*

---

## The Journey: Plans 1 → 4

### Plan 1 — Foundation Thinking

The seed of everything. The goal was deliberate: **do not repeat v1 mistakes**.  
v1 was fast to start and slow to finish because the language did not own its execution —  
it leaned on external runtimes and made interoperability an afterthought.

The commitments made here:

- **The language should own its own execution layer** — not just emit to other runtimes but define how agentic logic runs.
- **The type system comes first** — more testing than the actual code that implements the language.
- **Small, iterate, improve** — prove a fraction works before building the rest.

Nothing was written yet. But this plan shaped every decision that followed.

---

### Plan 2 — Lexer and Token System (`quew-lexer`)

The lexer is the contract between source text and the rest of the compiler.  
Every downstream crate — parser, checker, LSP, IDE extension — depends on the token set being stable.

**What was built:**

| Item | Detail |
|---|---|
| `TokenKind` | Complete terminal symbol set for the v2 grammar, including all keywords, operators, annotation tokens (`@tool`, `@desc`, `@context`, `@native`, `@block`), provider keywords (`gemini`, `openai`, `groq`), and DSL-specific symbols |
| `LexResult` | Flat `Vec<(TokenKind, Span)>` — zero-copy, span-preserving, recoverable |
| `lex()` | logos-based tokenizer; emits `TokenKind::Error` on unknown chars instead of panicking |
| Fixtures | 4 valid `.quew` fixtures, 2 invalid fixtures for lexer integration tests |
| Tests | **73 tests, 0 failures** |

**Design decision worth noting:** keywords like `model`, `tools`, `prompt` are tokenised as keyword variants, not identifiers. This means they cannot appear as variable names — a deliberate constraint that forces cleaner code. The parser compensates by providing a `field_name()` helper that accepts keywords as object key names in `with { }` blocks and member expressions.

---

### Plan 3 — AST Definitions and Parser (`quew-ast`, `quew-parser`)

The AST is the shape of the language. The parser is the proof that the shape is parseable.

**`quew-ast` — pure data, no semantics:**

Every node carries a `Span`. Every name is an `InternedStr`. No resolved types, no symbols — just structure.

Nodes implemented:

- `Module { items }` — root of the AST
- `Item` — `Agent`, `Function`, `Tool`, `Tools`, `Type`, `Model`, `Let`
- `AgentDecl` — annotations, one input param, optional return type, body
- `FunctionDecl` — annotations (`@tool`, `@desc`), params (including `@bound` refs), optional return type, body
- `Param` — `ParamBinding::Normal` vs `ParamBinding::BoundRef` for `@id: string` syntax
- `AnnotationArgs` — `None`, `Params(Vec<Param>)`, `Type(TypeExpr)`, `String(StringLit)`
- `Stmt` — `Let`, `If`, `Return`, `Reply`, `For`, `Expr`
- `ReplyStmt` — `input: Expr`, `with_block: WithBlock`
- `WithBlock / WithField` — `key: value` pairs; keys accept keyword tokens
- `Expr` — `Lit`, `Ident`, `Binary`, `Unary`, `Call`, `Provider`, `Member`, `Array`, `PostfixIf`, `Is`, `Error`
- `ProviderCall` — `Provider { Gemini, OpenAi, Groq }`, `model_name: StringLit`, `config: Vec<ConfigField>`
- `TypeExpr` — `Named`, `Optional`, `Union`, `Generic`

**`quew-parser` — chumsky 0.13, error-recovering:**

The parser uses `recursive()` for mutual recursion (statements reference expressions, `if` references statements). Error recovery uses `via_parser()` — on a bad statement it consumes to end of line and emits `Expr::Error`, letting parsing continue.

Notation-sensitive decisions:
- `reply(input) with { ... }` — the `with` block accepts both newline and comma as field separators
- `[a, b] if cond else [c]` — postfix-if is a valid expression
- `return expr` — `with turns` is **deliberately not parsed** (discussed below)
- Annotations stack: `@tool(id: string)` followed by `@desc "..."` followed by `function`

Tests: **24 integration tests, 0 failures** — plus the 46 AST unit tests.

---

### Plan 4 — Type System, Scoping, and Semantic Checking (`quew-types`, `quew-scope`, `quew-unify`, `quew-checker`)

This was the largest plan. It built the semantic layer on top of the syntactic layer.

---

#### `quew-types` — the type algebra

The `Ty` enum owns every type shape in the language.  
It deliberately has **no dependency on `quew-ast`** — types live in their own world.

```
Ty::Primitive(PrimTy)          — string, number, float, bool, null, void
Ty::Record(IndexMap<K, Ty>)    — named field map, insertion-order preserved
Ty::Array(Box<Ty>)             — homogeneous arrays
Ty::Union(Vec<Ty>)             — A | B | C, with flatten_union()
Ty::Optional(Box<Ty>)          — sugar for Ty | null
Ty::Function(FunctionTy)       — params + return
Ty::Tool(ToolTy)               — bound_params + model_params + return
Ty::Agent(AgentTy)             — input + return
Ty::Provider(ProviderKind)     — gemini | openai | groq
Ty::Error                      — sentinel; propagates without cascading
```

Key distinction: `Ty::Tool` carries **two parameter lists**:
- `bound_params` — what the **model sees** (declared in `@tool(id: string)`)
- `model_params` — **host-binding params** that must be pre-bound by the agent (regular function params of a `@tool`-decorated function)

This distinction is what makes the tool-gating pattern work semantically.

---

#### `quew-scope` — symbol table construction

`build_symbol_table(module)` walks the AST and registers every top-level declaration.  
It detects duplicate names across all namespaces (agents, functions, tools, types, models share one flat namespace — a deliberate choice to keep naming obvious).

The scope builder lowers:
- `@tool` annotations into `ToolTy.bound_params`
- Regular function params of `@tool`-decorated functions into `ToolTy.model_params`
- Type declarations into `Ty::Record` field maps
- Model declarations into `Ty::Provider`
- Agent declarations into `Ty::Agent`

---

#### `quew-unify` — type unification

A thin wrapper around `ena` (the union-find crate used by rustc).  
`UnifyTable::unify(a, b)` checks structural compatibility and returns `Ok(())` or an error message.  
It is deliberately simple — no inference variables, no generics, no polymorphism.  
The current phase only needs to verify declared types match actual types at `return` statements.

---

#### `quew-checker` — semantic orchestration

The checker is the most important crate in the frontend. It threads everything together.

**What the checker does today:**

| Pass | What it checks |
|---|---|
| Symbol collision | No two top-level declarations share a name (agent vs function vs type vs model) |
| Param registration | All function/agent params are registered with their **actual declared types** (not `Ty::Error`) |
| `@context` injection | `ctx` is injected into agent scope with the context type's record shape |
| `Expr::Provider` typing | `gemini(...)`, `openai(...)`, `groq(...)` correctly type as `Ty::Provider`, not `Ty::void()` |
| Primitive resolution | `string`, `number`, `bool`, `float`, `null`, `void`, `Text` resolved correctly via `PrimKeys` |
| Lexical scoping | Stack-based `LocalScope`; `let` bindings tracked per block; inner blocks can shadow outer |
| Duplicate `let` | Duplicate binding name within the same block → error |
| Duplicate params | Duplicate param name within the same function/agent → error |
| Unreachable code | Any statement after `return` in the same block → error |
| Return type checking | `return expr` type vs declared return type → unify; mismatch → error |
| `Expr::Member` inference | `obj.field` looks up field in `Ty::Record` — correctly types `ctx.isAdmin` as `bool` |
| `reply with` — `model` / `fallback` | Must be `Ty::Provider` — string/number/bool → error with help message |
| `reply with` — `prompt` | Must be `Ty::Primitive(String)` |
| `reply with` — `retry` / `maxTurn` | Must be numeric |
| `reply with` — `tools` | Must be an array; each element validated |
| `tools` array — bare ident | Must resolve to `SymbolKind::Tool` or `ToolGroup` |
| `tools` array — host params | Tool with non-empty `model_params` used bare → error; must pre-bind with call |
| `tools` array — call form | `myTool(arg)` → validates arg count matches `model_params.len()` |
| `tools` array — non-tools | Plain function, literal, etc. → error |
| `tools` — local vars | `let selected = [...]` then `tools: selected` → trusted (dynamic list) |
| `@tool` bound-param contract | `@id: string` in param list must exist in `@tool(...)` annotation |
| For loop var scoping | `for item in list` registers `item` in the loop body scope |

**Test coverage: 83 tests, 0 failures across three test targets.**

---

## What We Can Do Today (DX View)

If you were writing a `.quew` program today, the compiler would:

1. **Lex your source** — every keyword, literal, annotation, and operator tokenised with spans
2. **Parse it** — produce a full AST with error recovery; bad statements don't abort the whole file
3. **Check it semantically** — you get clear errors for:
   - Using the wrong type for `model:` (`"gemini-pro"` is a string, not a model → error with a help hint)
   - Putting a function in `tools: [myFunction]` when it has no `@tool` annotation
   - Using `delete_person` bare in tools when it requires `isAdmin` to be pre-bound
   - Duplicate names at any level — top-level, params, or let bindings
   - Accessing `ctx.isAdmin` correctly with the right type when `@context(Context)` is declared
   - Returning the wrong type from a typed function
   - Dead code after `return`

What it does **not** yet do:
- Emit IR — that is Plan 5
- Cross-file resolution — no `import` syntax yet
- Full control-flow analysis — the "all paths return" guarantee is deferred to IR
- Generics

---

## What Was Deliberately Left Out

### `return Agent(input) with turns`

This syntax appears in `not.txt` to describe a **context pipeline handoff** — where an agent delegates to a sub-agent and the caller's session turns are passed along:

```quew
agent Main(input: Text) {
    if inputType.data.includes("high") {
        return One(input) with turns   // exits Main, output is One's output, turns carried over
    }
    return Two(input) with turns
}
```

This was **deliberately not implemented** in Plans 3 or 4 for a good reason:

`with turns` is not a type-level concept — it is a **runtime execution directive**. It tells the engine how to thread session state between agent invocations. The right place to implement it is in the **IR** (Plan 5), not the parser or checker.

Adding `ReturnMode::WithTurns` to the AST now would be premature — we would need the IR shape to know exactly what data it needs to carry. Implementing it in Plan 5 keeps the concerns clean:

- **Parser (Plan 3):** `return expr` — structural shape
- **Checker (Plan 4):** return type matches declared type
- **IR (Plan 5):** `return expr with turns` — lowers to a `Handoff { target, carry_turns: true }` IR node

The parser currently recovers gracefully if `with turns` appears (it would absorb `with` as part of the statement error recovery), so existing programs will not crash the compiler — they will just not emit the handoff flag until Plan 5 wires it in.

---

## Before Plan 5 — The State of the Codebase

```
quew-errors       — Span, Diagnostic, Severity              (tested)
quew-interner     — ThreadedRodeo, InternedStr               (tested)
quew-source       — SourceId, SourceFile, SourceMap          (tested)
quew-lexer        — TokenKind, lex(), LexResult              (73 tests)
quew-ast          — Full AST, every node with Span           (46 tests)
quew-parser       — Full grammar, error recovery             (24 tests)
quew-types        — Ty enum, ToolTy, AgentTy, ProviderKind  (tested)
quew-scope        — SymbolTable, build_symbol_table()        (tested)
quew-unify        — UnifyTable, structural unification       (tested)
quew-checker      — Full semantic pass                       (83 tests)
```

**Total: 9 production crates, all tested, all passing.**

The frontend is complete. We can take source text in, lex it, parse it, and fully validate its semantics. We know the types. We know the names. We know what is wrong and where.

Plan 5 takes this validated, typed AST and translates it into the IR — the structured representation the runtime will consume.

---

## Plan 5 Preview

The IR lowering pass (`quew-ir`) will:

1. Consume the `CheckResult` (symbol table + typed AST)
2. Emit `AgentIR` — a JSON-serialisable representation of every agent, its tools, its reply configuration, and its execution mode
3. Handle the `reply(...) with { ... }` block as a first-class construct — lowering `model`, `tools`, `prompt`, `fallback`, `retry`, `maxTurn` into IR fields
4. Introduce `Handoff` nodes for agent-to-agent delegation (including `with turns` when the parser supports it)
5. Wire into `quew-cli` so `quew compile file.quew` produces a `.quew.json` IR file

The runtime then loads the IR and runs it — same as the Auwgent engine today, but now driven by the quew compiler frontend.
