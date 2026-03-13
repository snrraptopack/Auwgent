# Auwgent VS Code Extension

This extension is a thin VS Code client for the Rust-based Auwgent language server.

## Features

- Registers the `auwgent` language for `.agent` files.
- Bundles TextMate syntax highlighting for immediate editor coloring.
- Launches the Rust `auwgent-lsp` binary over stdio.
- Surfaces diagnostics, completion, hover, definition, references, and rename from the Rust server.

## Development

Build the Rust language server first:

```powershell
cargo build -p auwgent-lsp
```

from [auwgent-compiler/Cargo.toml](https://github.com/snrraptopack/Auwgent/blob/main/auwgent-compiler/Cargo.toml).

Then build the extension:

```powershell
npm run compile
```

For a live edit loop, start the `watch` task once and then use the `Run Extension` launch profile. That avoids rebuilding on every `F5` while still picking up TypeScript and esbuild changes.

Use `Run Extension (Start Watchers)` when you want VS Code to start the watch tasks for you.

If the Rust binary is not present, the extension now offers to build it and retries startup automatically after a successful build.

Current Rust LSP limitation: parse and import-resolution diagnostics are published per file, but successful multi-file checker diagnostics still come from a merged model without per-file provenance. Root-file diagnostics are solid; imported-file checker diagnostics need compiler-side source mapping to be exact.

## Configuration

The extension reads `auwgent.serverPath` when set. If omitted, it looks for the binary in:

- `../auwgent-compiler/target/debug/auwgent-lsp(.exe)`
- `../auwgent-compiler/target/release/auwgent-lsp(.exe)`
