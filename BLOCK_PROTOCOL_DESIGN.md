# Block-Based Protocol Design

## Overview

A lightweight multi-modal protocol for LLM responses that uses explicit `@@marker` delimiters to separate different types of content. This eliminates parsing ambiguity and allows models to freely mix conversational text, tool calls, and structured output in a single response.

## Core Principle

The `@@` marker is the ground truth. The parser knows from the first token what mode it's entering - no lookahead, no bracket counting, no ambiguity.

---

## Block Types

### 1. Chat Block (`@@chat`)

**Purpose**: Conversational text to display to the user

**Syntax**:
```
@@chat
Let me fetch that data for you.
@@end
```

**Mapping**: Maps to `response_text` intent

**Rules**:
- Content is passed through as-is (plain text)
- Can contain multiple lines
- No parsing of content needed

---

### 2. Tool Block (`@@tool`)

**Purpose**: Execute one or more tool calls in parallel

**Syntax**:
```
@@tool
fetch_session(session_id = "sess_123")
get_user(user_id = "usr_123")
calculate(x = 5, y = 10)
@@end
```

**Mapping**: Each function call maps to a `tool_call` intent

**Rules**:
- Can contain multiple tool calls (one per line or separated)
- Each call uses function syntax: `tool_name(arg1 = "value", arg2 = "value")`
- All tools in the block execute in parallel
- Content is parsed using the existing function parser

---

### 3. Workflow Block (`@@workflow`)

**Purpose**: Execute a single workflow

**Syntax**:
```
@@workflow
process_data(input = "...", config = {...})
@@end
```

**Mapping**: Maps to `workflow_call` intent

**Rules**:
- Contains exactly ONE workflow call per block
- Uses function syntax: `workflow_name(args)`
- Workflows are sequential operations (not parallel like tools)
- Content is parsed using the existing function parser

---

### 4. Helper Block (`@@helper`)

**Purpose**: Delegate to a sub-agent (nested engine)

**Syntax**:
```
@@helper
SubAgent(args = {...})
@@end
```

**Mapping**: Maps to `helper_call` intent

**Rules**:
- Contains exactly ONE helper call per block
- Uses function syntax: `helper_name(args)`
- Helpers represent control flow transfer to another agent
- Content is parsed using the existing function parser

---

### 5. Output Block (`@@out`)

**Purpose**: Return structured data matching a defined schema

**Syntax**:
```
@@out ContextCompilerOutput
{session_id: "sess_123", user: {id: "usr_123", name: "Nana"}}
@@end
```

**Mapping**: Maps to `response_schema` intent with:
- `type`: The schema name (e.g., "ContextCompilerOutput")
- `response`: The parsed object

**Rules**:
- First line after `@@out` contains the schema name
- Content is a TypeScript-style object literal: `{key: value}`
- Keys do NOT require quotes (TS-style): `{session_id: "123"}` ✓
- Keys CAN have quotes (JSON-style): `{"session_id": "123"}` ✓ (also accepted)
- String values MUST have quotes: `{name: "Nana"}` ✓
- Supports nested objects and arrays
- More LLM-friendly than strict JSON (trained on TS/JS code)

---

### 6. Result Block (`@@result`)

**Purpose**: System-injected tool/workflow results fed back to the model

**Syntax**:
```
@@result
fetch_session: {"data": "...", "status": "ok"}
get_user: {"name": "Nana", "id": "usr_123"}
@@end
```

**Format**: `name: json_object`

**Rules**:
- Multiple results per block (one per line)
- Name is the tool/workflow/helper name
- Colon separator
- Right side is strict JSON (with quoted keys)
- This is system-generated, not model-generated

---

### 7. Error Block (`@@error`)

**Purpose**: System-injected errors from tool/workflow/helper execution

**Syntax**:
```
@@error
tool: fetch_session
reason: "Session not found: sess_8f3a92c1"
@@end
```

**Format**: Key-value pairs (one per line)

**Rules**:
- `tool`/`workflow`/`helper`: Name of the failed operation
- `reason`: Error message string
- Can include additional fields like `code`, `details`, etc.
- This is system-generated, not model-generated
- Model receives this and can retry, adjust, or inform user

