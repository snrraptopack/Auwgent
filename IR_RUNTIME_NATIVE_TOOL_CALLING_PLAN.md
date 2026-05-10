# IR Runtime Native Tool Calling Plan

## Goal

Add a "traditional" provider-native tool/function calling path beside the current Auwgent block protocol path.

In native mode, the model should not be taught the block protocol from `ir-runtime/src/intents.rs`. It should receive normal provider-native tools, produce native tool calls, and still let the Auwgent runtime execute tools, workflows, and helpers through the same callbacks and session persistence flow.

The runtime should therefore support two model-output protocols:

- `block`: existing text blocks such as `[tool_call: name]`, `[workflow_call: name]`, `[schema: Output]`, and `[response_text]`.
- `native`: provider-native function/tool calls plus provider-native structured final output.

The executor should normalize both protocols into the same internal intent/action shape before execution.

## Current Findings

### Block mode is deeply integrated

The current runtime assumes model output is text. `AuwgentEngine::run` streams `ModelEvent::ContentChunk`, appends it to `current_raw_response`, writes it into `BlockOrchestrator`, then calls `process_intents`.

That gives block mode a clear path:

1. Prompt is generated from the user prompt plus `generate_block_protocol_prompt_with_binding_rules`.
2. Provider returns text chunks.
3. `BlockOrchestrator` parses blocks into pending intents.
4. `process_intents` executes `tool_call`, `workflow_call`, and `helper_call`.
5. Tool results are serialized back as `[result]` blocks in the next turn.

Native mode cannot reuse this by only adding JSON Schema. It needs provider events for tool calls and tool results, plus a native session representation.

### `native_schema.rs` is a start, not a complete contract

`ir-runtime/src/runtime/engine/native_schema.rs` currently builds function schemas for:

- `ir.tools`
- `ir.workflows`
- `ir.helpers`

Issues found:

- It is not wired into `ModelDriver::stream_generate`.
- It does not preserve action kind. A tool, workflow, and helper with the same name would collide. Even without collisions, the executor cannot tell which execution path to use from the native function name alone unless a registry maps name to action kind.
- Helper input handling does not match the existing helper flattening semantics. Current block mode uses `flatten_helper_input_specs`, including a default `input: string` for missing/null helper input. `native_schema.rs` treats missing helper input as `{}`.
- Type resolution is incomplete. It handles primitive strings, arrays, `literal`, and one `ref` shape, but the IR also uses shapes like `typeRef`, `object`, `union`, nested property maps, optional flags, and output variants.
- Literal schema is wrong for non-string literals because it always emits `"type": "string"`.
- It does not include `additionalProperties: false`, provider strictness flags, or any deterministic schema normalization.
- It does not handle final output schemas from `ir.output`.
- It does not handle custom intents or component/render intents. That may be fine for an initial native tool-calling scope, but it must be explicit.

### Prompt generation needs a mode boundary

`generate_main_prompt` always appends the block protocol prompt from `intents.rs`. That is correct for block mode, but wrong for native mode.

Native mode should still evaluate the user's configured prompt expression and context bindings, but it should not append:

- block syntax rules
- tool/workflow/helper signature text
- examples of `[tool_call]`, `[workflow_call]`, `[helper_call]`, `[schema]`

In native mode the model should learn callable surfaces from native `tools` and final output schemas from provider-native structured output, not from prompt instructions.

### Session state is text/block-oriented

`SessionState` stores:

- `system_prompt: Option<String>`
- `turns: Vec<Turn>`
- each `Turn` has `input`, optional `inputParts`, and `model_response: String`

This is easy to export/import today, but native tool calling needs to persist more than assistant text:

- assistant tool calls, including provider call IDs where required
- tool results linked to those call IDs
- final assistant content
- provider-native structured output
- enough normalized data to replay the next native request after import

If native calls are stored only as synthetic text, the runtime loses provider semantics and cannot reliably continue a native tool-call conversation, especially for OpenAI-style `tool_call_id` or Gemini `functionCall/functionResponse` relationships.

### Driver trait only exposes text chunks

