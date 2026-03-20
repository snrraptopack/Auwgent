# Intent Format Proposal — Function Composition DSL

**Author:** Amihere  
**Status:** RFC — Pre-implementation Review  
**Replaces:** YAML-based intent format

---

## Background

The current intent architecture uses a YAML-based format for LLM output. After extensive stress testing, YAML was identified as fundamentally problematic for this use case due to:

- Whitespace and indentation sensitivity making streaming parsing fragile
- Block scalar complexity (`|`, `>`, `|-`, `|+`, `|2`) creating significant parser surface area
- Colon-in-value ambiguity causing false intent key detections mid-stream
- Multiline content requiring the model to remember scalar indicators consistently across long generations — which it doesn't reliably do

The proposed replacement eliminates these problems entirely by shifting to a function composition format with string literal values.

---

## Proposed Format

### Basic Structure

An intent is expressed as a function call. Fields inside the intent are variable assignments. String values are wrapped in double quotes.

```
intent_name(
  field_one = "value"
  field_two = "value"
)
```

### Real Examples

```
response_text(
  text = "Here is your answer, Theophilus."
)

thought(
  explain = "I need to break this down step by step.
  The user is asking about compound interest.
  Formula is A = P(1 + r/n)^nt — walking through each variable now."
)

helper_call(
  type = "StoryTeller"
  args = "Write a horror story set in Accra.
  Protagonist: a night-shift nurse named Abena.
  Tone: psychological dread, no supernatural elements."
)
```

### Multiple Intents in One Turn

```
thought(
  explain = "Theophilus is asking a creative writing question.
  Best to delegate this to StoryTeller rather than respond directly."
)
helper_call(
  type = "StoryTeller"
  args = "Write a short sci-fi story about a robot discovering emotions.
  Setting: deep-space observatory.
  Tone: poetic and introspective. Length: 300 words."
)
```

---

## Grammar

```
response       := intent+
intent         := intent_name "(" NEWLINE field* ")" NEWLINE
intent_name    := identifier
field          := identifier "=" quoted_string NEWLINE
quoted_string  := '"' content '"'
content        := any characters including newlines, escaped \" for literal quotes
identifier     := [a-zA-Z_][a-zA-Z0-9_]*
```

One formatting rule the model must follow: the closing `)` goes on its own line.

---

## Schema Definition

Intents are defined using TypeScript-style type signatures. This is familiar to the model from training data and consistent with the TypeScript SDK the parser feeds into.

```
# Intent Schema

thought(
  explain: string
)

response_text(
  text: string
)

helper_call(
  type: string
  args: string
)
```

No special multiline type annotation is needed. Because values are string literals delimited by `"`, multiline content is handled naturally — the parser accumulates everything until the closing `"` regardless of what the content looks like internally.

---

## Parser Design

### States

```
IDLE        — scanning for intent_name(
IN_SCOPE    — inside a function, scanning for key = or )
IN_VALUE    — accumulating characters until closing "
```

Three states total. This is a significant reduction from the YAML parser which required tracking scalar mode, indent depth, chomping behavior, and continuation state simultaneously.

### Field Key Detection

A line is treated as a field declaration if and only if:

1. It starts with an identifier
2. Followed by ` = "`
3. The identifier matches a known field name of the **currently open intent** in the schema

This schema-aware field detection prevents false triggers on content like:

```
thought(
  explain = "working through the math:
  A = P(1 + r/n)^nt where P = 5000
  x = 5 is an assignment in the example code"
)
```

`A`, `x`, and `P` are not known fields of `thought`, so they are absorbed as content not treated as new field declarations.

### Scope Close Detection

The `)` on its own line closes the current intent scope. Everything between `(` and the closing `)` belongs to that intent.

### Escape Handling

A literal `"` inside a value is escaped as `\"`. This is standard programming convention the model already knows and applies naturally.

---

## Why This Works Better Than YAML

| Problem in YAML | Status in new format |
|---|---|
| Indentation drift mid-generation | Irrelevant — delimiter-based not whitespace-based |
| Colon inside value triggers false key detection | Irrelevant — colons are not field delimiters |
| Block scalar modes `\|`, `>`, `\|-`, `\|+` | Eliminated entirely |
| `response_text:` inside a pipe block | Irrelevant — inside quotes, treated as content |
| Explicit indent indicator `\|2`, `\|4` | Eliminated entirely |
| `)` inside value | Handled — only `)` on its own line is a scope close |
| `=` inside value | Handled — schema-aware field detection |
| Streaming partial chunk ambiguity | Reduced — parser only needs `"` matching, not indent tracking |

---

## What the Model Sees in the System Prompt

```
# Response Format

Every response must be one or more intent calls.
Each intent is a function. Fields are variable assignments. Values are strings in quotes.

Example:

thought(
  explain = "I will break this down step by step."
)
response_text(
  text = "Here is your answer."
)

Rules:
- Closing ) goes on its own line
- Use \" inside a value if you need a literal quote
- Multiple intents in one response are allowed
- Do not add any text outside of intent calls
```

Short, concrete, uses conventions the model already knows from programming context.

---

## Edge Cases and Recovery

### Model omits closing `)`

Detectable at end of stream with an open scope. Parser closes the scope at stream end and emits what it has. No data is lost.

### Model produces unknown intent name

Intent name does not match schema. Parser can either skip and continue scanning, or flag for caller to handle. Does not crash.

### Model produces unknown field name inside a known intent

Field name not in schema for current intent. Two options:
1. Absorb as continuation of previous field value
2. Flag as a structural anomaly and surface to the re-emit layer

Option 2 is recommended for production — gives the re-emit system a precise error: "field `X` is not a known field of intent `Y`, re-emit this segment with a valid field name."

### Re-emit Recovery

When a structural anomaly is flagged, only the affected segment is sent back to the model with a targeted correction prompt. The rest of the response is preserved. This keeps recovery latency low — no full regeneration, just a surgical re-emission of the problematic segment.

---

## Open Questions for Review

1. Should the closing `)` rule be enforced strictly or should the parser also handle `)` mid-line via lookahead for rare edge cases?

2. For the re-emit recovery path — should the correction prompt show the model the raw broken segment, or only the schema definition for the affected intent?

3. Should `args` in `helper_call` remain a flat string, or is there value in defining it as a structured object `args: Record<string, string>` now while the schema is being redesigned anyway?

---

*End of proposal*