---

## Parsing Rules

### Auto-Close Behavior

**Rule**: If a new `@@marker` is encountered while a previous block is still open (no `@@end` seen), automatically close the previous block.

**Example**:
```
@@tool
fetch_session(session_id = "sess_123")
@@chat
Here's the data.
@@end
```

Parser behavior:
1. Sees `@@tool` → opens tool block
2. Collects `fetch_session(session_id = "sess_123")`
3. Sees `@@chat` → **auto-closes tool block**, opens chat block
4. Collects `Here's the data.`
5. Sees `@@end` → closes chat block

**Result**: Two blocks extracted:
- Tool block (auto-closed): `fetch_session(session_id = "sess_123")`
- Chat block (explicit end): `Here's the data.`

### EOF Behavior

**Rule**: If EOF is reached while a block is open, treat the block as complete with whatever content was collected.

**Example**:
```
@@chat
Processing your request...
```

Parser behavior:
1. Sees `@@chat` → opens chat block
2. Collects `Processing your request...`
3. Reaches EOF → **auto-closes chat block**

---

## Multi-Block Response Example

A model can output multiple blocks in one response:

```
@@chat
Let me fetch the session data for you.
@@end

@@tool
fetch_session(session_id = "sess_8f3a92c1")
get_user(user_id = "usr_00412")
@@end

@@chat
Got it. Here's the compiled output.
@@end

@@out ContextCompilerOutput
{
  session_id: "sess_8f3a92c1",
  user: {id: "usr_00412", name: "Nana"},
  token_budget: {total: 8000, used: 3000, remaining: 5000}
}
@@end
```

**Execution flow**:
1. Display "Let me fetch the session data for you." to user
2. Execute `fetch_session` and `get_user` in parallel
3. Wait for results
4. System injects results back as `@@result` block
5. Model continues with next turn
6. Display "Got it. Here's the compiled output." to user
7. Validate and deliver the ContextCompilerOutput object

---

## Multi-Turn Flow

**Turn 1 - Model responds with chat + tool calls:**
```
@@chat
Let me run the first workflow to get the session data.
@@end

@@workflow
fetch_session_workflow(session_id = "sess_8f3a92c1")
@@end
```

**System injects results:**
```
@@result
fetch_session_workflow: {"session_data": {...}, "status": "success"}
@@end
```

**Turn 2 - Model continues:**
```
@@chat
Got it. Here's the compiled output.
@@end

@@out ContextCompilerOutput
{session_id: "sess_8f3a92c1", user: {...}}
@@end
```

---

## Parser Architecture

### High-Level Flow

```
Raw LLM Output
      ↓
BlockParser (scans for @@markers)
      ↓
Blocks: [{type, content}, ...]
      ↓
Route by type:
  - @@chat → pass through
  - @@tool → FunctionParser (multiple calls)
  - @@workflow → FunctionParser (single call)
  - @@helper → FunctionParser (single call)
  - @@out → TypeScriptObjectParser
  - @@result → ResultParser (name: json pairs)
      ↓
Intents: [{name, value}, ...]
      ↓
Engine processes intents
```

### BlockParser Responsibilities

1. **Scan for markers**: Look for `@@chat`, `@@tool`, `@@workflow`, `@@helper`, `@@out`, `@@result`, `@@end`
2. **Collect content**: Accumulate text between marker and `@@end`
3. **Auto-close**: If new marker appears before `@@end`, close previous block
4. **EOF handling**: If EOF reached, close any open block
5. **Return blocks**: Array of `{type, content, schema_name?}`

### Sub-Parser Routing

After BlockParser extracts blocks, route to specialized parsers:

- **@@tool/@@workflow/@@helper**: Use existing `FunctionParser`
  - Already handles function syntax: `name(arg = value)`
  - Already handles nested objects and arrays
  - Already supports both `=` and `:` for assignment

- **@@out**: Use new `TypeScriptObjectParser`
  - Parse TS-style object literals: `{key: value}`
  - Support unquoted keys: `{session_id: "123"}`
  - Support quoted keys: `{"session_id": "123"}`
  - Support nested structures
  - Convert to JSON for validation

