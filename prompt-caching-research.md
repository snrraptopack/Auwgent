# Cache-Aware Prompt Compilation in Auwgent

## Short Version

The core point in `scratch.txt` is correct: LLM providers own the cache machinery, but frameworks decide whether requests are cache-friendly. Provider caches work best when the early prompt prefix is identical across requests. Auwgent currently weakens that property by appending runtime context into the generated system prompt before the static protocol/tool content.

The strongest product framing is:

> Auwgent should make prompt cacheability a compiler/runtime invariant, not a developer convention.

## What The Provider Docs Confirm

OpenAI says cache hits require exact prefix matches and recommends putting static instructions/examples first and variable user-specific content at the end. OpenAI prompt caching starts at 1024 input tokens, reports `cached_tokens`, and can cache messages, tool definitions, and structured output schemas.

Anthropic documents the same structural rule: cache prefixes cover `tools`, then `system`, then `messages`. For explicit caching, `cache_control` should be placed on the last block whose prefix is identical across the requests that should share the cache. A breakpoint after a timestamp or per-request context causes repeated misses. Anthropic now also has automatic caching, but explicit breakpoints still matter when there is a varying suffix.

Gemini has implicit and explicit context caching. Its docs also recommend putting large common content at the beginning of the prompt and sending similar prefixes close together.

Sources:
- OpenAI prompt caching: https://developers.openai.com/api/docs/guides/prompt-caching
- Anthropic prompt caching: https://platform.claude.com/docs/en/build-with-claude/prompt-caching
- Gemini context caching: https://ai.google.dev/gemini-api/docs/caching

## What Is Wrong In Auwgent Today

`ir-runtime/src/runtime/engine/prompt.rs` currently evaluates the configured prompt with runtime context, then appends unused context as `# ADDITIONAL CONTEXT`, then appends the static block protocol:

```rust
let prompt_val = evaluator.evaluate(&parsed_prompt, &mut scope)?;
let mut prompt = prompt_val.as_str().unwrap_or("").to_string();

prompt.push_str("\n\n# ADDITIONAL CONTEXT\n");
prompt.push_str(yaml.trim());

let intents = crate::intents::generate_block_protocol_prompt(&self.ir);
prompt.push_str(&intents);
```

That means user/session-specific values can appear before the protocol prompt, tool list, workflow list, helper list, examples, and constraints. If `age`, `user_name`, `id`, timestamps, retrieval results, or middleware metadata vary per request, the expensive static suffix stops being shareable.

The rule should be:

1. Provider/tool schemas first, in deterministic order.
2. Static system/protocol instructions next.
3. Explicit cache breakpoint at the end of the static prefix when the provider supports it.
4. Dynamic bindings/context after the cached prefix, preferably as runtime-managed synthetic content.
5. User frontier last.

## Symbolic Indirection Is Good, But Needs A Precise Contract

The proposed `@@age` idea is useful. It turns this:

```text
The person is old {{ctx.age}}
```

into this stable system text:

```text
The person is old @@age
```

and moves the value into runtime bindings:

```text
[bindings]
@@age = 10
[/bindings]
```

This preserves the system prompt prefix across users while still letting the model resolve values. The compiler can scan prompt interpolations, allocate deterministic symbol names, replace interpolations in cacheable prompt blocks, and emit a binding manifest for the runtime.

However, conditionals need stricter handling. This claim from the scratch note is too broad:

> Branch conditionals with static bodies are not a problem.

A runtime-evaluated conditional can still create multiple system prompt variants:

```text
{{#if ctx.age > 20}}
  The person is old
{{else}}
  not that old
{{/if}}
```

Even without interpolation, this produces different prefixes for different users. Low-cardinality branches may be acceptable, but they are not globally cache-stable.

A cache-safe compiler should classify prompt expressions:

| Prompt expression | Cache behavior | Recommended compiler action |
|---|---|---|
| Static literal | Stable | Keep in system prompt |
| `{{ctx.x}}` interpolation | Dynamic value | Replace with `@@x`, emit binding |
| Conditional on static config | Stable | Evaluate at compile time |
| Conditional on `ctx.x` with small finite outputs | Multiple stable variants | Either allow as cache partition or rewrite |
| Conditional on `ctx.x` with dynamic body | Dynamic | Rewrite to symbolic instruction/binding or move after cache boundary |
| Retrieved memory, time, session ids | Highly dynamic | Keep out of cached system prefix |

For a context-dependent conditional, the most cache-stable rewrite is not to choose a branch at prompt-generation time. Instead, compile the condition into stable text:

```text
Use @@age_status for age-dependent guidance.
```

with bindings:

```text
[bindings]
@@age = 10
@@age_status = "not that old"
[/bindings]
```

That keeps the system prompt stable and moves the dynamic branch result into bindings.

## The Binding Turn Design

The “floating binding cursor” is directionally right, but it should avoid accumulating stale bindings in history unless the runtime has a strong reason to retain them.

Safer design:

```text
[SYSTEM static cacheable prompt]
[HISTORY user/assistant/tool turns]
[BINDINGS current_runtime_values]
[USER current message]
```

The runtime should ensure:

1. Binding keys are deterministic and sorted.
2. Binding serialization is canonical JSON or canonical YAML.
3. Only the latest binding block is authoritative.
4. The static system prompt explicitly says symbols like `@@age` must be resolved from the latest `[bindings]` block.
5. Previous binding blocks are either removed from replayed history or tagged with turn ids and ignored when superseded.

Keeping every prior binding block can improve continuity for old turns, but it also increases token count and can create conflicts when values change. If Auwgent keeps them, the system instruction must say latest binding wins.

## Implementation Direction

The fix is bigger than moving `# ADDITIONAL CONTEXT`. It should become a prompt compilation phase.

Recommended runtime/API shape:

```rust
struct CompiledPrompt {
    static_system: String,
    binding_specs: Vec<BindingSpec>,
    cache_policy: PromptCachePolicy,
}

struct BindingSpec {
    symbol: String,
    source: ContextPath,
    render: BindingRenderMode,
}
```

Then request assembly becomes:

```text
tools/static schemas
system/static prompt + protocol instructions
cache breakpoint
history
synthetic bindings
frontier user input
```

Minimum engineering checklist:

1. Make tool/workflow/helper/schema ordering deterministic.
2. Stop appending `# ADDITIONAL CONTEXT` inside `systemPrompt`.
3. Split prompt generation into `static_system` and `dynamic_context`.
4. Add canonical binding rendering.
5. Add provider adapters for cache features:
   - OpenAI: stable prefix, optional `prompt_cache_key`, inspect `cached_tokens`.
   - Anthropic: `cache_control` at the static boundary, track cache read/write tokens.
   - Gemini: support explicit cache when available and inspect `usage_metadata`.
6. Add tests that compare generated static prompts across different context values.

## Better Discussion Framing

Use this version in docs or design discussion:

> Prompt caching is provider-side, but cacheability is framework-side. Providers can only reuse a prefix if the request begins with the same tokens. Auwgent should therefore compile prompts into two layers: a deterministic static prefix and a dynamic binding layer. The static prefix contains tools, protocol rules, schemas, and stable instructions. Any runtime value referenced by the prompt is replaced with a stable symbol like `@@age`; the actual value is sent later in a synthetic binding block. This lets Auwgent preserve semantic access to runtime context without poisoning the provider's prefix cache.

## Key Caveat

Symbolic indirection is an optimization contract, not free magic. It works best for values the model can resolve from a nearby binding block. It should not be used blindly where the exact natural-language wording is safety-critical, where hidden conditionals change policy, or where a provider-native structured/tool field expects the literal value rather than a symbolic reference.
