# Generative UI Notes

This note captures the first generative UI work added to Auwgent on April 3, 2026.

## What We Added

A new top-level `component` declaration now exists in the DSL and flows through:

- parser
- checker
- IR lowering
- runtime schema/types
- streaming block protocol
- editor syntax support

Components currently support:

- normal typed props
- reserved `action` fields
- reserved `children` constraints

Example DSL shape:

```agent
component Button {
    action: {
        onclick: delete | add(id: string)
    }
}
```

## Runtime Protocol

The runtime now supports component instance blocks:

```txt
[component: Button, c_id:"button_instance"]
action_onclick: delete
[/component]
```

and render blocks:

```txt
[render_component]
root: "button_instance"
[/render_component]
```

Important protocol decisions so far:

- `c_id` is required for emitted component instances
- `c_id` lives in the block header, not in the component signature
- component actions are presented in callable form like `delete(id: string)`
- component action values should stay readable, for example `action_onclick: delete(id: "123")`

## Model Presentation

The current intended model-facing style is compact, similar to how tools are presented:

```txt
Components available:
- Button(action_onclick: delete | add(id: string))
```

This is preferred over dumping raw IR JSON into the prompt.

## Current Limits

Some design questions are still intentionally unresolved:

- component scoping across files and helper contexts
- how component visibility should be selected into a compiled context
- whether same-file components should be implicitly available to helpers
- how global vs target-specific component files should be routed

For now, the implementation work should be treated as an initial foundation rather than the final scoping model.

## Future Direction

In the future, we plan to make the streaming UI output interoperable with Vercel's JSON render format so Auwgent-generated component output can be translated into a more standard render tree shape when needed.

That interop work should be done without giving up Auwgent's current strengths:

- strict block protocol parsing
- streaming-safe partial handling
- constrained component declarations from the DSL
- runtime reconstruction of structured UI output
