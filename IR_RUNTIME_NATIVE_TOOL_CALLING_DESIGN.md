# IR Runtime Native Tool Calling + Structured Output — Refined Design

> **Status:** Design validation & refinement complete. Ready for implementation.
> **Scope:** Dual-mode (`block` | `native`) support for OpenAI, Gemini, Groq, and Custom (OpenAI-compatible) drivers.

---

## 1. Architecture Validation

### 1.1 Current State Assessment

After deep codebase analysis (~61k LOC) and official provider doc research, the existing plan in `IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md` is **architecturally sound**. No major course corrections needed. The following sections refine specific decisions, add provider-specific details discovered during research, and flag subtle traps.

### 1.2 Core Design Principle (Confirmed)

> **Normalize native calls into the existing intent/execution pipeline. Do not build a parallel executor.**

The engine's `process_intents()` already handles:
- `tool_call` → `execute_tool`
- `workflow_call` → `execute_workflow`
- `helper_call` → `execute_helper`
- `response_schema` / `response_text` → terminal response

Native mode should produce the **same internal intent payloads** as block mode:

```json
{ "type": "canonical_name", "args": { ... } }
```

The only difference is *how* intents enter the system:
- **Block mode:** `BlockOrchestrator` parses text brackets → `pending_intents`
- **Native mode:** Driver emits `NativeToolCall` events → `enqueue_native_call()` → `pending_intents`

This ensures middleware, session export/import, streaming JSONL, and SDK callbacks all work identically.

---

## 2. Provider Research Findings

### 2.1 OpenAI (Chat Completions + Responses APIs)

**Request — Chat Completions:**
```json
{
  "model": "gpt-4.1",
  "messages": [...],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "...",
        "parameters": { "type": "object", "properties": {...}, "required": [...], "additionalProperties": false },
        "strict": true
      }
    }
  ],
  "tool_choice": "auto",
  "stream": true
}
```

**Request — Responses API (GPT-5+):**
```json
{
  "model": "gpt-5",
  "input": [...],
  "tools": [
    {
      "type": "function",
      "name": "get_weather",
      "description": "...",
      "parameters": { ... },
      "strict": true
    }
  ],
  "stream": true
}
```

**Streaming Response — Chat Completions:**
```json
{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","function":{"name":"get_weather","arguments":""}}]}}]}
{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc\""}}]}}]}
// ... deltas until finish_reason: "tool_calls"
```

**Streaming Response — Responses API:**
```json
{"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_123","call_id":"call_123","name":"get_weather","arguments":""}}
{"type":"response.function_call_arguments.delta","output_index":0,"delta":"{\"location\":\"Paris\"}"}
{"type":"response.function_call_arguments.done","output_index":0,"arguments":"{\"location\":\"Paris\"}"}
```

**Tool Result — Chat Completions:**
```json
{"role": "tool", "tool_call_id": "call_abc", "content": "{\"temperature\": 22}"}
```

**Tool Result — Responses API:**
```json
{"type": "function_call_output", "call_id": "call_abc", "output": "{\"temperature\": 22}"}
```

**Structured Output — Chat Completions:**
```json
{"response_format": {"type": "json_schema", "json_schema": {"name": "Output", "schema": {...}, "strict": true}}}
```

**Structured Output — Responses API:**
```json
{"text": {"format": {"type": "json_schema", "name": "Output", "schema": {...}, "strict": true}}}
```

**Key OpenAI Constraints Discovered:**
1. **Strict mode** (recommended) requires `additionalProperties: false` on EVERY object and ALL fields in `required`.
2. Optional fields must use `{"type": ["string", "null"]}` union, NOT omitting from `required`.
3. Chat Completions and Responses APIs have **different tool schema shapes** (nested `function` object vs flat).
4. Tool call IDs (`call_xxx`) are **mandatory** for result correlation.
5. For reasoning models (GPT-5/o4-mini), reasoning items must be passed back with tool outputs.

### 2.2 Gemini (generateContent / streamGenerateContent)

