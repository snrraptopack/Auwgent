//! # auwgent-cli
//!
//! CLI binary for the Auwgent compiler.
//! Commands: compile, generate.

use auwgent_analysis::AnalysisError;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "auwgent", about = "Auwgent DSL Compiler", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a .agent file to IR JSON
    Compile {
        /// Path to the .agent file
        file: PathBuf,

        /// Output directory (defaults to same directory as input)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate type stubs from a .agent file
    Generate {
        /// Path to the .agent file
        file: PathBuf,

        /// Target language: ts or python
        #[arg(short, long, default_value = "ts")]
        target: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { file, output } => compile(&file, output.as_deref()),
        Commands::Generate {
            file,
            target,
            output,
        } => generate(&file, &target, output.as_deref()),
    }
}

fn compile(file: &PathBuf, output: Option<&Path>) {
    let model = match auwgent_analysis::load_model_with_imports(file) {
        Ok(model) => model,
        Err(error) => {
            report_analysis_error(&error);
            std::process::exit(1);
        }
    };

    let filename = file.display().to_string();
    let source = std::fs::read_to_string(file).unwrap_or_default();

    let diagnostics = auwgent_checker::check(&model);
    if auwgent_errors::render_diagnostics(&diagnostics, &filename, &source) {
        std::process::exit(1);
    }

    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for error in &errs {
                eprintln!("Error: {}", error);
            }
            std::process::exit(1);
        }
    };

    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let stem = file.file_stem().unwrap().to_string_lossy();
    let out_path = out_dir.join(format!("{}.agent.json", stem));

    let json = serde_json::to_string_pretty(&ir).unwrap();
    std::fs::write(&out_path, json).unwrap();

    eprintln!("\x1b[Compiled → {}\x1b[0m", out_path.display());
}

fn generate(file: &PathBuf, target: &str, output: Option<&Path>) {
    let model = match auwgent_analysis::load_model_with_imports(file) {
        Ok(model) => model,
        Err(error) => {
            report_analysis_error(&error);
            std::process::exit(1);
        }
    };

    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for error in &errs {
                eprintln!("Error: {}", error);
            }
            std::process::exit(1);
        }
    };

    let stem = file.file_stem().unwrap().to_string_lossy();

    let code = match target {
        "ts" | "typescript" => auwgent_codegen::generate_typescript(&ir, &stem),
        "py" | "python" => auwgent_codegen::generate_python(&ir, &stem),
        _ => {
            eprintln!("Unknown target '{}'. Use 'ts' or 'python'.", target);
            std::process::exit(1);
        }
    };

    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let file_name = if target.starts_with("ts") {
        format!("{}.agent.types.ts", stem)
    } else {
        format!("{}_types.py", stem)
    };
    let out_path = out_dir.join(file_name);

    std::fs::write(&out_path, code).unwrap();
    eprintln!("\x1b[Generated → {}\x1b[0m", out_path.display());
}

fn report_analysis_error(error: &AnalysisError) {
    match error {
        AnalysisError::Lex { path, source, diagnostics }
        | AnalysisError::Parse {
            path,
            source,
            diagnostics,
        } => {
            let filename = path.display().to_string();
            auwgent_errors::render_diagnostics(diagnostics, &filename, source);
        }
        AnalysisError::ResolveImport {
            current_file,
            import_path,
            message,
            ..
        } => {
            eprintln!(
                "Error: could not resolve import '{}' from '{}': {}",
                import_path,
                current_file.display(),
                message
            );
        }
        _ => eprintln!("Error: {}", error),
    }
}
