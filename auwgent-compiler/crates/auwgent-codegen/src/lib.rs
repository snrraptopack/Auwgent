//! # auwgent-codegen
//!
//! Generates TS (`.types.ts`) and Python (`_types.py`) type stubs from the IR.
//! Replaces `typescriptGenerator.ts` and `pythonGenerator.ts`.

use serde_json::Value;

/// Generate TypeScript type definitions from an IR JSON value.
pub fn generate_typescript(ir: &Value) -> String {
    let _ = ir;
    // TODO: Port typescriptGenerator.ts
    String::new()
}

/// Generate Python type definitions from an IR JSON value.
pub fn generate_python(ir: &Value) -> String {
    let _ = ir;
    // TODO: Port pythonGenerator.ts
    String::new()
}
