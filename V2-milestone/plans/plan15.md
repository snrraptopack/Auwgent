# Plan 15: Graph IR Runtime Executor — Value System and Deterministic Execution

**Status:** Completed. Derived from Discussion 9, Section 4–5.

**Scope:** First runtime crate. Phase 1 (Value system + expression evaluator) and Phase 2 (deterministic graph executor). No LLM, no host tools, no checkpoint store yet.

---

## Where We Are

Plan 14 cleaned up the compiler:
- `CheckResult::default()` exists
- `lower_expr` correctly maps function call arguments
- All 415+ tests pass

The compiler emits complete `QuewGraphIR` with graphs for agents, functions, and extension methods. But there is still **no runtime** to execute them.

## Goals

1. Create a `quew-runtime` crate in the `quew-compiler` workspace.
2. Define a `Value` enum that can represent every Quew type at runtime.
3. Implement `eval_expr()` — evaluates `IrExpr` into `Value` using a node output map.
4. Implement a deterministic graph executor that can run pure computation graphs:
   - `Input`, `Context`, `Output`
   - `LetBind`
   - `Branch` (with branch-taken recording)
   - No `FuncCall`, `HostToolCall`, `Reply`, or `AgentCall` yet.
5. Write integration tests that compile `.quew` sources and execute them through the runtime.

## Non-Goals

- Native function registry (`@@rust` dispatch) — deferred to Plan 16
- `FuncCall` node execution — deferred to Plan 16
- `HostToolCall` — deferred to Plan 17
- `AgentCall` — deferred to Plan 18
- `Reply` node / LLM integration — deferred to Plan 19
- Checkpoint store / serialization — deferred to Plan 20
- Async execution — this plan is sync-only for simplicity

## Architecture

### Crate Layout

A new crate `quew-runtime` in the `quew-compiler` workspace:

```
quew-compiler/crates/quew-runtime/
  Cargo.toml
  src/
    lib.rs
    value.rs
    eval.rs
    execution.rs
```

`quew-runtime` depends on `quew-ir` (for graph types) and `quew-interner` (for `InternedStr`).

### `Value` — Runtime Value Representation

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(i64),
    Float(f64),
    Bool(bool),
    Null,
    Object(IndexMap<String, Value>),
    Array(Vec<Value>),
}
```

Operations:
- `Display` for debugging
- `type_name()` → `"string"`, `"number"`, etc.
- `as_str()`, `as_number()`, `as_bool()` — typed accessors that return `Option`
- Binary operator dispatch (`Value::add`, `Value::eq`, etc.)

### `eval_expr()` — Pure Expression Evaluator

```rust
pub fn eval_expr(
    expr: &IrExpr,
    outputs: &HashMap<NodeId, Value>,
) -> Result<Value, EvalError>
```

Evaluates every `IrExpr` variant by looking up `DataRef` values in the `outputs` map.

### `Execution` — Deterministic Graph Runner

```rust
pub struct Execution<'a> {
    pub ir: &'a QuewGraphIR,
    pub outputs: HashMap<NodeId, Value>,
    pub branch_taken: HashMap<NodeId, NodeId>, // branch node -> chosen then/else node
}