**Request:**
```json
{
  "contents": [...],
  "tools": [
    {
      "functionDeclarations": [
        {
          "name": "get_weather",
          "description": "...",
          "parameters": { "type": "object", "properties": {...}, "required": [...] }
        }
      ]
    }
  ],
  "toolConfig": {
    "functionCallingConfig": { "mode": "AUTO" }
  }
}
```

**Response (non-streaming):**
```json
{
  "candidates": [{
    "content": {
      "parts": [
        {"functionCall": {"name": "get_weather", "args": {"location": "Paris"}, "id": "8f2b1a3c"}}
      ],
      "role": "model"
    }
  }]
}
```

**Tool Result:**
```json
{
  "role": "user",
  "parts": [{
    "functionResponse": {
      "name": "get_weather",
      "id": "8f2b1a3c",
      "response": {"result": "22°C"}
    }
  }]
}
```

**Structured Output:**
```json
{
  "generationConfig": {
    "responseFormat": {
      "text": {
        "mimeType": "application/json",
        "schema": { "type": "object", "properties": {...}, "required": [...] }
      }
    }
  }
}
```

**Key Gemini Constraints Discovered:**
1. Gemini 3 **always returns `id`** on `functionCall` parts. Must include exact `id` in `functionResponse`.
2. Gemini uses **thought signatures** for stateful context. Any model response part may contain `thought_signature` — must pass back unmodified for function calling to work across turns. (Current driver does NOT handle this.)
3. Function calling modes: `AUTO` (default), `ANY` (force function call), `NONE`, `VALIDATED`.
4. No `strict` flag, but `VALIDATED` mode ensures schema adherence when combined with structured outputs.
5. Current driver hardcodes `functionResponse` name as `"tool_result"` (line 362 of `gemini.rs`). **This is a bug** — must use the actual function name from the call.
6. For streaming, SSE chunks contain `candidates` with `content.parts`. Need to accumulate parts across chunks.

### 2.3 Groq / Custom (OpenAI-compatible)

Groq is OpenAI-compatible (Chat Completions shape). Custom driver targets OpenAI-compatible endpoints. Both can reuse OpenAI Chat Completions logic.

---

## 3. Refined Design Decisions

### 3.1 Protocol Mode Configuration

Add `toolProtocol` to `ModelConfigIR` (compiler) and runtime config resolution:

```rust
// In auwgent-ir-schema/src/lib.rs — ModelConfigIR
pub struct ModelConfigIR {
    pub model: ModelProviderIR,
    pub embedding: Option<ModelProviderIR>,
    pub prompt: JsonValue,
    #[serde(default)]
    pub tool_protocol: Option<String>, // "block" | "native"
}
```

**Resolution order:**
1. `model_config[0].default_config.tool_protocol`
2. If absent → `"block"` (default, backward-compatible)

The protocol mode lives at the IR level, not per-request. An agent uses one protocol for its entire lifecycle.

### 3.2 Provider-Native Namespacing + Routing

**Problem:** Tools, workflows, and helpers can share names. In block mode, `[tool_call: foo]` vs `[workflow_call: foo]` disambiguates. In native mode, providers see only a flat function name.

**Solution:** Prefix provider-visible names with the action kind:

```
tool_<name>
workflow_<name>
helper_<name>
```

Example: tool `search`, workflow `search`, helper `search` become:
- `tool_search`
- `workflow_search`
- `helper_search`

**Routing is prefix-based — no registry lookup needed for dispatch:**

```rust
fn route_native_call(provider_name: &str, args: Value) -> Option<(ActionKind, String, Value)> {
    if let Some((kind, name)) = provider_name.split_once('_') {
        match kind {
            "tool" => Some((ActionKind::Tool, name.to_string(), args)),
            "workflow" => Some((ActionKind::Workflow, name.to_string(), args)),
            "helper" => Some((ActionKind::Helper, name.to_string(), args)),
            _ => None,
        }
    } else {
        None
    }
}
```

This mirrors the orchestrator's intent routing (`tool_call`, `workflow_call`, `helper_call`) but uses the function name itself as the carrier. The engine receives `tool_search` from the provider, splits on the first `_`, knows it's a `Tool` named `search`, and routes to `execute_tool`.

