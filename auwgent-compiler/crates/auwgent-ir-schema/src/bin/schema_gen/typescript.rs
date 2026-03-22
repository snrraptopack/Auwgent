use auwgent_ir_schema::AgentIR;
use ts_rs::TS;
use std::path::PathBuf;

pub fn generate(out_dir: &PathBuf) {
    // natively-generated TS bindings. We set it here and then invoke `export_all`.
    std::env::set_var("TS_RS_EXPORT_DIR", out_dir.to_str().unwrap());

    // Export all #[ts(export)] derives across the crate.
    // They will automatically land in separate `.ts` files inside out_dir.
    if let Err(e) = auwgent_ir_schema::AgentIR::export_all() {
        eprintln!("Failed to export TypeScript schema: {}", e);
    }
}
