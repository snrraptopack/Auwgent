//! # auwgent-cli
//!
//! CLI binary for the Auwgent compiler.
//! Commands: compile, generate.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

fn compile(file: &PathBuf, output: Option<&std::path::Path>) {
    let filename = file.display().to_string();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: could not read '{}': {}", filename, e);
            std::process::exit(1);
        }
    };

    // 1. Lex
    let (tokens, lex_errors) = auwgent_lexer::tokenize(&source);
    if !lex_errors.is_empty() {
        auwgent_errors::render_lex_errors(&lex_errors, &filename, &source);
        std::process::exit(1);
    }

    // 2. Parse
    let (model, parse_errors) = auwgent_parser::parse(&tokens);
    if !parse_errors.is_empty() {
        auwgent_errors::render_diagnostics(&parse_errors, &filename, &source);
        std::process::exit(1);
    }

    // 3. Check
    let diagnostics = auwgent_checker::check(&model);
    if auwgent_errors::render_diagnostics(&diagnostics, &filename, &source) {
        std::process::exit(1);
    }

    // 4. Lower to IR
    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for e in &errs {
                eprintln!("Error: {}", e);
            }
            std::process::exit(1);
        }
    };

    // 5. Write JSON
    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let stem = file.file_stem().unwrap().to_string_lossy();
    let out_path = out_dir.join(format!("{}.agent.json", stem));

    let json = serde_json::to_string_pretty(&ir).unwrap();
    std::fs::write(&out_path, json).unwrap();

    eprintln!("\x1b[Compiled → {}\x1b[0m", out_path.display());
}

fn generate(file: &PathBuf, target: &str, output: Option<&std::path::Path>) {
    let filename = file.display().to_string();

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: could not read '{}': {}", filename, e);
            std::process::exit(1);
        }
    };

    let (tokens, _) = auwgent_lexer::tokenize(&source);
    let (model, _) = auwgent_parser::parse(&tokens);
    let ir = auwgent_ir::lower(&model).unwrap();

    let code = match target {
        "ts" | "typescript" => auwgent_codegen::generate_typescript(&ir),
        "py" | "python" => auwgent_codegen::generate_python(&ir),
        _ => {
            eprintln!("Unknown target '{}'. Use 'ts' or 'python'.", target);
            std::process::exit(1);
        }
    };

    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let stem = file.file_stem().unwrap().to_string_lossy();
    let ext = if target.starts_with("ts") {
        "types.ts"
    } else {
        "_types.py"
    };
    let out_path = out_dir.join(format!("{}.agent.{}", stem, ext));

    std::fs::write(&out_path, code).unwrap();
    eprintln!("\x1b[Generated → {}\x1b[0m", out_path.display());
}