`ModelDriver::stream_generate` returns `ModelEventStream`, but `ModelEvent` currently has only:

- `ContentChunk(String)`
- usage and finish metadata

OpenAI's driver notices a `"tool_calls"` finish reason but does not parse streamed tool-call deltas. Gemini's driver maps function response messages in one direction for `Role::ToolResult`, but it does not expose native function calls from model output.

Native mode needs driver events for:

- assistant content chunks
- tool-call start/delta/done or a complete tool-call event
- provider-native structured final output
- provider-specific call IDs and names

### Executor already has the right core shape

`process_intents` already knows how to execute normalized actions:

- `tool_call` -> `execute_tool`
- `workflow_call` -> `execute_workflow`
- `helper_call` -> `execute_helper`

The right design is not a second executor. Native output should be normalized into the same internal payload shape:

```json
{
  "type": "tool_or_workflow_or_helper_name",
  "args": {}
}
```

with an internal action kind attached before it reaches `process_intents`, or with separate normalized names: `tool_call`, `workflow_call`, `helper_call`.

## Proposed Design

### 1. Introduce an output protocol mode

Add an explicit runtime mode, likely derived from model config:

```json
{
  "toolProtocol": "block" | "native"
}
```

Default should remain `block` to avoid breaking current behavior.

This mode should affect:

- prompt generation
- schema/tool construction
- driver request shape
- session message rendering
- model event handling
- result continuation format

### 2. Split prompt generation by protocol

Keep `generate_prompt(None)` as the public method, but internally branch:

- block mode: current behavior.
- native mode: evaluate only the configured prompt expression and binding context behavior; do not append `generate_block_protocol_prompt_with_binding_rules`.

Bindings can remain as runtime-rendered user messages for now, but the binding text should be reviewed. It currently uses `[binding]` blocks and `@@symbols`, which is less severe than block tool syntax but still prompt-protocol-ish. If native mode wants zero custom protocol, bindings should become a plain context message or provider metadata in a later pass.

### 3. Replace `native_schema.rs` with a schema normalization layer

Make native schema generation use the same IR normalization helpers block mode already depends on:

- `flatten_named_field_specs`
- `flatten_helper_input_specs`
- `flatten_output_specs`
- `unflatten_object`

For native function calling, there are two viable choices:

- Preserve nested JSON Schema and pass nested args to execution.
- Use the same flat aliases as block mode and unflatten before execution.

Recommendation: use nested JSON Schema for native mode where possible, but keep a registry that maps any flattened/native aliases back to IR paths. The important part is one canonical `NativeCallableRegistry` that records:

- provider-visible name
- original IR name
- action kind: tool, workflow, helper
- input schema
- output/return schema if available
- argument alias map if flattening is used
- source description/examples if needed

This avoids relying on the function name alone to decide execution behavior.

### 4. Add native call events to the driver contract

Extend `ModelEvent` with normalized native events, for example:

```rust
NativeToolCall {
    id: Option<String>,
    name: String,
    arguments: Value,
}
NativeStructuredOutput(Value)
```

If streaming deltas are needed later, add partial events separately. The first implementation can buffer provider deltas inside the driver and emit completed calls when the provider marks them done.

OpenAI driver work:

- Send `tools` and `tool_choice`/strict options in the request body for native mode.
- Parse streamed `delta.tool_calls`.
- Emit complete native tool calls with IDs.
- Render assistant tool calls and tool responses correctly in future request messages.

Gemini driver work:

- Send `tools.function_declarations`.
- Parse `functionCall` parts from candidates/steps.
- Render `functionResponse` parts using the provider's expected structure.

### 5. Add native session turns, without breaking exported sessions

Extend `Turn` rather than replacing it. Keep existing fields for backwards compatibility and add optional native fields:

```rust
native_response: Option<NativeAssistantTurn>
native_results: Option<Vec<NativeToolResult>>
protocol: Option<"block" | "native">
```

The exported session remains a plain JSON document that can be saved the same way as today.

Native assistant turn should store:

- text content, if any
- tool calls: id, provider name, canonical IR name, action kind, args
- structured final output, if any

Native tool result should store:

