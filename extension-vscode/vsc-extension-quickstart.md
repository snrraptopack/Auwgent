# Auwgent VS Code Extension Quickstart

## What this extension does

- Registers the `auwgent` language for `.agent` files.
- Bundles TextMate syntax highlighting for `.agent` files.
- Starts the Rust `auwgent-lsp` server over stdio.
- Uses the Rust server for diagnostics, completion, hover, definition, references, and rename.

## Run it locally

1. Build the Rust language server from the workspace root:

	```powershell
	cargo build -p auwgent-lsp --manifest-path .\auwgent-compiler\Cargo.toml
	```

2. Build the extension from `extension-vscode`:

	```powershell
	npm run compile
	```

3. Press `F5` in `extension-vscode` to launch an Extension Development Host.

	Use `Run Extension` for the fast path after you have already built once or started the watch task.

	Use `Run Extension (Start Watchers)` when you want VS Code to start the watch tasks for you.

## Configuration

- Set `auwgent.serverPath` to an absolute `auwgent-lsp` binary path if you do not want the extension to use the default compiler target directories.
- If the Rust binary is missing, the extension offers to build it and restarts the client automatically after the build succeeds.
