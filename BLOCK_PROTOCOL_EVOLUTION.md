# Block Protocol Evolution (V2)

This document outlines the findings from our research into model friction with the `@@` delimiter and proposes a more natural, approachable syntax for intent extraction.

## Findings: The "@@" Friction

While the `@@` protocol is token-efficient and streaming-friendly, it introduces several "friction points" for modern LLMs:

1.  **Alignment Mismatch**: Models like Llama-3, Gemini, and Claude are heavily fine-tuned on XML-like tags (`<tag>`) and Markdown. Custom delimiters like `@@` require the model to "learn a new dialect" on the fly, increasing the chance of syntax errors (e.g., forgetting `@@end` or adding a space after `@@`).
2.  **Context Switching**: The rigid structure of `@@chat` -> `@@tool` -> `@@end` feels "boxed in." It discourages the model from reasoning *within* or *near* the action, often leading to sparse reasoning or "robotic" output.
3.  **Schema Ceremony**: Forcing every reasoning block to have a field like `explain = "..."` creates quoting issues and unnecessary verbosity.

---

## Proposal: Approachable Tag-Based Namespacing

We propose moving to a **Tag-Aligned** syntax that feels more native to the model's training while maintaining the streaming benefits of the block protocol.

### 1. Explicit Text Output (`<response_text>`)
Normal conversational output should be explicit and unambiguous, instead of overloading `<thought>`.

**Syntax:**
```markdown
<response_text>
Hello! How can I assist you today?
</response_text>
```

### 2. Structured Blocks With Open/Close Pairs
All non-text intents should use explicit open/close blocks with typed headers.

**Schema output:**
```markdown
[schema: Output]
success: true
data: "Greeting delivered"
message: "User greeted successfully"
[/schema]
```

**Tool call:**
```markdown
[tool_call: db_query_products]
filter: "name:Model X"
[/tool]
```

**Workflow/helper call follow the same style:**
- `[workflow_call: type] ... [/workflow]`
- `[helper_call: type] ... [/helper]`

### 3. Custom Intent Block
Custom intents should also be namespaced and explicit.

**Syntax:**
```markdown
[custom: SpeakLoud]
explain: "Checking inventory before alerting manager."
[/custom]
```

---

## Summary of Changes

| Feature | V1 (Current) | V2 (Proposed) |
| :--- | :--- | :--- |
| **Text Response** | `@@chat ... @@end` | `<response_text> ... </response_text>` |
| **Schema Output** | `@@out [Schema] ... @@end` | `[schema: Schema] ... [/schema]` |
| **Action Header** | `@@tool` / `@@workflow` / `@@helper` | `[tool_call: type] ... [/tool]` and equivalents |
| **Call Syntax** | `name(arg=val)` | `arg: value` (Keys on new lines) |
| **Custom Intent** | `@@IntentName ... @@end` | `[custom: IntentName] ... [/custom]` |
| **Delimiters** | `@@marker` ... `@@end` | XML-like tags + bracketed paired blocks |

## Implementation Path

1.  **Update `BlockScanner.rs`**: Add support for `<response_text>...</response_text>` and paired bracket blocks like `[tool_call: ...] ... [/tool]`.
2.  **Update `BlockOrchestrator.rs`**: Map new headers to intents (`response_text`, `response_schema`, `tool_call`, `workflow_call`, `helper_call`, and custom intents).
3.  **Refactor `intents.rs`**: Update the system prompt generator to use the V2 syntax.
4.  **Define strict close-token rules**: Validate correct closing tags (`[/tool]`, `[/schema]`, `[/custom]`) and recover gracefully on malformed blocks.

---

## Prompt Assembly Policy Matrix (Scalable Mode)

This matrix keeps prompts flexible while preventing unnecessary protocol noise.

