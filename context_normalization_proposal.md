# Proposal: Context Normalization (Opt-in)

## 1. Overview

This document outlines a standardized approach for transforming raw JSON conversation history into a structured, single-string format before sending it to the LLM.

This is an **opt-in** feature designed for models that perform better with a structured text "View" of the conversation than with a native JSON message array.

## 2. Core Concept: Strict 1:1 Normalization

The "Context Compiler" acts as a pure transformation layer. It takes the exact JSON structure (turns) and reformats them using a consistent set of delimiters.

### Input (Storage JSON)
```json
[
  { "role": "user", "content": "Turn 1 content..." },
  { "role": "assistant", "content": "Turn 2 response..." },
  { "role": "user", "content": "Turn 3 content..." }
]
```

### Output (Execution Format)
```text
# RECENT CONVERSATION
[USER]: Turn 1 content...
[ASSISTANT]: Turn 2 response...

# CURRENT TASK
[USER]: Turn 3 content...
```

## 3. Implementation Plan

- **New Crate**: `auwgent-context`
- **Primary Function**: `normalize_session(session: Session) -> String`
- **Execution**: The resulting string is injected into the prompt as a single turn (usually a "user" role) at the end of the final API call.

## 4. Why this matters (Phase 1)

- **Predictability**: 1:1 mapping ensures no data loss or "black box" behavior.
- **Attention Steering**: Explicitly labeling the "Latest" turn as the `# CURRENT TASK` helps models stay focused on the user's immediate goal.
- **Portability**: Allows the framework to adapt to APIs that have limited support for complex message arrays.