impl Execution<'_> {
    pub fn run(&mut self, graph_id: &str, input: Value) -> Result<Value, ExecutionError> {
        // 1. Seed Input node
        // 2. Topologically walk nodes
        // 3. Execute each node kind
        // 4. Return Output value
    }
}
```

Node execution:

| NodeKind | Execution |
|----------|-----------|
| `Input` | Use the provided input value |
| `Context` | `Value::Null` for now (no context injection yet) |
| `Output` | Look up `DataRef` in `outputs`, return it |
| `LetBind` | `eval_expr(value, &outputs)`, store result |
| `Branch` | Evaluate condition `DataRef` → bool, record taken branch, continue to that subgraph |

**Graph walking:** Since `AgentGraph` nodes are in an `IndexMap` in insertion order (which is topologically valid), a simple forward iteration works for deterministic graphs. Branch nodes require following the chosen subgraph and then returning.

For this plan, we handle branching by:
1. Evaluating the condition
2. Recording which branch was taken
3. Executing only the statements in the taken branch
4. Continuing after the `if` block

Since the graph is flat (not a tree), `Branch` nodes use `then_node` and `else_node` as **jumps**. The executor maintains a program counter (current node index) and can skip over untaken branch bodies.

### Handling Branch Nodes

The graph lowerer (`graph_lower.rs`) already numbers nodes sequentially. `Branch` stores `then_node` and `else_node` as `NodeId`s. The executor:

1. Evaluates the condition
2. If true, jumps to `then_node`
3. If false, jumps to `else_node` (or skips past the branch if no else)
4. After the branch body completes, execution continues after the branch

The challenge: the flat node list doesn't explicitly mark "end of branch body." For this plan, we use a simple heuristic: when executing a branch body, we run nodes until we hit a node that was not part of the branch's reachable set, then return to the caller.

A simpler approach for Phase 2: since branch bodies are contiguous in the node list (the lowerer emits them in order), we can estimate body length. But that's fragile.

**Recommended approach:** Don't try to be clever. Build a **reverse adjacency map** from edges, then use a recursive `execute_subgraph(start_node, end_boundary)` function that runs nodes reachable from `start_node` until it would execute a node at or after `end_boundary`.

Even simpler: treat the graph as a DAG and use **topological execution with a ready set**. A node is ready when all its predecessors are done. For `Branch`, after evaluating the condition, we mark the untaken branch's entry node as `skipped` so its downstream nodes never become ready.

This is the cleanest approach. Let's use it.

## Implementation Steps

### Step 1: Create `quew-runtime` crate

- Add `crates/quew-runtime/Cargo.toml`
- Depend on `quew-ir`, `quew-interner`, `indexmap`
- Add to workspace `Cargo.toml`

### Step 2: Implement `Value`

- `value.rs`: `Value` enum + `Display` + typed accessors + binary ops
- Unit tests for each operation

### Step 3: Implement `eval_expr`

- `eval.rs`: `eval_expr()` function
- Handle all `IrExpr` variants
- Handle `DataRef` resolution (look up `NodeId` → `Value`, optionally field select)
- Unit tests with mock `outputs` map

### Step 4: Implement deterministic `Execution`

- `execution.rs`: `Execution::run()` for pure graphs
- Forward walk through nodes
- Handle `Input`, `Context`, `Output`, `LetBind`, `Branch`
- For `Branch`: evaluate condition, mark taken path, skip untaken path

### Step 5: Integration tests

Compile `.quew` sources through the full pipeline and execute them:

```rust
#[test]
fn executes_pure_computation_graph() {
    let (interner, ir) = compile_source(r#"
function double(x: number): number { return x + x }
agent Main(input: number) {
    let result = double(input)
}
"#);

    // For this test, we only execute the function graph
    let mut exec = Execution::new(&ir);
    let result = exec.run("function:double", Value::Number(5)).unwrap();
    assert_eq!(result, Value::Number(10));
}
```

More test cases:
- Literal return
- Binary arithmetic
- Branch (if/else)
- Nested let bindings
- Extension method call (if we can execute it deterministically)

## Test Plan

1. **Unit tests:**
   - `Value` operations
   - `eval_expr` for each `IrExpr` variant
   - `Execution::run` on hand-built graphs

2. **Integration tests:**
   - Compile `.quew` → execute function graph → assert output
   - Branching computation
   - Multiple let bindings

3. **Existing tests:** `cargo test` in the full workspace must still pass.

## Acceptance Criteria

- [ ] `quew-runtime` crate exists and compiles
- [ ] `Value` enum supports all Quew primitive types + Object + Array
- [ ] `eval_expr` evaluates every `IrExpr` variant correctly
- [ ] `Execution::run` can execute a pure function graph end-to-end
- [ ] Branch nodes correctly route execution and skip untaken arms
- [ ] Integration tests compile `.quew` sources and run them
- [ ] All existing compiler tests still pass

## Open Questions

1. **Context injection:** For this plan, `Context` nodes return `Value::Null`. When do we add real context support?
2. **Error model:** Should `ExecutionError` carry node IDs for debugging?
3. **Graph call recursion:** `FuncCall` is out of scope, but the architecture should not prevent adding it later.