| User Request Mode | Enabled Blocks in Prompt | Disabled/Omitted Blocks | Goal |
| :--- | :--- | :--- | :--- |
| **Plain Q&A** | `<response_text>` | Tool/workflow/helper/custom usage details | Keep output natural and simple |
| **Structured Output Needed** | `<response_text>`, `[schema: Name]...[/schema]` | Tool/workflow/helper/custom usage details | Enforce schema accuracy |
| **Data Lookup / Action** | `<response_text>`, `[tool_call: type]...[/tool]` | Unused workflow/helper/custom details | Enable grounded retrieval/actions |
| **Complex Orchestration** | `<response_text>`, tool + workflow/helper blocks | Irrelevant custom intent details | Support multi-step execution |
| **Reasoning Trace Enabled** | `<response_text>`, `[custom: type]...[/custom]` | Unused tools/workflows if not needed | Add explainability without clutter |

### Runtime Prompt Assembly Rules

1. **Always include core contract**
   - valid block syntax
   - proper close tags
   - no fabricated tool results
2. **Conditionally include capabilities**
   - inject only blocks relevant to current request mode
3. **Inject compact registries, not full verbose docs**
   - one-line entries per tool/custom intent
4. **Include at most one short example per enabled block type**
   - avoids bloated prompts at scale

---

## Minimal Registry Format (Current Constraint)

Right now we should assume:
- no grouping by domain
- no nested capability taxonomy
- prompt must stay minimal for small models

So the registry should be flat, linear, and short.

### Custom Intent Catalog

```markdown
Custom intents available:
- Thoughts(explain: string) -> explicit reasoning trace only
```

### Tool Catalog

```markdown
Tools available:
- get_name(id: string) -> returns user name for the given id.
```

After listing capabilities, include concrete call examples:

```markdown
Tool call example:
[tool_call: get_name]
id: "42"
[/tool]

Custom intent example:
[custom: Thoughts]
explain: "Need lookup before final response."
[/custom]
```

---

## Minimal Prompt Skeleton (Dynamic)

```markdown
You are an assistant that must emit valid protocol blocks.

Core rules:
- Use only enabled blocks listed below.
- Close every block correctly.
- Never invent tool results.
- If no external action is needed, answer with <response_text>.

Enabled blocks for this turn:
- <response_text>...</response_text>
- [tool_call: type]...[/tool]
- [custom: type]...[/custom]

Custom intents available:
- Thoughts(explain: string)

Tools available:
- get_name(id: string)

Examples:
Tool:
[tool_call: get_name]
id: "42"
[/tool]

Custom:
[custom: Thoughts]
explain: "Need lookup before final response."
[/custom]
```

This structure stays readable even with many tools/intents because each capability is one line and only relevant sections are injected per turn.

---

## Small-Model Prompting Principles

For smaller models, prompt quality comes more from **clarity and tightness** than from rich documentation.

### Core Rules

1. **Use the shortest valid contract**
   - do not explain every block in long prose
   - define the syntax once, then list allowed capabilities
2. **Prefer linear lists over rich formatting**
   - flat tool/custom intent registries are easier to follow than large grouped sections
3. **Show one exact example per enabled capability type**
   - one tool example
   - one custom intent example
   - one schema example if needed
4. **State what to do first**
   - if no external action is needed, answer with `<response_text>`
5. **State what not to do in one short block**
   - do not invent tools
   - do not invent args
   - do not emit disabled block types
6. **Avoid repeating capability names too many times**
   - repeated listings increase confusion for weaker models

### Recommended Prompt Shape

The most reliable minimal shape is:

```markdown
Role / behavior

Core rules

Enabled blocks for this turn

Custom intents available

Tools available

Examples
```

### Recommended Ordering

Use this exact order:

1. **One-sentence role**
2. **Four to six hard rules**
3. **Enabled block syntax**
4. **Flat capability list**
5. **One short example for each enabled non-text block**

This order helps small models because it answers:
- what am I
- what must I obey
- what forms can I emit
- what names are legal
- what does a correct call look like

### Minimal Rule Set

```markdown
Rules:
- Use only the block types enabled below.
- If no external action is needed, answer with <response_text>.
- If a tool is needed, call only a listed tool with listed args.
- Do not invent tools, custom intents, schemas, or fields.
- Close every block correctly.
```

### What To Avoid

