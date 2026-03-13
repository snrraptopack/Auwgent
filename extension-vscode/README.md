# Auwgent VS Code Extension

This extension is a thin VS Code client for the Rust-based Auwgent language server.

## Features

- Registers the `auwgent` language for `.agent` files.
- Bundles TextMate syntax highlighting for immediate editor coloring.
- Launches the Rust `auwgent-lsp` binary over stdio.
- Surfaces diagnostics, completion, hover, definition, references, and rename from the Rust server.
- Keeps the VS Code layer intentionally thin so compile semantics stay in Rust.

## Development

### Build the bundled Rust LSP binary

Build the Rust language server and copy it into the extension `bin` folder:

```powershell
node scripts/build-lsp.js
```

This workflow is the preferred development path when you want the extension to run a bundled `auwgent-lsp` binary directly.

You can still build the Rust server manually from [auwgent-compiler/Cargo.toml](https://github.com/snrraptopack/Auwgent/blob/main/auwgent-compiler/Cargo.toml):

```powershell
cargo build -p auwgent-lsp
```

### Build the extension

Then build the extension:

```powershell
npm run compile
```

For a live edit loop, start the `watch` task once and then use the `Run Extension` launch profile. That avoids rebuilding on every `F5` while still picking up TypeScript and esbuild changes.

Use `Run Extension (Start Watchers)` when you want VS Code to start the watch tasks for you.

If the Rust binary is not present, the extension now offers to build it and retries startup automatically after a successful build.

## Compile parity

The design goal is that VS Code stays minimal and the Rust compiler/LSP layer remains the source of truth for:

- diagnostics
- hover
- completion
- metadata
- definition, references, and rename

Compile parity means editor diagnostics should reflect the same validation pipeline used by CLI compilation as closely as possible. In practice, that means the Rust language server should validate not only parsing and semantic checks, but also compile-stage validation.

Current limitation: parse and import-resolution diagnostics are published per file, but successful multi-file checker diagnostics still come from a merged model without per-file provenance. Root-file diagnostics are solid; imported-file checker diagnostics need compiler-side source mapping to be exact.

Another current limitation is that lowering-stage failures are less precise than parser/checker diagnostics unless the compiler returns structured spans for them.

## Configuration

The extension reads `auwgent.serverPath` when set. If omitted, it looks for the binary in:

- `../auwgent-compiler/target/debug/auwgent-lsp(.exe)`
- `../auwgent-compiler/target/release/auwgent-lsp(.exe)`

When you are testing the bundled workflow, point `auwgent.serverPath` at the copied binary only if you want to override the default lookup behavior.
