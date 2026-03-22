use std::path::PathBuf;

mod python;
mod typescript;

fn main() {
    let workspace_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let out_dir = workspace_dir.join("bindings");
    std::fs::create_dir_all(&out_dir).expect("Failed to create bindings/ directory");

    let ts_dir = out_dir.join("typescript");
    std::fs::create_dir_all(&ts_dir).expect("Failed to create bindings/typescript/ directory");
    println!("Generating TypeScript bindings...");
    typescript::generate(&ts_dir);

    let py_dir = out_dir.join("python");
    std::fs::create_dir_all(&py_dir).expect("Failed to create bindings/python/ directory");
    println!("Generating Python bindings...");
    python::generate(&py_dir);

    println!("Done! Generated SDK schema bindings to {}", out_dir.display());
}
