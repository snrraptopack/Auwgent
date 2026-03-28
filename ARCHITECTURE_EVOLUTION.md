# Auwgent Architecture: Current Boundaries and Multi-Language Direction

## Overview

Auwgent already has a substantial shared Rust core. The `ir-runtime` crate owns the canonical agent execution loop, session model, prompt generation, helper execution, stack-aware resumption, and the callback hook surfaces that language runtimes bind to.

The main architectural issue today is not that the Rust FFI layers are heavy. The Rust FFI layers for TypeScript and Python are already fairly thin. The real duplication lives one layer above them: in the host-language SDK wrappers that compose middleware behavior, manage listener lifecycle, adapt callbacks, and coordinate host-facing session/prompt policies.

This distinction matters because it changes what should be moved into Rust next and what can remain language-specific.

## Current Layering

### 1. Shared Rust Runtime

The following responsibilities already live in `ir-runtime`:

- The canonical `AuwgentEngine` execution loop
- Session import/export and the authoritative session structure
- Stack-aware resumption and fast-forward/teleportation behavior
- Helper execution and sub-engine construction
- Prompt generation for main agents and helpers
- Hook registration points for:
  - intent callbacks
  - partial intent callbacks
  - sub-engine preload/save callbacks
  - llm start/end callbacks

This means the runtime core is already more than a parser or transport layer. It already owns important orchestration behavior.

### 2. Thin Rust FFI Bindings

The TypeScript N-API binding and Python PyO3 binding are mostly thin transport layers over `EngineBridge` and `AuwgentEngine`.

They primarily do:

- JSON/value conversion between the host language and Rust
- async/future bridging
- callback bridging
- exposure of the Rust engine API to the host language

They do not contain the main middleware policy or high-level orchestration logic.

### 3. Host-Language SDK Wrappers

The TypeScript and Python SDK wrappers add a richer developer-facing surface on top of the thin FFI layers.

This is where the current duplication lives.

These wrappers currently implement:

- middleware interface definition and execution policy
- deferred listener registration and teardown
- callback shaping for object-style handlers
- partial intent post-processing
- host-side context building for middleware hooks
- host-side session preload/save coordination
- host-side stack injection and resumption policy around `run()`
- prompt interception composition before LLM execution

## What Is Actually Duplicated

### 1. Middleware Semantics

TypeScript and Python both expose a middleware model with similar hooks:

- `onRunStart`
- `onIntent`
- `onLLMStart`
- `onLLMEnd`
- `onRunComplete`
- `onError`

However, the orchestration of those hooks currently exists separately in each wrapper. Adding a new middleware hook or changing middleware ordering semantics requires touching both SDK implementations.

### 2. Listener Lifecycle

Both wrappers store user callbacks separately, then activate and deactivate native listeners around a `run()` call.

This is valid behavior, but it is duplicated behavior.

The TypeScript wrapper does this partly for ThreadSafeFunction lifecycle reasons. Python mirrors the same high-level pattern. The duplication is not in the FFI layer itself; it is in the wrapper lifecycle policy.

### 3. Host-Side Session Coordination

Rust already owns the canonical session structure and the core run loop, but the host wrappers still add their own coordination logic around that runtime:

- caching helper sessions in host memory
- synchronizing active stack state into middleware contexts
- deciding what stack to inject back into Rust on `run()`
- shaping preload/save flows for helper sub-engines

So the duplication here is not the whole session system. It is the wrapper-level session coordination layer around the Rust-owned session model.

### 4. Prompt Interception Policy

Rust already generates the base system prompt and calls the `llm_start` hook.

What is duplicated today is the wrapper policy that:

- builds the middleware context
- runs host middleware in sequence
- interprets returned prompt overrides
- synchronizes stack mutations back into the runtime call

So prompt generation is already centralized, but prompt interception semantics are not.

## What Is Not Accurately Described As Duplication

### 1. Canonical Session Model

The session structure itself is already defined in Rust. The host runtimes do not own the authoritative session schema.

### 2. Core Stack-Aware Resumption Algorithm

