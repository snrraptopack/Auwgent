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

### 1. Free-Form Reasoning (`<thought>`)
Instead of a custom `@@SpeakLoud` intent, use a dedicated, bare-content reasoning block.

**Concise & Clear:**
```markdown
<thought>
I need to query the products first to see if 'Model X' is in stock.
Once I have the ID, I'll check orders.
</thought>
```

### 2. Functional Call Blocks (`tool_call`, `helper_call`)
Instead of `@@tool` followed by a function-style call, use a **Namespaced Header** that identifies the call type and target directly.

**Syntax:** `[type: name]`
- `[tool_call: name]`
- `[helper_call: name]`
- `[workflow_call: name]`

**Example:**
```markdown
[tool_call: db_query_products]
filter: "name:Model X"
```

### 3. Integrated Multi-Reasoning
By using tags, the model can intersperse reasoning and actions more naturally without the "Schema Ceremony."

**Example Flow:**
```markdown
<thought>
Checking inventory...
</thought>

[tool_call: db_query_products]
filter: "all"

<thought>
Now that I see 'Laptop' is low on stock, I'll alert the manager.
</thought>

[tool_call: send_alert]
target: "manager"
message: "Low stock on Laptop"
```

---

## Summary of Changes

| Feature | V1 (Current) | V2 (Proposed) |
| :--- | :--- | :--- |
| **Reasoning** | `@@SpeakLoud(explain="...")` | `<thought> ... </thought>` |
| **Action Header** | `@@tool` | `[tool_call: name]` |
| **Call Syntax** | `name(arg=val)` | `arg: value` (Keys on new lines) |
| **Delimiters** | `@@marker` ... `@@end` | `[type: name]` or `<thought>` |

## Implementation Path

1.  **Update `BlockScanner.rs`**: Add support for `[...]` and `<...>` style markers.
2.  **Update `BlockOrchestrator.rs`**: Handle the new `tool_call: name` mapping logic.
3.  **Refactor `intents.rs`**: Update the system prompt generator to use the V2 syntax.
4.  **Simplify DSL**: Allow "Bare String" content for reasoning blocks so no keys are required.
