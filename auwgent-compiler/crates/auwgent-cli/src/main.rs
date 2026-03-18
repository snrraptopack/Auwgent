//! # auwgent-cli
//!
//! CLI binary for the Auwgent compiler.
mod config;
mod watch;
mod commands;
mod scaffold;
mod resolution;
mod errors;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "auwgent",
    about = "Auwgent DSL Compiler",
    version,
    after_help = "EXAMPLES:\n  auwgent generate\n  auwgent compile main.agent"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile .agent file(s) to IR JSON
    Compile {
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// Generate type stubs from .agent file(s)
    Generate {
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
        #[arg(short, long, value_name = "LANG")]
        target: Option<String>,
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,
        #[arg(short, long)]
        watch: bool,
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { path, output, config } => {
            commands::run_compile(path, output, config);
        }
        Commands::Generate { path, target, output, watch, config } => {
            commands::run_generate(path, target, output, watch, config);
        }
    }
}