**Name collision safety:**
- A tool named `workflow_foo` becomes `tool_workflow_foo`. Split → kind=`tool`, name=`workflow_foo`. Works correctly.
- A helper named `tool_helper` becomes `helper_tool_helper`. Split → kind=`helper`, name=`tool_helper`. Works correctly.
- The first underscore is always the delimiter. Names with multiple underscores (e.g., `search_docs`) are fine: `tool_search_docs` → kind=`tool`, name=`search_docs`.

**Schema generation still uses a lightweight registry** to collect all callable surfaces and their JSON schemas, but routing is pure prefix logic:

```rust
pub struct NativeCallableRegistry {
    /// provider_name → schema + metadata
    entries: HashMap<String, NativeCallableEntry>,
}

pub struct NativeCallableEntry {
    pub provider_name: String,   // e.g. "tool_search"
    pub canonical_name: String,  // e.g. "search"
    pub action_kind: ActionKind,
    pub input_schema: Value,     // JSON Schema for provider
}

pub enum ActionKind { Tool, Workflow, Helper }
```

### 3.3 Schema Generation (Replaces `native_schema.rs`)

**Decision:** Use **nested JSON Schema** for native mode (not flattened aliases).

Reasoning:
- Providers natively understand nested objects.
- Flattening would require unflattening args before execution, adding complexity.
- The IR already has rich nested type information; flattening discards it.
- OpenAI strict mode and Gemini validated mode both expect proper nested schemas.

**Implementation strategy:**
Build a `TypeSchemaBuilder` that recursively transforms IR type shapes into JSON Schema:

```rust
fn ir_type_to_json_schema(
    ir_type: &Value,
    types: Option<&HashMap<String, TypeDefinition>>,
    strict: bool,  // for OpenAI strict mode
) -> Value
```

**IR type shapes to support:**

| IR Shape | JSON Schema Output |
|----------|-------------------|
| `"string"` | `{"type": "string"}` |
| `"number"` | `{"type": "number"}` |
| `"integer"` | `{"type": "integer"}` |
| `"boolean"` | `{"type": "boolean"}` |
| `"any"` | `{}` (unconstrained) |
| `{"type": "array", "items": <type>}` | `{"type": "array", "items": <schema>}` |
| `{"type": "literal", "value": <val>}` | `{"type": <inferred>, "enum": [<val>]}` |
| `{"type": "object", "properties": {...}}` | `{"type": "object", "properties": {...}, "required": [...], "additionalProperties": false}` |
| `{"type": "typeRef", "name": "Foo"}` | Resolve from `types` map, recurse |
| `{"type": "union", "variants": [...]}` | `{"anyOf": [...]}` (with limitations for strict mode) |
| field with `"optional": true` | Include in `required` if strict; for OpenAI optional, use `{"type": ["string", "null"]}` |

**Helper input handling:**

Helpers have three input shapes:
1. `null` / absent → default `{"input": {"type": "string"}}` (single text param)
2. `{"kind": "direct", "type": <type>}` → use that type directly
3. `{"kind": "properties", "fields": {...}}` → object with those fields

The schema builder must call `flatten_helper_input_specs` logic to determine the actual parameter shape, then generate the schema.

**OpenAI strict mode specifics:**
- Every object must have `additionalProperties: false`
- Every property must be in `required`
- Optional fields expressed as `{"type": ["T", "null"]}`
- If a union type is used, strict mode may reject it. For initial implementation, warn and fall back to non-strict if unions are detected.

**Gemini specifics:**
- No `strict` flag
- `additionalProperties: false` is still good practice
- Optional fields can omit from `required` (standard JSON Schema behavior)

### 3.4 Prompt Generation Split

**Block mode (current):**
```
[User prompt expression result]

[Binding block if context present]

[Block protocol rules from intents.rs]
[Tool signatures, examples, constraints]
```

**Native mode (new):**
```
[User prompt expression result]

[Binding block if context present — UNCHANGED]
```

