//! # auwgent-codegen
//!
//! Generates target-language type stubs from the lowered IR.

mod common;
mod python;
mod typescript;

use serde_json::Value;

pub fn generate_typescript(ir: &Value, base_name: &str) -> String {
    typescript::generate(ir, base_name)
}

pub fn generate_python(ir: &Value, base_name: &str) -> String {
    python::generate(ir, base_name)
}