- **@@result**: Use new `ResultParser`
  - Parse `name: json` pairs
  - Split on first colon
  - Parse right side as strict JSON
  - Return map of name → result

- **@@chat**: No parsing needed, pass through as string

---

## LLM-Friendliness Assessment

### Advantages

1. **Clear mode boundaries**: `@@` markers are unambiguous, unlikely to appear naturally
2. **TypeScript familiarity**: `@@out` uses TS object syntax that LLMs know well
3. **Flexible composition**: Can mix chat, actions, and output in one response
4. **Forgiving syntax**: TS objects don't require quoted keys
5. **Streaming-friendly**: Know the block type before content arrives
6. **No bracket counting**: Parser doesn't need to track nesting depth across modes

### Potential Challenges

1. **New convention**: LLMs need to learn `@@marker` syntax (not in training data)
2. **Multiple markers**: Model must remember 6 different markers
3. **Consistency**: Model might forget `@@end` or use wrong marker
4. **Token overhead**: `@@marker` and `@@end` add tokens vs implicit syntax

### Mitigation Strategies

1. **Clear system prompt**: Teach the protocol with examples
2. **Auto-close recovery**: Parser handles missing `@@end` gracefully
3. **Consistent naming**: All markers follow `@@word` pattern
4. **Minimal markers**: Only 6 markers total (chat, tool, workflow, helper, out, result, end)

---

## Implementation Phases

### Phase 1: Block Scanner
- Create `BlockParser` that scans for `@@markers`
- Implement auto-close logic
- Handle EOF gracefully
- Return raw blocks with type and content

### Phase 2: TypeScript Object Parser
- Tokenize TS object literals
- Handle unquoted keys
- Transform to JSON
- Validate and return `serde_json::Value`

### Phase 3: Result Parser
- Parse `name: json` format
- Split on first colon
- Parse JSON values
- Return map of results

### Phase 4: Block Orchestrator
- Replace or wrap `FunctionOrchestrator`
- Route blocks to appropriate parsers
- Emit intents to engine
- Handle streaming and partial blocks

### Phase 5: Engine Integration
- Update `AuwgentEngine` to use `BlockOrchestrator`
- Update result injection to use `@@result` format
- Update system prompt generation to teach block protocol
- Test multi-turn flows

### Phase 6: System Prompt Updates
- Document block protocol for models
- Provide examples of each block type
- Explain when to use each marker
- Show multi-block response patterns

---

## Open Design Decisions

### 1. Implicit Chat Text

**Question**: Should text outside blocks be treated as implicit `@@chat`?

**Example**:
```
Let me help you.

@@tool
fetch_data(id = "123")
@@end

Here's the result.
```

**Options**:
- A: Treat as implicit chat (most forgiving)
- B: Ignore text outside blocks (strict)
- C: Wrap in auto-generated `@@chat` blocks

**Recommendation**: ?

### 2. Multiple `@@out` Blocks

**Question**: Can one response have multiple `@@out` blocks?

**Recommendation**: No - only one terminal output per response (last wins)

### 3. Streaming Partial Blocks

**Question**: When should partial blocks be emitted?

**Options**:
- A: Wait for `@@end` or next marker
- B: Emit immediately when marker is seen
- C: Emit when enough content is parsed

**Recommendation**: ?

### 4. Error Recovery Strictness

**Question**: How forgiving should the parser be?

**Scenarios**:
- Missing `@@end` → Auto-close at next marker or EOF ✓
- Malformed TS object in `@@out` → Try JSON fallback? Skip block? Error?
- Malformed function call in `@@tool` → Skip that call? Error?
- Unknown marker `@@unknown` → Ignore? Treat as chat?

**Recommendation**: ?

---

## Next Steps

1. Finalize open design decisions
2. Implement BlockParser with auto-close logic
3. Implement TypeScriptObjectParser
4. Implement ResultParser
5. Create BlockOrchestrator
6. Integrate with engine
7. Update system prompts
8. Test with real LLM outputs
