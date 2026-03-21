# Auwgent Zed Extension

Support for the Auwgent DSL in the Zed editor, featuring syntax highlighting, LSP diagnostics, and editor ergonomics.

## Local Development & Installation

Due to potential environment conflicts with Zed's internal Rust compiler (WASI-SDK) on some Windows machines, this extension is designed to be installed as a **pre-compiled binary**.

### 1. Manual Build (One-time or when code changes)

To rebuild the extension from source:

1.  **Add WASI Target**:
    ```powershell
    rustup target add wasm32-wasip1
    ```
2.  **Restore Source Files**:
    If files are "hidden" (prefixed with `_`), rename them back:
    - `_Cargo.toml_` -> `Cargo.toml`
    - `_src_` -> `src`
3.  **Compile**:
    ```powershell
    cargo build --target wasm32-wasip1 --release
    ```
4.  **Copy Binary**:
    Move the generated `.wasm` file to the root as `extension.wasm`:
    ```powershell
    cp target/wasm32-wasip1/release/auwgent_zed.wasm ./extension.wasm
    ```
5.  **Hide Source** (Crucial workaround):
    Rename `Cargo.toml` and `src` back to `_Cargo.toml_` and `_src_`. This prevents Zed from trying (and failing) to rebuild the extension when you install it.

### 2. Installation in Zed

1.  Open Zed.
2.  Open the Extensions View (`ctrl + shift + x` or `cmd + shift + x`).
3.  Ensure any existing "Auwgent" extension is **Removed**.
4.  Click **Install Dev Extension**.
5.  Select this `extension-zed` folder.

## Features
- **LSP Diagnostics**: Error reporting and hover info (via `auwgent-lsp`).
- **Syntax Highlighting**: High-performance Tree-sitter queries.
- **Ergonomics**: Auto-pairing, auto-indentation, and symbol outline navigation.



Step 1: Restore the project files
Rename-Item "_Cargo.toml_" "Cargo.toml" -Force; Rename-Item "_src_" "src" -Force

Step 2: Compile the extension
cargo build --target wasm32-wasip1 --release

Step 3: Copy the built binary
Copy-Item "target\wasm32-wasip1\release\auwgent_zed.wasm" "extension.wasm" -Force

Step 4: Hide the source code again
Rename-Item "Cargo.toml" "_Cargo.toml_" -Force; Rename-Item "src" "_src_" -Force