Nothing else is appended. The model learns all callable surfaces from the provider-native tool/function declarations. The `description` fields in the JSON Schema function definitions carry the capability descriptions. No prompt-based tool summary, no block syntax rules, no examples.

**Binding blocks are protocol-agnostic:**
Binding blocks with `@@symbol` notation exist for **prompt/prefix caching optimization**, not as part of the block protocol. The `@@` markers allow the static system prompt to remain cacheable while only the synthetic binding turn changes each iteration. Inlining resolved values into the system prompt would invalidate the cache on every binding change. Therefore:
- **Keep binding blocks unchanged in native mode** — same `[binding]`/`@@symbol` format, same synthetic turn injection
- The binding mechanism is orthogonal to the output protocol (block vs native)
- Both modes benefit equally from prefix caching

**Implementation:**
```rust
fn generate_main_prompt(&self) -> AuwgentResult<String> {
    let protocol = self.resolve_tool_protocol();
    let mut prompt = self.evaluate_user_prompt()?;
    
    if protocol == "block" {
        let intents = generate_block_protocol_prompt_with_binding_rules(&self.ir, ...);
        if !intents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&intents);
        }
    }
    // native mode: append nothing — tool schemas speak for themselves
    
    Ok(prompt)
}
```

### 3.5 Driver Event Contract Extensions

Extend `ModelEvent` with native-specific variants:

```rust
pub enum ModelEvent {
    ContentChunk(String),
    Usage(TokenUsage),
    FinishReason(FinishReason),
    Metadata(ModelMetadata),
    
    // NEW — Native mode only
    NativeToolCall {
        id: Option<String>,        // OpenAI: "call_abc", Gemini: "8f2b1a3c"
        provider_name: String,     // e.g. "tool_search"
        arguments: Value,          // Parsed JSON object
    },
    NativeStructuredOutput(Value), // Parsed JSON object matching output schema
}
```

**Driver buffering strategy:**

OpenAI Chat Completions streaming sends tool call **deltas**. The driver must buffer:

```rust
struct OpenAIToolCallBuffer {
    calls: HashMap<u32, BufferedToolCall>, // index → partial call
}

struct BufferedToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String, // accumulated JSON string
}
```

When `finish_reason: ToolCalls` arrives, flush buffered calls as `NativeToolCall` events.

Gemini streaming sends partial `candidates[0].content.parts`. Each part may contain a `functionCall`. For streaming, accumulate parts until the chunk has no function call parts or finish reason indicates completion.

**Important:** In native mode, `ContentChunk` events should still be emitted for any assistant text content that accompanies tool calls. Some models produce both text and tool calls in the same turn.

### 3.6 Session State Extensions

