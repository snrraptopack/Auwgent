# Plan 13: Checker-to-IR Type Bridge and Extension Method Graph Lowering

**Status: Completed.**

Plan 13 fixes a critical compiler gap: the checker resolves extension method calls and types, but the IR lowerer receives none of that information. This causes a panic on any `.quew` source that uses extension methods. Additionally, function and extension method bodies are not being lowered into graphs, making them non-executable even if the panic were fixed.

---

## Where We Are

Plans 1–12 built a complete compiler frontend:

- **Lexer, parser, AST** — stable
- **Checker** — resolves types, extension methods, generics, roles
- **IR types and agent lowering** — `AgentGraph` with nodes, edges, checkpoint policies
- **414 tests, 0 failures**

**What works:**
- `agent` bodies lower into `AgentGraph`
- `reply`, `let`, `if/else`, `return`, function calls lower correctly
- Extension methods parse, scope, and type-check correctly

**What is broken:**
- `value.isEmpty()` (extension method call) **panics** in `quew-ir/src/lower/expr.rs:37`
- Function bodies are **not lowered into graphs** — `lower_function()` ignores the `_graphs` parameter
- Extension method bodies are **not lowered into graphs** — `lower_extend()` has no graph output
- The checker discards **all resolved type information** before the lowerer sees the AST

---

## The Bug

### Reproduction

File `quew-compiler/test.quew`:

```quew
function custom_string_is_empty(value: string): bool {
    return true
}

extend string {
    function isSuperEmpty(): bool {
        return custom_string_is_empty(self)
    }
}

agent Main(input: string) {
    let result = input.isSuperEmpty()
}
```

Command: `cargo run --bin quew -- compile test.quew`

Panic:
```
thread 'main' panicked at crates\quew-ir\src\lower\expr.rs:37:22:
lowering bug: non-identifier call callee in pure expression
```

### Root Cause

In `quew-checker/src/lib.rs`, `infer_extension_method_call()` resolves:

```rust
let method = table.extension_methods.iter().find(|method| {
    method.name == member.field
        && receiver.is_assignable_to(&resolve_semantic_ty(...))
})?;
```

It finds the match, validates arguments, and returns the method's return type. **Then it discards the resolution.**

In `quew-ir/src/lower/expr.rs`, `lower_expr()` hits `Expr::Call`:

```rust
Expr::Call(call) => {
    let function = match call.callee.as_ref() {
        Expr::Ident(ident) => ident.name,
        _ => panic!("lowering bug: non-identifier call callee in pure expression"),
    };
    ...
}
```

The callee is `Expr::Member { object: input, field: isSuperEmpty }`. The lowerer has no type context, so it cannot re-resolve the extension method. It panics.

---

## Part A: Resolved Expression Sidecar (Checker → Lowerer)

### Problem

`CheckResult` only carries `symbol_table` and `diagnostics`. All per-expression resolution is lost.

### Solution

Add a `resolved_expressions` sidecar map to `CheckResult`:

```rust
pub struct CheckResult {
    pub symbol_table: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
    /// Maps expression spans to their checker-resolved meanings.
    /// Populated during type inference. Consumed by the IR lowerer.
    pub resolved: ResolvedExpressionMap,
}

pub struct ResolvedExpressionMap {
    /// Key: the span of the expression node.
    /// Value: what the checker determined this expression refers to.
    calls: HashMap<Span, ResolvedCall>,
}

pub struct ResolvedCall {
    /// The kind of call the checker resolved this to.
    pub kind: CallKind,
    /// The resolved target name (function, extension graph_ref, tool, agent).
    pub target: InternedStr,
    /// For extension methods: the receiver expression's inferred type.
    pub receiver_ty: Option<Ty>,
}

pub enum CallKind {
    Function,
    ExtensionMethod,
    Tool,
    Agent,
}
```

### Why `Span` as the key

Every AST expression carries a `Span { start, end }`. These are byte offsets into the source text. Two different expressions in the same source will always have different spans (unless they are literally the same bytes, which is impossible for distinct AST nodes). `Span` already derives `Hash + Eq + Copy`.

### Population

In `infer_extension_method_call()`, when a match is found:

```rust
let method = table.extension_methods.iter().find(...)?;

// Record the resolution for the lowerer.
resolved.calls.insert(
    call_span,
    ResolvedCall {
        kind: CallKind::ExtensionMethod,
        target: method.name, // or a generated graph_ref
        receiver_ty: Some(receiver),
    },
);
```

Similarly, `infer_expr()` records `Expr::Call` resolutions for regular functions, tools, and agents.

### Consumption

In `lower_expr()`, when encountering `Expr::Call` with a non-`Ident` callee:

```rust
Expr::Call(call) => {
    match call.callee.as_ref() {
        Expr::Ident(ident) => {
            // existing path
        }
        other => {
            // Look up checker resolution.
            if let Some(resolved) = check.resolved.calls.get(&call.span) {
                match resolved.kind {
                    CallKind::ExtensionMethod => {
                        // Lower as extension method call.
                    }
                    _ => { /* other non-ident callees */ }
                }
            } else {
                panic!("lowering bug: unresolved non-identifier call callee");
            }
        }
    }
}
```

---

## Part B: Lower Function Bodies into Graphs

### Current State

`lower_function()` in `defs.rs` takes `_graphs` but ignores it:

```rust
fn lower_function(
    decl: &FunctionDecl,
    interner: &Arc<Interner>,
    defs: &mut Definitions,
    _graphs: &mut IndexMap<String, AgentGraph>,
) {
    let graph_ref = format!("function:{}", interner.resolve(decl.name));
    // ... inserts into defs.functions or defs.tools
    // Never emits a graph.
}
```

