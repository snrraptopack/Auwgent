# Intent Parser Evolution: Scaling Beyond Vercel's `json-render`

This document outlines the architectural advantages of the current Auwgent Intent Parser and the roadmap for supporting **Streaming Forward References**, positioning it ahead of standard "Partial JSON" repair strategies used by Vercel's AI SDK.

## Current Architecture vs. Vercel `json-render`

### Vercel / Partial JSON Strategy
*   **Mechanism**: Synthetically closes truncated JSON (e.g., adding `}` or `]`) to make it parsable by standard JSON tools.
*   **The Re-parsing Problem**: To keep the UI updated during a stream, the entire JSON blob must be repaired and re-parsed repeatedly. This is $O(n^2)$ and computationally expensive for large streams.
*   **Implicit References**: Usually relies on full object arrival before references can be resolved.

### Auwgent's Event-Driven YAML Strategy
*   **Mechanism**: Uses a custom **Streaming Tokenizer** and **Frame-based Parser** (`ir-runtime/src/intent_parser/`).
*   **Advantage**: Because it uses indentation-based syntax, the parser naturally handles incomplete blocks without "repairing" them. It waits for the next valid indent level.
*   **Discrete Intent Events**: Instead of one giant JSON object, Auwgent emits individual `Intent` events. This avoids the $O(n^2)$ problem because we only ever parse the *current* active intent block.

## Future Improvement: Streaming Forward Referencing

Auwgent's `IRBuilder` is currently atomic, but the architecture is positioned for **Graph-based Incremental Resolution**:

### 1. Deferred Reference Inlining
Instead of resolving `$ref` strictly when an object is built, the SDK should support **Observable Refs**.
*   When a `ref: product-123` is encountered but the object with `id: product-123` hasn't arrived yet, the UI renders a "loading" or "placeholder" state.
*   The parser maintains a "Resolution Registry" of unresolved refs.

### 2. Live Graph Updates
As soon as the LLM outputs the object with the matching `id`, the Registry triggers a reactive update. Any UI components previously rendered as holes or placeholders are instantly hydrated with the newly arrived data.

### 3. Cross-Intent Referencing
One semantic block (e.g., a `summary` intent) can refer to structural data in a completely different block (e.g., a `data-table` intent) even if the second block appears much later in the stream.

## Implementation Roadmap
- [ ] **Reactive Registry**: Move the `IRBuilder` registry from a static HashMap to a reactive store that notifies the SDK of new ID arrivals.
- [ ] **Partial Ref Hydration**: Update the TypeScript and Python SDKs to return "Future" or "Proxy" objects for unresolved refs.
- [ ] **Incremental Schema Validation**: Allow validating parts of a schema even if the parent structure is still open.

---
*Created on 2026-03-15 as a design guide for the next major iteration of the Auwgent Runtime.*
