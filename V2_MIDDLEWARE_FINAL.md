# Auwgent v2 Middleware

Status: design reference
Source notes: `not.txt`, current runtime review

Middleware in v2 should become portable, runtime-owned policy logic while still allowing host-language middleware for external integrations.

The key design goal:

```text
Use DSL middleware for runtime-owned decisions.
Use host middleware for external systems.
Avoid crossing the FFI boundary for simple policy.
```

## 1. Current v1 Middleware Model

In v1, middleware talks to the Rust engine through a JSON bridge.

The Rust engine stores one middleware callback:

```rust
middleware_event_handler: Arc<Mutex<Option<AsyncMiddlewareEventCallback>>>
```

The bridge exposes:

```rust
on_middleware_event(handler)
```

The flow is:

```text
Rust engine
  -> typed middleware event
  -> JSON string
  -> host SDK callback
  -> optional JSON string response
  -> Rust parses response
  -> Rust mutates runtime state
```

This works across TypeScript, Python, Dart, Rust, Node, and WASM, but every middleware decision requires a Rust-host-Rust roundtrip.

## 2. v2 Middleware Goal

V2 should support middleware written in the DSL:

```ts
@middleware("logger")
function Logger(event: MiddlewareEvent) {
    if event.on == "llmStart" {
        let prompt = event.getPrompt()
        event.setPrompt(prompt + " ha ha")
    }
}
```

Attach it to an agent:

```ts
@middlewares(Logger)
agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer the user."
        model = Gemini
    }
}
```

This middleware can run inside the runtime without calling the host SDK.

## 3. Why Middleware Should Not Be A Full Graph

Middleware is usually small policy logic:

- inspect event type,
- read session,
- read context,
- mutate prompt,
- skip intent,
- override result,
- swallow error,
- choose retry behavior.

It does not naturally need:

- graph scheduling,
- edges,
- child graph state,
- node-level checkpoints for every statement,
- active node state for every expression.

Recommendation:

```text
Main agent execution -> graph IR
DSL middleware -> interpreted AST first, bytecode later
Host middleware -> SDK callback
```

## 4. Runtime Shape

```text
GraphExecutor
  -> reaches lifecycle boundary
  -> builds MiddlewareEvent object
  -> runs compiled DSL middleware locally
  -> applies collected effects
  -> optionally calls host middleware
  -> records final decision/effects in execution state
  -> continues graph execution
```

This keeps middleware lightweight while still making its effects resumable and auditable.

## 5. Middleware Events

Initial events:

```text
runStart
llmStart
intent
llmEnd
runComplete
error
```

### 5.1 runStart

Runs before the agent starts. Useful for loading session state.

```ts
@middleware("load-session")
@context({ id: string })
function LoadSession(event: MiddlewareEvent) {
    if event.on == "runStart" {
        let context = event.getContext()
        let current = event.getSession()
        let loaded = getDatabase(context.data.id)

        if not loaded.error.isEmpty() {
            return error("could not load the session")
        }

        let next = if loaded.data == null then current else loaded.data
        event.setSession(next)
    }
}
```

### 5.2 llmStart

Runs before the model request is sent. Useful for prompt/config/model changes.

```ts
@middleware("prompt-prefix")
function PromptPrefix(event: MiddlewareEvent) {
    if event.on == "llmStart" {
        let prompt = event.getPrompt()
        event.setPrompt(prompt + "\nUse a concise tone.")
    }
}
```

### 5.3 intent

Runs when the runtime observes an intent.

Important intent types in v2:

```text
tool_call
tool_result
response_text
response_schema
error
custom intents
```

Workflows and helpers are expected to collapse into functions and agent calls, so `workflow_call` and `helper_call` may not remain as public top-level intents in v2.

Example:

```ts
@middleware("intent-guard")
function IntentGuard(event: MiddlewareEvent) {
    if event.on == "intent" {
        let intent = event.getIntent()

        if intent.type == "tool_call" {
            let value = intent.getValue()

            if value.name == "delete_user" and not isAdmin {
                event.override({
                    error: "user is not allowed to delete users"
                })
            }
        }
    }
}
```

Intent value shapes:

```ts
tool_call: {
    name: string
    args: object
}

tool_result: {
    name: string
    args: object
    result: any
}

response_text: {
    data: string
}

response_schema: {
    type: string
    value: any
}
```

### 5.4 llmEnd

Runs after the model completes a turn.
the example below is false we cant do that
```ts
@middleware("redact")
function Redact(event: MiddlewareEvent) {
    if event.on == "llmEnd" {
        let response = event.getResponse()
        event.setResponse(response.replace("secret", "[redacted]"))
    }
}
```

Open question: whether `llmEnd` can modify final response or only observe it.

### 5.5 runComplete

Runs when the run completes. Useful for saving session state.

```ts
@middleware("save-session")
@context({ id: string })
function SaveSession(event: MiddlewareEvent) {
    if event.on == "runComplete" {
        let session = event.getSession()
        let context = event.getContext()
        setDatabase(context.data.id, session)
    }
}
```

