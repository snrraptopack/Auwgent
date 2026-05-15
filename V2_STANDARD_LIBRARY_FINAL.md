# Auwgent v2 Standard Library

Status: design reference  
Source notes: `not_standard.txt`, `not.txt`

The v2 standard library exists to reduce unnecessary host-language tool glue and FFI roundtrips for common agentic operations.

The goal:

```text
Common operations should run inside the runtime.
Custom business integrations should stay host-provided.
```

## 1. Current Host Tool Pattern

Today a developer often writes a host-language function:

```ts
async function getInfo(): Promise<string> {
  const response = await fetch("https://example.com/info")
  const result = await response.json()
  return result
}
```

Then registers it with the generated SDK:

```ts
import { auwgent, type AuwgentConfig } from "./generated"

const config: AuwgentConfig = {
  apiKey: "...",
  tools: {
    getInfo
  }
}

const agent = auwgent(config)
```

The DSL declares the tool:

```ts
tool getInfo(): string
    @desc "Use this to get info"

agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer the user."
        model = Gemini
        tools = [getInfo]
    }
}
```

This works, but every tool call crosses the runtime-host boundary:

```text
Rust runtime
  -> FFI
  -> host tool executes
  -> FFI
  -> Rust receives result
```

For common operations like HTTP fetch, JSON parsing, array manipulation, hashing, and UUIDs, this is unnecessary.

## 2. Standard Library Pattern

With a standard library, the tool can be written in the DSL:

```ts
@tool
@desc "Use this to get info"
function getInfo(): string {
    let response = fetch<string>("https://example.com/info")

    if response.error.isEmpty() {
        return response.data
    }

    return response.error
}

agent Hello(input: Text): Text {
    reply(input) with {
        prompt = "Answer the user."
        model = Gemini
        tools = [getInfo]
    }
}
```

The host config becomes simpler:

```ts
import { auwgent, type AuwgentConfig } from "./generated"

const config: AuwgentConfig = {
  apiKey: "..."
}

const agent = auwgent(config)
```

The `fetch` call runs in the runtime layer.

```text
Rust runtime
  -> stdlib fetch
  -> Rust receives result
```

## 3. Result Shape

Standard library effectful calls should return a result envelope:

```ts
{
    data: T | null
    error: string
}
```

Example:

```ts
let response = fetch<string>("https://example.com/info")

if response.error.isEmpty() {
    return response.data
}

return response.error
```

The `<string>` syntax is a type hint, not full generics.

```ts
fetch<string>("...")
fetch<UserProfile>("...")
```

The compiler uses this hint to type `response.data`.

## 4. What Belongs In The Standard Library

Good standard library candidates:

| Operation | Runtime-owned? | Reason |
|---|---:|---|
| `fetch` | yes | Very common, avoids host tool glue |
| `JSON.parse` / `JSON.stringify` | yes | Runtime-local data operation |
| `array.map` / `filter` / `reduce` | yes | Deterministic local operation |
| string helpers | yes | Deterministic local operation |
| `crypto.sha256` / hash | yes | Portable and common |
| `uuid` | yes | Common runtime utility |
| `date.now` | yes | Common, but non-deterministic |
| `regex` | yes | Common deterministic utility |
| `log` / `print` | yes | Routed to runtime/host logging |

Host tool candidates:

| Operation | Runtime-owned? | Reason |
|---|---:|---|
| database queries | no | App-specific credentials and drivers |
| filesystem access | no | Platform-specific permissions |
| custom business logic | no | Belongs to user application |
| analytics | no | App/platform-specific |
| Sentry or tracing vendors | no | App-specific integration |
| secrets access | no | Host/platform security boundary |

## 5. Standard Library And Resumability

Standard library operations must be classified by effect.

### 5.1 Pure Operations

Pure operations can be replayed.

Examples:

```ts
input.trim()
items.filter(...)
JSON.stringify(value)
hash("hello")
```

Graph behavior:

```text
No mandatory checkpoint.
Can replay on resume.
May cache output for debugging.
```

### 5.2 Non-Deterministic Operations

Non-deterministic operations should checkpoint.

Examples:

```ts
date.now()
uuid()
random()
```

Graph behavior:

```text
Checkpoint output after execution.
Do not regenerate value on resume.
```

### 5.3 External Operations

External operations must checkpoint.

Examples:

```ts
fetch<string>("https://example.com")
```

Graph behavior:

```text
Create effectful node.
Store invocation metadata.
Store result.
Resume from stored result if already completed.
```

## 6. IR Shape

Pure stdlib call:

```json
{
  "id": "n4",
  "type": "stdlib_call",
  "name": "string.contains",
  "effect": "pure",
  "args": [
    { "from": "n0" },
    { "literal": "weather" }
  ]
}
```

External stdlib call:

```json
{
  "id": "n8",
  "type": "stdlib_call",
  "name": "fetch",
  "effect": "external",
  "args": {
    "url": { "literal": "https://example.com/info" },
    "method": { "literal": "GET" }
  },
  "typeHint": "string",
  "checkpoint": "required"
}
```

Non-deterministic stdlib call:

```json
{
  "id": "n9",
  "type": "stdlib_call",
  "name": "uuid",
  "effect": "nondeterministic",
  "args": {},
  "checkpoint": "required"
}
```

## 7. Fetch API

Initial shape:

```ts
fetch<T>(url: string): Result<T>
```

Future extended shape:

```ts
fetch<T>(url: string, options: {
    method?: string
    headers?: object
    body?: string | object
    timeoutMs?: number
}): Result<T>
```

Example:

```ts
type Weather = {
    city: string
    temperature: number
    summary: string
}

@tool
@desc "Use this to get weather"
function getWeather(city: string): string {
    let response = fetch<Weather>("https://api.example.com/weather?city={city}")

    if not response.error.isEmpty() {
        return response.error
    }

    return "{response.data.city}: {response.data.summary}"
}
```

## 8. Runtime Targets

The standard library should run in:

- native Rust runtime,
- WASM runtime,
- Node bindings,
- Python bindings,
- Dart bindings,
- Rust target SDK.

The implementation may differ by target, but the DSL behavior should be stable.

For example:

- Native Rust can use a Rust HTTP client.
- WASM can use browser/worker fetch.
- Node can use runtime-provided fetch through the Rust/WASM layer if available.

## 9. Host Escape Hatch

The standard library should not try to replace host tools.

Use host tools for:

```ts
tool getUserFromDatabase(id: string): User
tool saveAuditLog(event: AuditEvent): bool
tool chargeCustomer(customerId: string, amount: number): ChargeResult
```

Use stdlib for:

```ts
fetch<string>("https://example.com")
JSON.parse<User>(raw)
crypto.sha256(value)
uuid()
```

## 10. Security And Permissions

Runtime-owned stdlib operations need permissions.

Example config:

```json
{
  "stdlib": {
    "fetch": {
      "allow": ["https://api.example.com/*"],
      "timeoutMs": 5000
    }
  }
}
```

Open question: whether permissions live in DSL, host config, or both.

Recommendation:

```text
DSL declares what the program wants.
Host config grants what the runtime allows.
```

## 11. Decided Direction

- Add a standard library for common runtime-owned operations.
- Keep host tools for app-specific integrations.
- Standard library calls can reduce FFI roundtrips.
- Effectful stdlib calls must be checkpointed in graph execution.
- `fetch<T>` uses a type hint, not full generic programming.
- Result shape should be `{ data, error }`.

## 12. Still Deciding

- Exact initial stdlib surface.
- Exact `fetch` options shape.
- Whether stdlib APIs are globally available or imported.
- Permission model for `fetch`, logging, crypto, and time.
- Whether stdlib implementation is entirely Rust or partly target-specific.
- Whether `fetch<T>` parses JSON automatically or leaves parsing explicit.
- How stdlib errors map into middleware `error` events.