- long motivational prose
- repeated explanation of the same syntax
- multiple competing examples for the same tool
- giant schema dumps when schema is not needed
- mentioning tools when the current turn is text-only

---

## Exact Minimal Prompt Templates

These templates are written for small models: short, strict, and mode-specific.

### 1. Text-Only Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use only <response_text>.
- Do not call tools, workflows, helpers, schemas, or custom intents.
- Close the block correctly.

Allowed block:
<response_text>
your reply here
</response_text>

Example:
<response_text>
Hello! How can I help you today?
</response_text>
```

### 2. Tool Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use <response_text> if no external action is needed.
- Use [tool_call: type] only for listed tools.
- Use only listed args for each tool.
- Do not invent tools or args.
- Close every block correctly.

Allowed blocks:
- <response_text>...</response_text>
- [tool_call: type]...[/tool]

Tools available:
- get_name(id: string)

Example tool call:
[tool_call: get_name]
id: "42"
[/tool]

Example text response:
<response_text>
Hello! How can I help you today?
</response_text>
```

### 3. Workflow Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use <response_text> if no workflow is needed.
- Use [workflow_call: type] only for listed workflows.
- Use only listed args for each workflow.
- Do not invent workflows or args.
- Close every block correctly.

Allowed blocks:
- <response_text>...</response_text>
- [workflow_call: type]...[/workflow]

Workflows available:
- create_report(user_id: string, include_history: boolean)

Example workflow call:
[workflow_call: create_report]
user_id: "42"
include_history: true
[/workflow]

Example text response:
<response_text>
Hello! How can I help you today?
</response_text>
```

### 4. Custom Intent Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use <response_text> for normal replies.
- Use [custom: type] only for listed custom intents.
- Use only listed fields for each custom intent.
- Do not invent custom intents or fields.
- Close every block correctly.

Allowed blocks:
- <response_text>...</response_text>
- [custom: type]...[/custom]

Custom intents available:
- Thoughts(explain: string)

Example custom intent:
[custom: Thoughts]
explain: "Need more reasoning before final response."
[/custom]

Example text response:
<response_text>
Hello! How can I help you today?
</response_text>
```

### 5. Tool + Workflow + Custom Intent Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use <response_text> if no action is needed.
- Use [tool_call: type] only for listed tools.
- Use [workflow_call: type] only for listed workflows.
- Use [custom: type] only for listed custom intents.
- Use only listed args or fields.
- Do not invent names or fields.
- Close every block correctly.

Allowed blocks:
- <response_text>...</response_text>
- [tool_call: type]...[/tool]
- [workflow_call: type]...[/workflow]
- [custom: type]...[/custom]

Custom intents available:
- Thoughts(explain: string)

Tools available:
- get_name(id: string)

Workflows available:
- create_report(user_id: string, include_history: boolean)

Examples:

Tool:
[tool_call: get_name]
id: "42"
[/tool]

Workflow:
[workflow_call: create_report]
user_id: "42"
include_history: true
[/workflow]

Custom:
[custom: Thoughts]
explain: "Need lookup before deciding next step."
[/custom]

Text:
<response_text>
Hello! How can I help you today?
</response_text>
```

### 6. Schema Output Mode

```markdown
You are an assistant that must respond using valid protocol blocks.

Rules:
- Use <response_text> if a plain text answer is enough.
- Use [schema: Name] only for listed schemas.
- Use only listed fields.
- Do not invent schema names or fields.
- Close every block correctly.

Allowed blocks:
- <response_text>...</response_text>
- [schema: Name]...[/schema]

Schemas available:
- UserSummary {
    success: boolean
    data: {
      id: string
      name: string
      profile: {
        age: number
        city: string
      }
    }
    message: string
  }

Example schema response:
[schema: UserSummary]
success: true
data:
  id: "42"
  name: "Amihere"
  profile:
    age: 28
    city: "Lagos"
message: "User found successfully."
[/schema]

Example text response:
<response_text>
I found the user successfully.
</response_text>
```

### 7. Nested Field Style

Nested schema or intent fields should use indentation consistently:

```markdown
[schema: UserSummary]
success: true
data:
  id: "42"
  name: "Amihere"
  profile:
    age: 28
    city: "Lagos"