Only `agent` bodies get graphs in `lower.rs`:

```rust
for item in &module.items {
    if let quew_ast::Item::Agent(agent) = item {
        let graph = graph_lower::lower_agent(agent, check, interner, &mut definitions);
        graphs.insert(graph_key, graph);
    }
}
```

### Fix

1. Extract a generic `lower_function_body()` from `lower_agent()` or refactor `graph_lower.rs` to accept both `AgentDecl` and `FunctionDecl`.

2. In `lower.rs`, after `lower_definitions()`, iterate over all `Item::Function` and `Item::Extend` and lower their bodies into graphs:

```rust
// After agent lowering:
for item in &module.items {
    if let quew_ast::Item::Function(func) = item {
        let graph_ref = format!("function:{}", interner.resolve(func.name));
        let graph = graph_lower::lower_function(func, check, interner, &mut definitions);
        graphs.insert(graph_ref, graph);
    }
    if let quew_ast::Item::Extend(ext) = item {
        for method in &ext.methods {
            let graph_ref = format!(
                "extension:{}:{}",
                type_ref_name(&lowered_receiver, interner),
                interner.resolve(method.name)
            );
            let graph = graph_lower::lower_function(method, check, interner, &mut definitions);
            graphs.insert(graph_ref, graph);
        }
    }
}
```

3. Update `graph_lower.rs` to handle `FunctionDecl` bodies. The main difference from `AgentDecl`:
   - Functions have zero or more params (not exactly one)
   - No implicit `Input` node — params bind directly to `ctx.slots`
   - No `@context` injection
   - Return type defaults to `void` if not declared

---

## Part C: Lower Extension Method Calls

### In `lower_value_node()`

Extension method calls should become `FuncCall` graph nodes (deterministic, checkpoint optional):

```rust
Expr::Call(call) => {
    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(resolved) = check.resolved.calls.get(&call.span) {
            if resolved.kind == CallKind::ExtensionMethod {
                let graph_ref = format!("extension:{}:{}", ...);
                let mut args = IndexMap::new();
                args.insert(interner.intern("self"), ensure_ref(&member.object, check, builder));
                for (idx, arg) in call.args.iter().enumerate() {
                    args.insert(param_name(idx), ensure_ref(arg, check, builder));
                }
                return (
                    NodeKind::FuncCall {
                        function: interner.intern(&graph_ref),
                        args,
                    },
                    CheckpointPolicy::Optional,
                    args,
                );
            }
        }
    }
    // existing ident callee path...
}
```

### In `lower_expr()`

When an extension method call appears inside a pure expression (e.g., inside a `let` bind or binary op), it needs to lower to an `IrExpr::Call` referencing the extension method graph:

```rust
Expr::Call(call) => {
    match call.callee.as_ref() {
        Expr::Ident(ident) => IrExpr::Call { function: ident.name, args: ... },
        _ => {
            if let Some(resolved) = check.resolved.calls.get(&call.span) {
                match resolved.kind {
                    CallKind::ExtensionMethod => {
                        let mut args = IndexMap::new();
                        // Add self from member.object
                        // Add explicit args
                        IrExpr::Call { function: resolved.target, args }
                    }
                    _ => panic!("..."),
                }
            } else {
                panic!("lowering bug: unresolved non-identifier call callee");
            }
        }
    }
}
```

---

## Part D: Update `CheckResult` Constructors

Every place that constructs `CheckResult` must initialize the new `resolved` field:

- `check()` in `quew-checker/src/lib.rs`
- `check_with_prelude()` in `quew-checker/src/lib.rs`
- Any test helpers that build `CheckResult` manually

---

## Testing Strategy

### Unit tests for the sidecar

- `infer_expr` on `value.isEmpty()` populates `resolved.calls` with `CallKind::ExtensionMethod`
- `infer_expr` on `foo()` populates `resolved.calls` with `CallKind::Function`
- Duplicate spans do not overwrite (each expression has a unique span)

### Integration tests for lowering

- `.quew` source with extension method call → lower → assert no panic
- Assert the lowered graph contains a `FuncCall` node with the correct `extension:receiver:method` function name
- Assert `graphs` contains an `extension:string:isEmpty` graph
- Assert `value.isEmpty()` inside a `let` bind lowers correctly
- Assert `value.isEmpty()` as a standalone statement lowers correctly

### Equivalence tests

```quew
function direct(value: string): bool { return custom_string_is_empty(value) }
extend string { function indirect(): bool { return custom_string_is_empty(self) } }
```

`direct("hello")` and `"hello".indirect()` should lower to graphs with equivalent structure.

---

## Definition of Done

- [x] `CheckResult` carries `ResolvedExpressionMap`
- [x] Checker populates resolved call info for extension methods, functions, tools, agents
- [x] `cargo run -- compile test.quew` no longer panics
- [x] Extension method bodies are lowered into graphs under `extension:receiver:method` keys
- [x] Regular function bodies are lowered into graphs under `function:name` keys
- [x] Extension method calls lower into `FuncCall` nodes
- [x] All existing tests still pass
- [x] New tests cover extension method graph lowering and call lowering
- [x] `test.quew` is deleted once the bug is fixed

---

## Deferred After Plan 13

- Provider calls (`gemini(...)`) as `Expr::Call` with non-`Ident` callee — currently handled specially, could use the sidecar
- More resolved expression kinds (member access field resolution, array element types)
- Runtime executor that actually walks these graphs

---

*Date: 2026-05-19*