Extend `Turn` with optional native fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub input: String,
    #[serde(default, rename = "inputParts", skip_serializing_if = "Option::is_none")]
    pub input_parts: Option<Vec<Value>>,
    pub model_response: String,
    
    // NEW
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>, // "block" | "native"
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_assistant_turn: Option<NativeAssistantTurn>,
    
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tool_results: Option<Vec<NativeToolResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAssistantTurn {
    pub text_content: Option<String>,
    pub tool_calls: Vec<NativeToolCallRecord>,
    pub structured_output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolCallRecord {
    pub id: Option<String>,
    pub provider_name: String,
    pub canonical_name: String,
    pub action_kind: String, // "tool" | "workflow" | "helper"
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolResult {
    pub call_id: Option<String>,
    pub provider_name: String,
    pub canonical_name: String,
    pub action_kind: String,
    pub arguments: Value,
    pub result: Value,
}
```

**Protocol-aware message reconstruction:**

`SessionState::to_messages_with_bindings` must branch by protocol:

**Block mode:** (current behavior, unchanged)
- System prompt
- For each turn: [binding?] → User(input) → Model(response_text)
- Tool results are embedded as `[result]` blocks in the next turn's user input

**Native mode — OpenAI Chat Completions:**
- System prompt
- For each turn:
  - User(input)
  - Assistant message with `tool_calls` array (if any)
  - Tool result messages: `{"role": "tool", "tool_call_id": "...", "content": "..."}`
  - Or standard assistant content message

**Native mode — Gemini:**
- System instruction (extracted separately)
- Contents array:
  - User turns with text parts
  - Model turns with `functionCall` parts
  - User turns with `functionResponse` parts

**Critical for resumability:**
- Provider call IDs MUST be persisted in `NativeToolCallRecord.id`
- On session import, `to_messages_with_bindings` must reconstruct the exact provider-specific message format
- If a session was started in block mode and imported, it continues in block mode (protocol is per-session, not per-turn)

### 3.7 Execution Adapter

```rust
impl AuwgentEngine {
    pub(super) fn enqueue_native_call(&self, call: NativeToolCall) -> AuwgentResult<()> {
        let registry = self.native_callable_registry.lock().unwrap();
        let (kind, canonical_name, args) = route_native_call(&call.provider_name, call.arguments)
        .ok_or_else(|| AuwgentError::UnknownNativeCallable(call.provider_name.clone()))?;
        
        let intent_payload = serde_json::json!({
            "type": canonical_name,
            "args": args,
            "_native_call_id": call.id, // internal tracking, removed before middleware/SDK
        });
        
        let intent_name = match kind {
            ActionKind::Tool => "tool_call",
            ActionKind::Workflow => "workflow_call",
            ActionKind::Helper => "helper_call",
        };
        
        self.pending_intents.lock().unwrap().push((intent_name.to_string(), intent_payload));
        Ok(())
    }
}
```

**Storing call IDs for results:**

During `process_intents()`, when executing a native-mode action, store the mapping:

```rust
// In engine state:
native_call_id_map: Arc<Mutex<HashMap<String, (String, Value)>>>, // call_id → (provider_name, args)
```

After `execute_tool` / `execute_workflow` / `execute_helper` completes, look up the call ID and store the result in `native_tool_results` for message reconstruction.

### 3.8 Native Final Output (Structured Output)

**Decision:** Keep output schema **separate** from tools. Do NOT model it as a synthetic function.

OpenAI and Gemini both have dedicated structured output APIs. Using them provides:
- Better model adherence to schema
- No risk of the model "calling" the output as a tool
- Cleaner SDK semantics

**OpenAI Chat Completions:**
```json
{
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "AuwgentOutput",
      "schema": { ... },
      "strict": true
    }
  }
}
```

**OpenAI Responses API:**
```json
{
  "text": {
    "format": {
      "type": "json_schema",
      "name": "AuwgentOutput",
      "schema": { ... },
      "strict": true
    }
  }
}
```

**Gemini:**
```json
{
  "generationConfig": {
    "responseFormat": {
      "text": {
        "mimeType": "application/json",
        "schema": { ... }
      }
    }
  }
}
```

**Engine integration:**
- When `ir.output` is present and protocol is `native`, generate output schema and pass to driver via `config` parameter
- Driver embeds it in the provider-specific request field
- On response, driver parses the structured output and emits `NativeStructuredOutput(Value)`
- Engine normalizes it to `response_schema` intent with the same payload shape as block mode

**Fallback (future):** If a custom OpenAI-compatible endpoint doesn't support structured output, we can fall back to a synthetic tool `__auwgent_output`. This is not needed for the initial implementation.

### 3.9 Middleware Preservation

Native mode must emit **identical** middleware events:

| Native Event | Middleware Intent | Payload Shape |
|-------------|-------------------|---------------|
| Tool call | `tool_call` | `{type, args}` |
| Tool result | `tool_result` | `{name, args, result}` |
| Workflow call | `workflow_call` | `{type, args}` |
| Workflow result | `workflow_result` | `{name, args, result}` |
| Helper call | `helper_call` | `{type, args}` |
| Helper result | `helper_result` | `{name, args, result}` |
| Structured output | `response_schema` | `{type, ...output fields}` |
| Text response | `response_text` | `{text, ...}` |

**No `_raw` field** in native mode (there are no raw blocks to capture).

---

## 4. Implementation Phases (Refined)

### Phase 1: Schema Normalization Layer + Registry

**Goal:** Replace `native_schema.rs` with a complete, tested schema builder.

**Files:**
- `ir-runtime/src/runtime/engine/native_schema.rs` (rewrite)
- `ir-runtime/src/runtime/engine/native_registry.rs` (new)

**Tasks:**
1. Define `NativeCallableRegistry`, `NativeCallableEntry`, `ActionKind`
2. Build `TypeSchemaBuilder` with recursive IR → JSON Schema conversion
3. Handle all IR type shapes: primitives, arrays, literals, objects, typeRef, unions
4. Handle helper input semantics (`flatten_helper_input_specs` integration)
5. Generate prefixed provider names (`tool_`, `workflow_`, `helper_`)
6. Support OpenAI strict mode requirements (`additionalProperties: false`, all fields required)
7. Support Gemini standard JSON Schema
8. Add comprehensive unit tests covering:
   - Primitives, optional fields
   - Nested objects, arrays of objects
   - `typeRef` resolution
   - Helper null/missing input defaults
   - Helper `kind: direct` and `kind: properties`
   - Output schema variants (`__variants`)
   - Name collision across tool/workflow/helper
   - Strict mode compliance validation

**Exit criteria:** All tests pass. Registry can be constructed from any IR and produce valid schemas for both OpenAI and Gemini shapes.

### Phase 2: Protocol Mode + Prompt Split

**Goal:** Add `toolProtocol` config and split prompt generation.

**Files:**
- `auwgent-compiler/crates/auwgent-ir-schema/src/lib.rs`
- `ir-runtime/src/runtime/engine/prompt.rs`
- `ir-runtime/src/runtime/engine.rs` (add `resolve_tool_protocol()`)

**Tasks:**
1. Add `tool_protocol: Option<String>` to `ModelConfigIR` (compiler)
2. Add protocol resolution to engine
3. In native mode, skip `generate_block_protocol_prompt_with_binding_rules`
4. ~~Add lightweight capability summary~~ — removed; native tool schemas carry descriptions
5. Binding blocks remain unchanged in native mode (caching optimization, not protocol syntax)
6. Add tests proving native prompt does not contain block syntax
7. Add tests proving block prompt is unchanged

**Exit criteria:** Prompt tests pass. Native prompt is clean and informative.

### Phase 3: Driver Contract + OpenAI Implementation

**Goal:** Extend `ModelEvent` and implement native tool calling in OpenAI driver.

**Files:**
- `ir-runtime/src/runtime/drivers/mod.rs`
- `ir-runtime/src/runtime/drivers/openai.rs`

**Tasks:**
1. Add `NativeToolCall` and `NativeStructuredOutput` to `ModelEvent`
2. Update OpenAI driver `stream_generate`:
   - Accept tools/config via the `config` parameter OR add a new driver method
   - Actually, better: pass native tools via `config` since `ModelDriver::stream_generate` signature is fixed
   - Wait — `config` is `Option<Value>` and merged into body. We can pass `{"tools": [...], "response_format": {...}}` via config.
   - BUT the driver needs to know it's in native mode to parse tool_calls from response.
   - Decision: Add `native_tools: Option<Vec<Value>>` and `native_output_schema: Option<Value>` to a new driver method, OR extend config with a sentinel.
   - **Better approach:** Add new methods to `ModelDriver` trait:
     ```rust
     async fn stream_generate_native(
         &self,
         model: &str,
         messages: &[Message],
         tools: Option<Vec<Value>>,
         output_schema: Option<Value>,
         config: Option<Value>,
     ) -> Result<ModelEventStream, String>;
     ```
     With default implementation delegating to `stream_generate` for backward compatibility.
   - Actually, trait changes require updating all drivers. Safer: keep `stream_generate` but driver detects native mode from message structure or config.
   - **Final decision:** Overload `config` with `{"auwgent_native_tools": [...], "auwgent_native_output_schema": {...}}`. Driver strips these before sending to provider. This avoids trait changes.
   - *Note: The `auwgent_` prefix here is for internal config keys only, not provider-visible function names.*
3. Implement OpenAI Chat Completions tool call delta buffering
4. Emit `NativeToolCall` events when tool calls are complete
5. Parse `response_format` structured output from `message.content` when configured
6. Add mocked stream tests for:
   - Single tool call
   - Parallel tool calls
   - Tool call with accompanying text
   - Structured output response

**Exit criteria:** Mocked OpenAI streams produce correct `NativeToolCall` events.

### Phase 4: Gemini Driver Implementation

**Goal:** Implement native tool calling in Gemini driver.

**Files:**
- `ir-runtime/src/runtime/drivers/gemini.rs`

**Tasks:**
1. Accept native tools via config (`auwgent_native_tools` → `tools.function_declarations`)
2. Accept output schema via config (`auwgent_native_output_schema` → `generationConfig.responseFormat`)
3. Parse `functionCall` parts from `candidates[0].content.parts`
4. Emit `NativeToolCall` events
5. Handle Gemini 3 `id` field on function calls
6. **Fix existing bug:** `functionResponse` name must be actual function name, not hardcoded `"tool_result"`
7. Handle thought signatures (pass through unmodified)
8. Add mocked response tests

**Exit criteria:** Mocked Gemini responses produce correct `NativeToolCall` events.

### Phase 5: Session State + Message Reconstruction

**Goal:** Extend session for native turns and provider-specific message reconstruction.

**Files:**
- `ir-runtime/src/runtime/session.rs`
- `ir-runtime/src/runtime/engine/runtime_loop.rs`

**Tasks:**
1. Add `NativeAssistantTurn`, `NativeToolResult`, `NativeToolCallRecord` types
2. Extend `Turn` with optional native fields
3. Implement `to_messages_native_openai(&self) -> Vec<Message>`
4. Implement `to_messages_native_gemini(&self) -> Vec<Value>` (Gemini contents are Value, not Message)
   - Actually, keep Message abstraction but add native payload. Or add a new method to session that returns provider-specific format.
   - **Decision:** Keep `Message` as the abstraction. Add `native_payload: Option<Value>` to `Message` for driver-specific extensions. Driver ignores it in block mode, uses it in native mode.
   - Better yet: don't pollute Message. Have session return `Vec<Message>` and a separate `Vec<NativeTurnData>` that the driver can query.
   - **Simplest:** Add `tool_calls: Option<Vec<Value>>` and `tool_call_id: Option<String>` to `Message` as optional serde fields. OpenAI driver uses them; Gemini driver ignores and uses its own content format.
5. Update runtime loop to store native assistant turns after streaming
6. Update runtime loop to store native tool results after execution
7. Add round-trip tests:
   - Export session after tool calls → import → continue → verify messages reconstruct correctly

**Exit criteria:** Session round-trip tests pass for both OpenAI and Gemini message formats.

### Phase 6: Execution Adapter + Integration

**Goal:** Wire native calls into `process_intents` and test end-to-end.

**Files:**
- `ir-runtime/src/runtime/engine/execution.rs`
- `ir-runtime/src/runtime/engine/runtime_loop.rs`
- `ir-runtime/src/runtime/engine.rs`

**Tasks:**
1. Implement `enqueue_native_call()`
2. Store call ID → (provider_name, canonical_name, action_kind) mapping
3. In `process_intents()`, when building results, look up call IDs and store in `native_tool_results`
4. In `build_results_payload()`, skip `[result]` blocks in native mode (results go via native messages)
5. Update runtime loop:
   - If native mode: emit `NativeToolCall` events → `enqueue_native_call()` → `process_intents()`
   - If structured output event: normalize to `response_schema` intent
6. Add end-to-end tests:
   - Native tool call executes registered callback
   - Native workflow call executes workflow body
   - Native helper call enters helper execution
   - Multiple parallel native calls execute concurrently
   - Middleware skip/override works
   - Structured output emits correct `response_schema` event

**Exit criteria:** End-to-end tests pass. Native and block modes produce identical SDK-visible intent payloads.

### Phase 7: Groq + Custom Driver + WASM Bindings

**Goal:** Enable native mode for remaining drivers and WASM target.

**Files:**
- `ir-runtime/src/runtime/drivers/openai.rs` (Groq reuses this)
- `targets/wasm-runtime/src/lib.rs`

**Tasks:**
1. Groq driver: Already uses OpenAI-compatible logic. Native mode works once OpenAI Chat Completions support is done.
2. Custom driver: Same as OpenAI Chat Completions.
3. WASM bindings:
   - Expose `set_tool_protocol("native" | "block")` if needed (or read from IR config)
   - Ensure `ModelEvent` new variants serialize correctly through `wasm-bindgen`
   - Test in browser/Cloudflare Workers context

**Exit criteria:** WASM compiles. Native mode works in browser.

---

## 5. Risk Register (Updated)

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| OpenAI strict mode rejects complex IR schemas (unions, deep nesting) | Medium | High | Detect unsupported shapes, auto-fallback to non-strict with warning |
| Gemini thought signatures not passed back → broken multi-turn | Medium | High | Capture and persist `thought_signature` in session turns |
| Session export/import size bloat from native fields | Low | Medium | Use `skip_serializing_if = Option::is_none`; native fields are small |
| Name collision with `tool_`/`workflow_`/`helper_` prefix | Very Low | Medium | Prefix is deterministic; user names like `tool_search` become `tool_tool_search` — still unambiguous |
| Block mode regression | Low | Very High | Comprehensive regression tests; default remains block |
| Streaming tool call parsing bugs (delta accumulation) | Medium | High | Extensive mocked stream tests for both providers |
| Helper input semantics broken in native mode | Medium | High | Reuse existing `flatten_helper_input_specs` logic; test all 3 input shapes |
| Groq/Custom endpoints don't support `response_format` | Medium | Medium | Document limitation; future fallback to synthetic tool |

---

## 6. Recommended First Code Change

**Start with Phase 1: Schema Normalization Layer.**

Create `native_registry.rs` and rewrite `native_schema.rs` as a `TypeSchemaBuilder`. Add unit tests that exercise the full IR type surface before any provider code is written.

Why first:
- It validates whether the IR truly captures everything needed for provider schemas
- It's a pure function — easy to test, no async, no driver dependencies
- It exposes helper input edge cases early
- It defines the `NativeCallableRegistry` contract that every subsequent phase depends on

**Specific first commit:**
```
ir-runtime/src/runtime/engine/native_registry.rs   (new)
ir-runtime/src/runtime/engine/native_schema.rs     (rewrite)
ir-runtime/src/runtime/engine/native_schema_tests.rs (new, comprehensive)
```

---

## 7. Appendix: Provider Message Format Reference

### OpenAI Chat Completions — Native Tool Calling

**Assistant turn with tool calls:**
```json
{"role": "assistant", "content": null, "tool_calls": [
  {"id": "call_abc", "type": "function", "function": {"name": "tool_search", "arguments": "{\"query\":\"hello\"}"}}
]}
```

**Tool result:**
```json
{"role": "tool", "tool_call_id": "call_abc", "content": "{\"results\":[]}"}
```

### Gemini — Native Tool Calling

**Model turn with function call:**
```json
{"role": "model", "parts": [
  {"functionCall": {"name": "tool_search", "args": {"query": "hello"}, "id": "fc_123"}}
]}
```

**Function result:**
```json
{"role": "user", "parts": [
  {"functionResponse": {"name": "tool_search", "id": "fc_123", "response": {"result": "..."}}}
]}
```

---

*Design validated against:*
- Current `ir-runtime` architecture (engine, session, drivers, execution, prompt, intents)
- OpenAI API docs: Function Calling (2025-08-07), Structured Outputs (2024-08-06)
- Gemini API docs: Function Calling (2024-07-29), Structured Output (2026-05-07)
- Existing `IR_RUNTIME_NATIVE_TOOL_CALLING_PLAN.md` (all 6 phases confirmed, refined with provider specifics)