message: "User found successfully."
[/schema]
```

For arrays:

```markdown
[schema: UserOrders]
success: true
orders:
  - id: "ord_1"
    amount: 120
  - id: "ord_2"
    amount: 80
message: "Orders fetched successfully."
[/schema]
```

This keeps nested content readable for both the model and the parser.

### 8. Selection Rule

Use the smallest matching template:

- text-only turn -> Text-Only Mode
- tool-enabled turn -> Tool Mode
- workflow-enabled turn -> Workflow Mode
- custom-intent-enabled turn -> Custom Intent Mode
- schema-required turn -> Schema Output Mode
- mixed-capability turn -> Tool + Workflow + Custom Intent Mode

Do not use the mixed template unless the turn actually needs mixed capabilities.

---

## Compiler-Level Flattening Strategy

Nested fields are a prompt burden for small models. Since the compiler already knows the full shape at compile time, the model should not be responsible for reconstructing nesting.

### Finding

The compiler/runtime already has structured shape metadata for:
- tool params
- workflow params
- custom intent fields
- output schemas
- referenced custom types

This means flattening can be deterministic and does not need to rely on parser guesses.

### Recommendation

Use **compiler-owned flat aliases** in prompts and examples, then rebuild the real nested JSON shape in the interpreter/runtime.

### Why This Is Better

- reduces indentation errors
- removes nesting burden from small models
- keeps examples short
- avoids ambiguous multiline structures
- improves parse reliability

### Do Not Infer Nesting From Raw Key Names

Do not reconstruct by splitting keys like:

```text
name_name
profile_email
```

because the runtime cannot know whether a key is:
- a literal flat field name, or
- a nested path alias

### Correct Approach

Generate an explicit alias map at compile time.

```text
prompt key -> canonical path
user_profile_name -> ["user", "profile", "name"]
user_profile_email -> ["user", "profile", "email"]
```

The prompt shows only the flat keys.  
The runtime rebuilds the nested object from the canonical path map.

### Example

Instead of asking the model to emit:

```markdown
[tool_call: create_user]
profile:
  name: "Ada"
  contact:
    email: "ada@test.com"
[/tool]
```

use:

```markdown
[tool_call: create_user]
profile_name: "Ada"
profile_contact_email: "ada@test.com"
[/tool]
```

and reconstruct internally into:

```json
{
  "profile": {
    "name": "Ada",
    "contact": {
      "email": "ada@test.com"
    }
  }
}
```

### Suggested Alias Metadata

For each prompt-visible field, keep:

- flat key
- canonical path
- type
- optional/required flag
- description

### Scope

This strategy should work for:
- tool arguments
- workflow arguments
- custom intent payloads
- schema outputs if needed later

### Final Rule

The alias is not the source of truth.  
The compiler-known canonical path is the source of truth.

---

## Example Injection Strategy (User-Provided Teaching Examples)

When users provide examples to teach calling style, include them as a separate, bounded section.

### Canonical Format

```markdown
Examples (follow style, do not copy content blindly):

[Example 1]
User: "what is user 42 name?"
Assistant:
[tool_call: get_name]
id: "42"
[/tool]

[Example 2]
User: "explain why you are calling the tool"
Assistant:
[custom: Thoughts]
explain: "Need id lookup before composing final response."
[/custom]
```

### Rules for Example Usage

1. **Prioritize contract over examples**
   - syntax and allowed capability registry always win
2. **Only inject relevant examples**
   - if no tool use is enabled for this turn, omit tool examples
3. **Keep examples short**
   - 1–3 examples max per turn to avoid prompt bloat
4. **Separate style from facts**
   - examples teach format, not factual truth
5. **Guard against conflict**
   - if example conflicts with current tool/custom registry, ignore conflicting parts

### Example Selection Heuristic

- If mode is text-only: include only `<response_text>` examples
- If mode needs tools: include one tool-call example matching current tool shape
- If mode enables custom reasoning intent: include one `[custom: type]` example
- If mode needs schema: include one `[schema: Name]...[/schema]` example
