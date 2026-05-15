# Auwgent v2 Open Decisions

Status: decision tracker  
Source notes: `not.txt`, `not_standard.txt`, `not_graph.txt`

This document lists the design areas that are not fully settled yet. It is intended to keep the v2 compiler work grounded while leaving room for deliberate choices.

## 1. Context Access

Question:

```text
Should context fields be directly available, or accessed through ctx?
```

Option A:

```ts
if isAdmin {
    ...
}
```

Pros:

- Shorter.
- Nice for prompt/config expressions.
- Feels lightweight.

Cons:

- Local variable collisions.
- Harder to reason about scope.
- More checker rules.

Option B:

```ts
if ctx.isAdmin {
    ...
}
```

Pros:

- Clear source of data.
- Avoids name collisions.
- Easier for compiler and users.

Cons:

- More verbose.

Current leaning:

```text
Direct fields are convenient, but ctx.field is cleaner for v2 stability.
```

If direct fields are kept, the compiler must reject any local variable that collides with context.

## 2. Tool Function Partial Binding

Question:

```text
How should a DSL tool function expose model-provided args versus runtime-bound args?
```

Problem:

```ts
@tool
function delete_person(id: string, isAdmin: bool): string {
    ...
}

tools = [delete_person(user_id, isAdmin)]
```

There are two arg classes:

- args the model should provide,
- args the runtime/context should bind deterministically.

Possible design:

```ts
@tool(args = { id })
function delete_person(id: string, isAdmin: bool): string {
    ...
}

tools = [delete_person(isAdmin = ctx.isAdmin)]
```

Or:

```ts
tools = [
    delete_person with {
        modelArgs = [id]
        boundArgs = { isAdmin = ctx.isAdmin }
    }
]
```

Current leaning:

```text
Do not overload normal function calls too much.
Make model args vs bound args explicit.
```

## 3. Tool Groups

Question:

```text
Is a tool group only a namespace, or does it have progressive disclosure behavior?
```

Current leaning:

```text
Tool groups should be progressive disclosure units.
```

Example:

```ts
tools userTools {
    getUsername(): string
    getUserLocation(): string
} @desc "User profile tools"
```

IR should preserve:

```json
{
  "kind": "group",
  "disclosure": "lazy",
  "members": ["getUsername", "getUserLocation"]
}
```

Still deciding:

- internal discovery tool names,
- whether model can reveal one member or whole group,
- whether disclosure state is per reply turn or whole run.

## 4. Agents List Syntax

Question:

```text
Should model-callable agents use arrays or braces?
```

Option A:

```ts
agents = [One, Two]
```

Option B:

```ts
agents = { One, Two }
```

Arrays are consistent with `tools = [...]`. Braces may visually signal a callable set.

Current leaning:

```text
Use arrays unless there is a strong semantic reason for braces.
```

## 5. Output Defaults

Question:

```text
Does Image output implicitly include Text?
```

Option A:

```ts
agent Draw(input: Text): Image
```

means:

```text
Image only
```

Option B:

```ts
agent Draw(input: Text): Image
```

means:

```text
Text | Image
```

Current leaning:

```text
Avoid hidden unions.
```

Recommended rule:

```text
No output annotation -> Text
: Image -> Image only
: Text | Image -> Text or Image
```

## 6. Middleware Representation

Question:

```text
Should DSL middleware lower into graph nodes, AST, or bytecode?
```

Option A: graph nodes.

Pros:

- Unified execution representation.
- Every middleware step can be checkpointed.

Cons:

- Noisy.
- Overkill for policy code.
- Makes simple middleware look like workflow execution.

Option B: interpreted AST.

Pros:

- Easy first compiler target.
- Easy to debug.
- Lightweight.

Cons:

- Slower than bytecode.
- Needs interpreter.

Option C: bytecode.

Pros:

- Fast.
- Compact.
- Good sandboxing.

Cons:

- More upfront compiler/runtime work.

Current leaning:

```text
Start with middleware AST. Move to bytecode after syntax stabilizes.
```

## 7. Middleware And Host Tools

Question:

```text
Can DSL middleware call tools?
```

Use case:

```ts
@middleware("session-db")
function SessionDb(event: MiddlewareEvent) {
    if event.on == "runStart" {
        let loaded = getDatabase(ctx.id)
        event.setSession(loaded.data)
    }
}
```

Options:

- middleware can call any registered tool,
- middleware can call only explicitly granted tools,
- middleware has a separate `middlewareTools` list,
- middleware cannot call tools and must use host middleware for external effects.

Current leaning:

```text
Allow tool calls only through explicit grants.
```

Reason:

- avoids accidental model/runtime capability leakage,
- keeps middleware permissions auditable,
- fits managed hosted execution.

## 8. Standard Library Permissions

Question:

```text
Where do stdlib permissions live?
```

Example:

```ts
fetch<string>("https://api.example.com/users")
```

Potential controls:

- DSL declares allowed domains,
- host config grants allowed domains,
- both required.

Current leaning:

```text
DSL declares intent. Host config grants permission.
```

## 9. fetch Type Hint

Question:

```text
Does fetch<T> parse JSON automatically?
```

Option A:

```ts
let response = fetch<User>("...")
let user = response.data
```

Option B:

```ts
let raw = fetch<string>("...")
let user = JSON.parse<User>(raw.data)
```

Current leaning:

```text
fetch<T> should parse JSON when T is not string, but this needs clear error behavior.
```

## 10. Checkpoint Granularity

Question:

```text
Should pure expression outputs be saved?
```

Option A:

```text
Only checkpoint external/non-deterministic nodes.
```

Option B:

```text
Store all node outputs.
```

Current leaning:

```text
Checkpoint external and non-deterministic nodes by default.
Allow debug mode to store more.
```

## 11. Transparent Agent Calls

Question:

```text
What exactly does with turns mean?
```

Possible meanings:

- inline child turns into parent session,
- attach child trace to parent run,
- inline child graph into parent graph,
- expose child state through a cursor.

Current leaning:

```text
Attach child trace by default.
Do not mutate or inline the compiled parent graph.
```

## 12. Host Middleware Ordering

Question:

```text
Does DSL middleware run before or after host middleware?
```

Current leaning:

```text
DSL middleware first, host middleware second.
```

Reason:

- portable policy runs locally,
- host infra sees the effective event,
- host can still override if necessary.

Still deciding:

- whether host receives original and effective events,
- whether ordering can be configured.

## 13. Compiler Milestone Scope

Question:

```text
What should the first v2 compiler support?
```

Recommended minimum:

- type declarations,
- model declarations,
- host tool declarations,
- functions,
- agents,
- `reply(...) with { ... }`,
- `let`,
- `if`,
- function calls,
- agent calls,
- basic graph IR output.

Defer:

- middleware bytecode,
- progressive tool disclosure runtime,
- media output,
- provider built-ins,
- full stdlib,
- advanced permission model.

## 14. Paid Tier Boundary

Question:

```text
What is open/core versus paid?
```

Recommended boundary:

Open/core:

- v2 compiler,
- graph IR,
- local executor,
- local execution state,
- local resume,
- SDK bindings,
- custom checkpoint store interface.

Paid/hosted:

- managed durable checkpoints,
- cloud resume,
- run dashboard,
- replay timeline,
- audit logs,
- hosted queues,
- team/project history,
- managed secrets.

Current leaning:

```text
Do not make graph IR itself paid.
Make managed resumability paid.
```