- call id, when provider requires one
- canonical name
- action kind
- args
- result

Then `to_messages` should become protocol-aware:

- block mode emits the current `Message` sequence.
- native mode emits provider-neutral messages containing assistant tool calls and tool results.

The provider-specific conversion should stay in the driver, not in session state.

### 6. Normalize native calls into existing execution

Add a method like:

```rust
enqueue_native_call(call: NativeToolCall)
```

It should:

1. Look up the call in `NativeCallableRegistry`.
2. Decode/unflatten arguments if needed.
3. Push the equivalent pending intent:
   - `("tool_call", { "type": ir_name, "args": args })`
   - `("workflow_call", { "type": ir_name, "args": args })`
   - `("helper_call", { "type": ir_name, "args": args })`
4. Preserve the provider call ID outside the intent so results can be attached to the next native provider message.

`process_intents` can remain the execution center. The adaptation layer should sit before it.

### 7. Native final output

For `ir.output`, generate provider-native structured output schema separately from callable tools.

Do not model final output as a tool unless the target provider lacks structured output support. If fallback is needed, define a synthetic native function name such as `__auwgent_final_output`, then normalize it into `response_schema`.

Native final output should emit the same external intent/JSONL shape that block mode emits for `response_schema`, so SDK users do not need a separate callback path.

### 8. Preserve middleware behavior

Middleware currently sees intents and run lifecycle events. Native mode should keep those hooks stable:

- Native tool call -> same `tool_call` middleware payload.
- Native workflow call -> same `workflow_call` middleware payload.
- Native helper call -> same `helper_call` middleware payload.
- Native final output -> same `response_schema` or `response_text` payload.

Provider raw blocks will not exist in native mode, so `_raw` should be optional and absent.

## Implementation Phases

### Phase 1: Schema correctness tests

Before wiring provider calls, add tests for `native_schema.rs` or its replacement:

- primitive params
- optional required list
- nested object params
- array of objects
- `typeRef`
- helper null/missing input defaults
- helper `kind: direct`
- output schemas and output variants
- workflow params
- duplicate names across tool/workflow/helper

Expected output should assert JSON Schema and registry metadata, not just that JSON exists.

### Phase 2: Runtime protocol switch

Add protocol mode and make prompt generation skip `intents.rs` in native mode.

At this point native mode can still be disabled for actual provider runs until drivers are ready. The test should prove the native prompt does not contain block syntax.

### Phase 3: Driver/model event contract

Add native model events and provider request options. Start with OpenAI-compatible chat completions because the current OpenAI driver already exists and can send arbitrary config fields.

Tests should use mocked streams rather than live API calls.

### Phase 4: Native session representation

Extend session export/import with optional native fields. Add round-trip tests that:

- save a session after assistant tool calls
- import it
- continue with tool results
- verify provider messages can be reconstructed

### Phase 5: Execution adapter

Implement native-call-to-intent normalization and feed it into `process_intents`.

Tests should prove:

- native tool calls execute the registered tool callback
- native workflow calls execute workflow bodies
- native helper calls enter helper execution
- multiple native calls in one assistant turn execute concurrently like current block actions
- middleware skip/override still works

### Phase 6: Native final output

Add output schema support and normalize provider structured output to `response_schema`.

Tests should compare block and native external intent payloads for the same logical output.

## Main Risks

- Provider session replay is stricter than block mode. OpenAI-style tool results must reference the correct tool call ID.
- Name collisions across tools, workflows, and helpers are currently hidden by block type. Native function names need namespacing or a registry.
- Helper input semantics are easy to break because block mode has special default/direct/properties handling.
- Output schemas are separate from function calling and should not be collapsed into tools too early.
- Existing exported sessions must remain importable.

## Recommended First Code Change

Do not start by changing OpenAI/Gemini drivers. First make `native_schema.rs` produce a full `NativeCallableRegistry` and add focused tests around IR coverage.

That creates a stable foundation for the rest of the native path and will quickly expose whether the schema layer truly captures everything the IR returns: inputs, outputs, params, helper inputs, workflows, custom type refs, and optional fields.
