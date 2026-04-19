//! # auwgent-codegen
//!
//! Generates target-language type stubs from the lowered IR.

mod common;
mod dart;
mod generation_plan;
mod python;
mod rust;
mod typescript;

use auwgent_ir_schema::AgentIR;

pub fn generate_typescript(ir: &AgentIR, base_name: &str) -> String {
    let ir_value = serde_json::to_value(ir).expect("Schema serialization should succeed");
    let plan = generation_plan::CodegenPlan::new(ir_value);
    typescript::generate(&plan, base_name)
}

pub fn generate_python(ir: &AgentIR, base_name: &str) -> String {
    let ir_value = serde_json::to_value(ir).expect("Schema serialization should succeed");
    let plan = generation_plan::CodegenPlan::new(ir_value);
    python::generate(&plan, base_name)
}

pub fn generate_dart(ir: &AgentIR, base_name: &str) -> String {
    let ir_value = serde_json::to_value(ir).expect("Schema serialization should succeed");
    let plan = generation_plan::CodegenPlan::new(ir_value);
    dart::generate(&plan, base_name)
}

pub fn generate_dart_ir_module(ir: &AgentIR) -> String {
    let ir_value = serde_json::to_value(ir).expect("Schema serialization should succeed");
    dart::generate_ir_module(&ir_value)
}

pub fn generate_rust(ir: &AgentIR, base_name: &str) -> String {
    let ir_value = serde_json::to_value(ir).expect("Schema serialization should succeed");
    let plan = generation_plan::CodegenPlan::new(ir_value);
    rust::generate(&plan, base_name)
}
