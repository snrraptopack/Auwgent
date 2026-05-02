# Auwgent WASM Runtime

This package is the browser/edge runtime for the TypeScript SDK.

It does not use the C ABI. It binds `ir-runtime` directly through
`wasm-bindgen`, which is the correct path for environments that cannot load
native `.node`, `.dll`, `.so`, or `.dylib` artifacts.

Build output is expected to be copied into:

```text
targets/typescript/wasm-runtime/
```

Recommended build command:

```sh
wasm-pack build targets/wasm-runtime --target bundler --out-dir ../typescript/wasm-runtime
```

