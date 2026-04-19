# Auwgent Testing

This directory is the centralized testing area for Auwgent.

Its purpose is to validate the core runtime behavior in a deterministic, richer, and more maintainable way **without bloating the SDK target layer** and without relying first on TypeScript, Python, Dart, or other FFI-facing surfaces.

## Goals

This testing area exists to give you a better testing portfolio for Auwgent by separating concerns clearly:

- **Core/runtime confidence first**
- **Target and FFI verification second**
- **Manual exploration last**

That means when a target-specific integration fails later, you can reason much more clearly about where the problem lives:

- if centralized runtime tests pass, the core behavior is likely correct
- if a target test fails after that, the issue is likely in codegen, SDK wiring, or FFI behavior

## Testing Philosophy

The source of truth for each scenario is the `.agent` DSL source.

Each scenario/case contains:

- the `.agent` source files used to define the test scenario
- a committed `generated/` directory produced by the CLI
- deterministic Rust-side tests that use the generated Rust factory API

We **do not regenerate fixtures on every test run**.

Instead, the workflow is:

1. write or update `.agent` source for a scenario
2. run the CLI generate command for that scenario
3. commit the updated `generated/` files
4. run tests against those committed generated artifacts

This keeps tests:

- deterministic
- fast
- reviewable
- easy to diff when compiler or codegen behavior changes

## Directory Structure

A scenario should be self-contained.

Example layout:

```text
testing/
  README.md
  Cargo.toml
  src/
  tests/
  cases/
    simple-tool/
      main.agent
      generated/
        main.agent.json
        main.agent.rs
```

Over time, more cases can be added in a similar structure:

```text
cases/
  tools/
  workflows/
  helpers/
  transfers/
  context/
  components/
  streaming/
  session/
```

You can choose either:

- one case per leaf directory, or
- grouped folders with multiple named cases underneath

The important part is that every case owns both its source and its generated artifacts.

## Why Keep `generated/` Committed

The generated files are committed on purpose.

This gives you:

- a stable fixture model
- faster tests
- no regeneration cost during normal test execution
- explicit review of compiler/codegen changes
- easier debugging when behavior changes

If a compiler change affects the generated output, that should show up as a normal diff in version control.

That is a feature, not a problem.

## Why the JSON Exists but Is Not the Main Entry Point

The CLI generate step will typically produce generated files such as:

- `main.agent.json`
- `main.agent.rs`

For the centralized runtime tests, the main entry point should generally be the **generated Rust file**, because it provides:

- the Rust factory function
- typed config and helper types
- a higher-level developer experience
- a more realistic consumer path

The JSON is still important because the generated Rust factory internally depends on it, but the tests should not usually talk to raw IR directly unless a specific lower-level test needs that.

In other words:

- the JSON is part of the fixture
- the Rust-generated API is the preferred test surface

## What This Testing Layer Covers

This directory is intended to cover core behavior through the Rust path in two styles:

### 1. Static / Fixture Validation

These tests validate the structure of the scenario and its generated surface.

Examples:

- generated files exist
- generated Rust module compiles
- expected factory function is available
- expected tools/workflows/helpers appear in the generated surface
- expected fixture structure is present

These tests are fast and structural.

### 2. Deterministic Runtime Validation

These tests execute behavior through the generated Rust API and the Rust SDK/runtime path.

Examples:

- tool calls behave correctly
- workflow calls and results behave correctly
- helper transfer and handoff behavior works correctly
- middleware fires in the expected order
- partial intents stream correctly
- sessions export and import correctly
- runtime behavior is stable for known scenarios

These tests should avoid live model variability whenever possible.

## What This Testing Layer Does Not Replace

This directory does **not** replace:

- `manual-testing/`
- target-specific verification in `targets/`
- future live-provider scenario runs

Instead, it complements them.

### `manual-testing/`
Use this for exploratory checks and hands-on validation.

### `targets/`
Use those for SDK/FFI surface verification and target-specific behavior.

### centralized `testing/`
Use this for core confidence through generated Rust fixtures in a deterministic way.

## Recommended Workflow for Adding a New Scenario

When adding a new testing scenario:

1. create a new case directory under `cases/`
2. add the `.agent` source files for the scenario
3. generate the Rust fixture files into that case's `generated/` folder
4. add a Rust test that uses the generated Rust API
5. commit both the source and generated artifacts

### Example

1. Create a case:

```text
cases/my-new-case/
  main.agent
  generated/
```

2. Generate artifacts for the case:

```text
cargo run --manifest-path auwgent-compiler/Cargo.toml -p auwgent-cli -- generate testing/cases/my-new-case/main.agent --target rust --output testing/cases/my-new-case/generated
```

3. Add or update tests in `testing/tests/`

4. Run the centralized tests

## Regeneration Policy

You should regenerate a case when:

- the `.agent` source changes
- imported `.agent` files for that case change
- compiler/lowering behavior changes intentionally
- Rust codegen behavior changes intentionally

You should **not** regenerate just because you are running tests.

The rule is:

- **generate on change**
- **test from committed fixtures**

## Future Extensions

This structure is intentionally compatible with future growth.

Possible additions later:

- a fixture refresh script
- a fixture consistency checker
- grouped scenario suites
- richer reporting
- optional live-provider verification that still uses scenario fixtures
- cross-target comparisons using the same scenario source

## Summary

This centralized testing area is designed to give Auwgent:

- better runtime confidence
- deterministic and richer test cases
- less dependency on manual target testing for core validation
- a cleaner boundary between runtime behavior and SDK/FFI behavior

The key principle is simple:

**write `.agent` source once, generate fixtures intentionally, and test against the committed generated Rust surface.**