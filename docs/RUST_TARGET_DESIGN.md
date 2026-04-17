# Rust Target Design

This document captures the intended public shape of the Rust target.

The goal is to mirror the existing generated target contract used by TypeScript,
Python, and Dart, while still allowing Rust consumers to use more idiomatic
patterns where that does not change the public model.

## Source Of Truth

The Rust target should follow the same generated composition model already used
in other targets.

- Users should not manually load or pass IR in normal generated usage.
- Generated code should embed or load the compiled IR internally.
- The generated `Config` type is the composition root.
- Public aliases should follow the `Auwgent...` pattern.
- Conditional fields should remain conditional.

Rust-specific ergonomics must not change those rules.

## Public Entry Point

Generated code should expose:

- `create_<agent>()` style factory
- `auwgent` alias pointing at the generated factory
- `AuwgentConfig`
- `AuwgentAgent`
- `AuwgentTools`
- `AuwgentMiddleware`
- `AuwgentContext`
- `AuwgentApiKeys` when keys are required

Example shape:

```rust
let agent = auwgent(AuwgentConfig {
    api_keys: AuwgentApiKeys {
        openai_api_key: openai_key,
    },
    tools: MyTools,
    middleware: vec![MyMiddleware],
});
```

This is only illustrative. The exact field names should be generated from the
same conditions used in the existing targets.

## Conditional Config Generation

Generated Rust config must follow the IR in the same way as TypeScript and Dart.

- If the agent has tools, generate `tools`.
- If the agent has no tools, do not generate a `tools` field in the public config.
- If the agent has context, generate `context`.
- If the agent has no context, do not generate a `context` field.
- If the agent requires provider keys, generate `api_keys`.
- If no provider keys are required, do not generate `api_keys`.
- `middleware` remains part of config, matching the existing target family.

Do not use default-filling patterns to simulate conditional generation.

Avoid API shapes like:

- `..Default::default()`
- `library_path`
- mandatory `context: None` when no context exists

Those are not part of the intended native Rust target story.

## Intent Consumption

Rust should support the same conceptual intent-consumption paths as the other
targets:

1. direct generic intent hooks
2. generated typed handler wrappers
3. middleware lifecycle hooks

### Direct Hooks

The Rust analogue of `onIntent(...)` and `onIntentPartial(...)` should exist.

This is the lower-level public consumption path and should support a closure +
`match` style.

Example:

```rust
agent.on_intent(|name, value, agent_name| {
    match name {
        AuwgentIntentName::ResponseText => {
            let intent = AuwgentResponseTextIntent::from_json(&value)?;
        }
        AuwgentIntentName::ToolCall => {
            let intent = AuwgentToolCallIntent::from_json(&value)?;
        }
        _ => {}
    }

    Ok(None)
});

agent.on_intent_partial(|name, value, agent_name| {
    match name {
        AuwgentIntentName::ResponseText => {
            let intent = sdk::PartialTextIntentValue::from_json(&value)?;
            print!("{}", intent.delta.unwrap_or_default());
        }
        _ => {}
    }

    Ok(())
});
```

The important rule is that the generated name/value space should be exhaustive
for the generated agent surface.

### Typed Handler Wrappers

Rust may also expose generated typed wrappers that mirror the Dart target style.

These should be convenience APIs layered on top of the generic intent hooks.

Example shape:

```rust
agent.on_intent_handler(MyHandler);
agent.on_intent_partial_handler(MyPartialHandler);
```

These wrappers should dispatch through generated exhaustive matching over the
known intent set.

## Exhaustiveness

Generated Rust should lean into exhaustiveness wherever the IR allows it.

That applies especially to:

- generated intent-name enums
- generated tool/helper/workflow intent enums
- generated typed handler dispatch

If the IR defines a finite set of intent variants, the generated Rust layer
should prefer enums and exhaustive `match` dispatch over free-form strings in
the typed surface.

The lower-level runtime callback path may still carry names and raw JSON values,
but generated wrappers should turn those into exhaustive typed enums.

## Middleware

Middleware must follow the existing target model, especially the TypeScript
middleware lifecycle shape.

That means the Rust target should model generic lifecycle hooks such as:

- `on_run_start`
- `on_llm_start`
- `on_intent`
- `on_intent_partial`
- `on_llm_end`
- `on_run_complete`
- `on_error`

Do not generate middleware APIs as per-intent typed methods like:

- `tool_call(...)`
- `partial_response_text(...)`

Those were considered and rejected because they do not match the current target
family contract.

Typed intent handling belongs in handler wrappers and helper decoding utilities,
not in the middleware trait shape.

## Internal vs Public Shape

The native Rust implementation may use whatever internal representation is
necessary:

- boxed trait objects
- arcs
- enums
- dynamic dispatch
- async internals

But those choices should remain internal unless they are required by the public
contract already established in other targets.

The public generated API should remain aligned with:

- generated config composition
- generated aliases
- generic intent hooks
- optional typed handler wrappers

## Non-Goals For First Rust Target

The first public Rust target should not lead with:

- manual IR loading as the default generated usage
- a stream-first public API
- FFI-only fields like `library_path`
- Rust-default filler ergonomics that bypass conditional generation

Those can exist as internal or advanced layers later, but they should not define
the generated target story.