### 5.6 error

Runs when an error occurs.

```ts
@middleware("save-on-error")
@context({ id: string })
function SaveOnError(event: MiddlewareEvent) {
    if event.on == "error" {
        let session = event.getSession()
        let context = event.getContext()
        setDatabase(context.data.id, session)

        let err = event.getError()
        return error(err.message)
    }
}
```

Future control options may include `event.retry()`, `event.swallow()`, and `event.fail(...)`.

## 6. Event Object API

The proposed DX uses methods instead of return-object conventions:

```ts
event.getSession()
event.setSession(session)

event.getPrompt()
event.setPrompt(prompt)

event.getContext()
event.getIntent()
event.getError()
event.getResponse()

event.skip()
event.override(result)
event.swallow()
event.retry()
```

Internally, these methods should not mutate engine state immediately. They should record effects.

Example effect object:

```json
{
  "control": "continue",
  "effects": {
    "prompt": "new prompt",
    "session": null,
    "configPatch": null,
    "intentOverride": null
  }
}
```

After middleware finishes, the runtime validates and applies effects.

## 7. Middleware AST

Initial implementation should keep middleware as interpreted AST.

```json
{
  "middlewares": {
    "Logger": {
      "name": "Logger",
      "events": ["llmStart"],
      "body": [
        {
          "type": "if",
          "condition": {
            "type": "binary",
            "op": "==",
            "left": { "type": "member", "object": "event", "property": "on" },
            "right": { "type": "literal", "value": "llmStart" }
          },
          "then": [
            {
              "type": "let",
              "name": "prompt",
              "value": {
                "type": "call",
                "callee": "event.getPrompt",
                "args": []
              }
            },
            {
              "type": "expr",
              "value": {
                "type": "call",
                "callee": "event.setPrompt",
                "args": [
                  {
                    "type": "binary",
                    "op": "+",
                    "left": { "type": "var", "name": "prompt" },
                    "right": { "type": "literal", "value": " ha ha" }
                  }
                ]
              }
            }
          ]
        }
      ]
    }
  }
}
```

Later, the compiler can lower middleware AST to bytecode.

## 8. Future Bytecode

Possible bytecode:

```text
LOAD_EVENT_FIELD on
PUSH_CONST "llmStart"
EQ
JUMP_IF_FALSE L1
CALL_EVENT getPrompt 0
STORE_LOCAL prompt
LOAD_LOCAL prompt
PUSH_CONST " ha ha"
ADD
CALL_EVENT setPrompt 1
L1:
RETURN
```

Bytecode is useful for:

- faster repeated execution,
- smaller compiled artifacts,
- easier sandboxing,
- portable runtime behavior,
- WASM performance.

AST interpretation is better for the first compiler milestone because the language is still moving.

## 9. Host Middleware

Host middleware remains necessary for external systems:

- database adapters,
- filesystem access,
- secrets,
- analytics,
- custom observability,
- platform-specific APIs.

Example host middleware can still exist:

```ts
const logger = {
  name: "logger",
  onRunStart: async (session, ctx) => db.load("session.json", session),
  onRunComplete: async (session, ctx) => db.save("session.json", session),
  onError: async (error, session, ctx) => ({ swallow: true })
}
```

But v2 should not force every middleware to be written this way.

## 10. DSL Middleware With Host Tools

Since DSL middleware cannot directly access a database, it can call host tools if explicitly available.

```ts
tool getDatabase(id: string): Session
tool setDatabase(id: string, data: Session): Session

@middleware("session-db")
@context({ id: string })
function SessionDb(event: MiddlewareEvent) {
    if event.on == "runStart" {
        let context = event.getContext()
        let loaded = getDatabase(context.data.id)

        if loaded.error.isEmpty() and loaded.data != null {
            event.setSession(loaded.data)
        }
    }

    if event.on == "runComplete" {
        let context = event.getContext()
        let session = event.getSession()
        setDatabase(context.data.id, session)
    }
}
```

Open question: whether middleware can call all tools, only explicitly granted tools, or a separate middleware-tool list. #edit the middleware is being typed by human so it can call any tool provided it imported in the scope

## 11. Middleware Ordering

Recommended default:

```text
1. DSL middleware runs first.
2. Host middleware runs second.
3. Final combined effects are checkpointed.
```

This lets portable policy run locally, then host infra can observe or override.

Open question: whether host middleware should see the original event, the already-mutated event, or both.

## 12. Decided Direction

- Keep host middleware support.
- Add DSL-defined middleware for portable policy.
- Do not force middleware into the graph.
- Start with interpreted AST.
- Consider bytecode after syntax stabilizes.
- Middleware mutates an effect object, not runtime state directly.
- Middleware effects should be stored in execution state for resumability.

## 13. Still Deciding

- Exact `MiddlewareEvent` type shape.
- Whether event names are `runStart` or `run_start`.
- Whether middleware can call host tools.
- How middleware tool permissions are declared.
- Whether `llmEnd` can modify output.
- Whether host middleware runs before or after DSL middleware.
- How much middleware effect history to keep in checkpoints.