The fast-forward and teleportation behavior already lives in the Rust engine. The wrappers only participate by importing/exporting stack state and optionally injecting stack overrides.

### 3. Base Prompt Generation

Prompt generation for the main agent and helper agents already lives in Rust. The wrappers do not independently construct the base prompt model.

## Present Architectural Tension

The current system is in an in-between state:

- the runtime core is already substantial
- the FFI bindings are already fairly thin
- the high-level wrapper behavior is still duplicated across TypeScript and Python

This hybrid state works, but it creates three long-term problems:

1. Feature evolution requires parallel SDK changes
2. Behavioral parity depends on wrapper discipline rather than runtime guarantees
3. New languages must re-implement wrapper orchestration even if the FFI bridge is simple

## Better Mental Model

The current architecture is best described as:

- **Rust runtime** owns canonical execution
- **Rust FFI bindings** expose runtime capabilities to host languages
- **Host SDK wrappers** currently duplicate orchestration policy

That is more precise than saying the language targets are generally heavy or that the full session system is duplicated.

## Why This Matters For New Languages

If Java, Go, or another language is added today, the cost is not just creating JNI or C-ABI bindings.

The bigger cost is recreating the wrapper layer:

- middleware semantics
- listener lifecycle policy
- prompt interception flow
- session preload/save coordination
- stack injection ergonomics
- callback shaping and typed developer APIs

This is the real multi-language scaling problem.

## Architectural Goal

The goal should be to shrink the amount of behavior that must be re-implemented per language while preserving high-quality language-native typing and ergonomics.

In practice that means:

- keep the FFI bindings thin
- keep language-native typing and generated stubs language-specific
- move more orchestration policy into the shared Rust runtime where parity matters
- leave only true language adaptation in TS/Python/Java/Go wrappers

## Near-Term Direction

The next architectural step is not “move everything into Rust.”

The next step is to decide which parts of the wrapper layer are:

- **runtime policy** and should become shared
- **language ergonomics** and should remain host-specific

That design split should guide the proposed solution.

## Proposed Solution

The right direction is a stricter three-layer architecture:

- **Shared Rust runtime** owns execution policy
- **Thin FFI bindings** expose runtime capabilities to host languages
- **Language SDK wrappers** focus on typing, ergonomics, and native developer experience

The goal is not to remove language-specific SDKs. The goal is to remove duplicated orchestration logic from them.

### 1. Move Wrapper Policy Into Rust

The following behaviors should become first-class runtime concerns inside `ir-runtime`:

- middleware hook ordering and lifecycle semantics
- run lifecycle orchestration
- llm lifecycle orchestration
- helper session preload/save coordination
- stack override and resumption policy
- partial intent post-processing rules when those rules affect parity
- intent interception control flow

In other words, if changing a behavior should affect all languages the same way, that behavior should live in Rust.

### 2. Keep Language Ergonomics Outside Rust

The following should remain language-specific:

- generated type stubs
- host-language naming conventions
- TypeScript conditional and discriminated typing
- Python protocols, TypedDicts, and adapter classes
- convenience overloads and helper APIs
- language-native packaging and import surfaces

These are not duplication problems. They are part of delivering a good SDK in each language.

### 3. Introduce A Shared Runtime Hook Model

Instead of each wrapper implementing middleware policy manually, Rust should expose a single host-hook model that all SDKs bind to.

That model should define:

- hook names
- hook order
- hook inputs
- hook outputs
- control semantics
- error propagation semantics
- whether hooks are observational or mutating

Examples:

- `run_start`
- `intent`
- `partial_intent`
- `llm_start`
- `llm_end`
- `run_complete`
- `error`
- `sub_engine_start`
- `sub_engine_complete`

Once this contract is owned by Rust, TS/Python/Java/Go only need to adapt host callbacks to that contract.

### 4. Make EngineBridge The Canonical Interop Surface

`EngineBridge` should become the single language-agnostic surface for host runtimes.

Its responsibilities should be:

- engine construction
- driver registration
- tool registration
- context updates
- session import/export/clear
- prompt generation
- run/process operations
- registration of host hooks using one stable callback contract

Its responsibilities should not be:

- language-specific middleware abstractions
- host-side stack policy
- host-specific callback orchestration

This keeps the bridge stable even as SDK ergonomics evolve.

### 5. Reduce Wrappers To Adaptation Layers

After the runtime hook model exists, the TS and Python wrappers should mostly do:

- translate user-facing middleware/handlers into the shared hook contract
- provide generated typing around those hooks
- expose convenient factory functions
- perform host-native data conversion
- expose raw/native access for advanced use cases

This makes wrappers smaller without making them useless.

### 6. Treat Session Persistence As A Runtime Protocol

Today Rust owns the canonical session, but wrappers still coordinate parts of session preload/save behavior.

The cleaner design is:

- Rust defines when preload/save hooks fire
- Rust defines what session payload is passed
- Rust defines how stack changes are interpreted
- Hosts only provide persistence callbacks

That preserves host control over storage while removing duplicated orchestration policy.

### 7. Treat Stack Control As Shared Execution Policy

Stack-aware resumption already mostly lives in Rust. That should be completed rather than re-expanded into wrappers.

The target state is:

- Rust owns stack interpretation and teleportation semantics
- hosts may supply an initial stack or mutate stack through defined hooks
- hosts do not re-implement the meaning of stack changes

This keeps execution behavior consistent across all languages.

### 8. Clarify Partial Intent Responsibilities

Some partial intent behavior is pure transport, while some is policy.

The split should be:

- transport concerns stay in FFI
- language display helpers stay in SDKs
- parity-sensitive intent semantics move into Rust

For example, if a partial response delta is part of the official runtime contract, Rust should define it. If it is only a UI convenience helper, the SDK may derive it.

### 9. Design Principle For Future Features

For every new capability, ask:

**If this behavior changes, should all languages behave the same way?**

If yes, it belongs in Rust.

If no, it belongs in the SDK layer.

This rule is simple, but it prevents the architecture from drifting back into wrapper duplication.

## Proposed End State

The intended long-term shape is:

- **Rust runtime**
  - canonical execution semantics
  - canonical lifecycle semantics
  - canonical session and stack semantics
  - canonical hook contract

- **FFI layer**
  - minimal host interop glue
  - async bridging
  - callback bridging
  - value conversion

- **SDK layer**
  - typed facades
  - generated language-native APIs
  - convenience abstractions
  - ergonomic helpers

In this model, new languages are cheaper because they only need:

- an FFI binding
- a thin typed SDK surface

They do not need to recreate the runtime policy.

## Migration Strategy

The migration should be incremental, not a rewrite.

### Phase 1: Formalize The Boundary

- document the shared hook contract
- document which wrapper behaviors are policy vs ergonomics
- stop adding new parity-sensitive behavior directly in wrappers

### Phase 2: Move Shared Policies Into Rust

- centralize middleware ordering and lifecycle semantics
- centralize run and llm hook orchestration
- centralize helper preload/save flow semantics
- centralize stack override interpretation

### Phase 3: Simplify Existing SDKs

- reduce TS wrapper orchestration
- reduce Python wrapper orchestration
- preserve typing quality and generated API quality
- keep escape hatches for raw/native use

### Phase 4: Validate With A New Language

- implement one new host target such as Java or Go
- verify that only FFI and SDK ergonomics are needed
- confirm that no execution policy had to be re-implemented outside Rust

## Expected Benefits

- stronger behavioral parity across languages
- lower maintenance cost for new lifecycle features
- less wrapper drift over time
- easier addition of Java, Go, and native Rust usage
- clearer ownership boundaries inside the codebase

## Constraint To Preserve

This refactor should not weaken the strengths of the current SDKs.

Specifically, Auwgent should preserve:

- rich generated TypeScript typing
- strong Python typing and editor hints
- ergonomic object-style handlers
- direct raw/native access for advanced users

The target is not less expressive SDKs. The target is thinner orchestration with better shared correctness.
