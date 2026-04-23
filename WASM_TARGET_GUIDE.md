# Auwgent WebAssembly (WASM) Target Guide

This document outlines the architectural requirements and steps for adding a WebAssembly (`wasm32-unknown-unknown`) target to the Auwgent engine (`ir-runtime`). 

Compiling to WASM serves two primary use cases:
1. **Edge Computing:** Running the engine on V8 isolates like Cloudflare Workers or Vercel Edge.
2. **Client-Side Execution:** Running the engine entirely in the user's web browser (React, Vue, Vanilla JS) with zero server backend.

## Why WASM over NAPI-rs/PyO3?

Currently, the `targets/typescript` (via NAPI-rs) and `targets/python` (via PyO3) rely on native operating system binaries (`.node` or `.pyd` / `.so`). Edge environments and browsers cannot run OS executables. They require a sandboxed WebAssembly binary.

## Implementation Steps

To implement a WASM version, you should create a new dedicated package (e.g., `targets/wasm`) rather than modifying the existing NAPI/PyO3 packages.

### 1. Project Setup
Create the new target inside the `targets` directory:
- Add a new `Cargo.toml`.
- Add `wasm-bindgen` and `js-sys` as dependencies instead of `napi` or `pyo3`.
- Depend directly on your local `ir-runtime` crate.

### 2. Rust to WASM Bridging
Write the bridging logic using `wasm-bindgen`. This will look very similar to your `_native.rs` bridging, but using WebAssembly annotations:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct AuwgentWasm {
    // Wrap the core ir-runtime engine here
}

#[wasm_bindgen]
impl AuwgentWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(ir_json: &str) -> Result<AuwgentWasm, JsValue> {
        // Initialization logic
    }
    
    // ... expose run(), on_intent(), etc.
}
```

### 3. Critical Constraints & Refactors

Because WebAssembly runs in a restricted JS environment, your core `ir-runtime` must adhere to the following rules:

#### A. Single-Threaded Async Execution
WebAssembly in the browser and Cloudflare Workers does **not** support traditional OS threads.
- You **cannot** use `std::thread::spawn` or multi-threaded `tokio::spawn`.
- All asynchronous tasks must be spawned using `wasm_bindgen_futures::spawn_local` onto the single JavaScript event loop.
- If `ir-runtime` strictly requires multi-threading, you will need to gate those features using `#[cfg(not(target_arch = "wasm32"))]` or refactor to use single-threaded concurrency locally.

#### B. Network/HTTP Requests (LLM API Calls)
Your engine makes HTTP requests (e.g., calling APIs like OpenAI, Gemini, Groq).
- Blocking HTTP libraries will panic in WASM.
- If you are using `reqwest`, you must ensure it can compile to WASM (the async version of `reqwest` automatically converts its calls to the native JavaScript `fetch()` API when compiled to WASM).

### 4. Compilation & Usage

You can compile the target using tools like `wasm-pack`:
- **For the Browser:** `wasm-pack build --target web`
- **For Bundlers (Webpack/Vite/Cloudflare):** `wasm-pack build --target bundler`

This will generate a `.wasm` file and a strongly-typed `.js` / `.d.ts` wrapper. You can then import the WASM module directly into your Cloudflare Worker `index.ts` or your frontend browser app.
