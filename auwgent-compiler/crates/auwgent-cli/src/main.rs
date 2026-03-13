//! # auwgent-cli
//!
//! CLI binary for the Auwgent compiler.
//! Supports single files, directories, glob patterns, `auwgent.yml` config,
//! and `--watch` mode for automatic regeneration on save.

mod config;
mod watch;

use auwgent_analysis::AnalysisError;
use clap::{Parser, Subcommand};
use config::Config;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "auwgent",
    about = "Auwgent DSL Compiler",
    version,
    after_help = "\
EXAMPLES:
  auwgent generate                      # reads auwgent.yml, or scans current directory
  auwgent generate ./agents             # scan a folder
  auwgent generate main.agent           # single file
  auwgent generate ./agents --watch     # watch mode (regenerates on save)
  auwgent generate ./agents -t both     # output TypeScript + Python
  auwgent compile main.agent            # compile to IR JSON"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile .agent file(s) to IR JSON
    Compile {
        /// .agent file, directory, or glob pattern (default: reads auwgent.yml or scans .)
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,

        /// Output directory (default: same directory as each source file)
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,

        /// Config file path (default: auwgent.yml in current directory)
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },

    /// Generate type stubs from .agent file(s)
    Generate {
        /// .agent file, directory, or glob pattern (default: reads auwgent.yml or scans .)
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,

        /// Target language: ts, python, or both (default: ts, or from auwgent.yml)
        #[arg(short, long, value_name = "LANG")]
        target: Option<String>,

        /// Output directory (default: same directory as each source file)
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,

        /// Watch for .agent changes and regenerate automatically
        #[arg(short, long)]
        watch: bool,

        /// Config file path (default: auwgent.yml in current directory)
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            path,
            output,
            config,
        } => {
            let cfg = Config::load(config.as_deref());
            let out = output.or_else(|| cfg.as_ref().and_then(|c| c.output.as_deref().map(PathBuf::from)));
            let files = resolve_sources(path.as_deref(), cfg.as_ref().and_then(|c| c.source.as_deref()));
            if files.is_empty() {
                eprintln!("No .agent files found.");
                std::process::exit(1);
            }
            let mut had_error = false;
            for file in &files {
                if !compile_file(file, out.as_deref()) {
                    had_error = true;
                }
            }
            if had_error {
                std::process::exit(1);
            }
        }

        Commands::Generate {
            path,
            target,
            output,
            watch,
            config,
        } => {
            let cfg = Config::load(config.as_deref());
            let out = output.or_else(|| cfg.as_ref().and_then(|c| c.output.as_deref().map(PathBuf::from)));
            let targets = resolve_targets(target.as_deref(), cfg.as_ref().map(|c| c.targets.as_slice()));
            let config_source = cfg.as_ref().and_then(|c| c.source.as_deref());
            let files = resolve_sources(path.as_deref(), config_source);

            if files.is_empty() && !watch {
                eprintln!("No .agent files found.");
                std::process::exit(1);
            }

            if watch {
                let watch_roots = resolve_watch_roots(path.as_deref(), config_source);
                watch::watch_and_generate(&files, &watch_roots, &targets, out.as_deref());
            } else {
                let mut had_error = false;
                for file in &files {
                    for t in &targets {
                        if !generate_file(file, t, out.as_deref()) {
                            had_error = true;
                        }
                    }
                }
                if had_error {
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Resolve which targets to generate for.
/// Priority: CLI arg > auwgent.yml > default "ts"
fn resolve_targets(target_arg: Option<&str>, config_targets: Option<&[String]>) -> Vec<String> {
    match target_arg {
        Some("both") => vec!["ts".to_string(), "python".to_string()],
        Some(t) => vec![t.to_string()],
        None => {
            if let Some(targets) = config_targets {
                if !targets.is_empty() {
                    return targets.to_vec();
                }
            }
            vec!["ts".to_string()]
        }
    }
}

/// Resolve source files from: explicit PATH > auwgent.yml sources > scan current dir.
fn resolve_sources(path: Option<&Path>, config_source: Option<&str>) -> Vec<PathBuf> {
    if let Some(p) = path {
        return resolve_input(&p.to_string_lossy());
    }

    if let Some(source) = config_source {
        return resolve_input(source);
    }

    collect_agent_files(Path::new("."))
}

fn resolve_watch_roots(path: Option<&Path>, config_source: Option<&str>) -> Vec<PathBuf> {
    if let Some(p) = path {
        return input_watch_roots(&p.to_string_lossy());
    }
    if let Some(source) = config_source {
        return input_watch_roots(source);
    }
    vec![PathBuf::from(".")]
}

fn resolve_input(input: &str) -> Vec<PathBuf> {
    let path = Path::new(input);
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    if path.is_dir() {
        return collect_agent_files(path);
    }
    expand_glob(input)
}

fn input_watch_roots(input: &str) -> Vec<PathBuf> {
    let path = Path::new(input);
    if path.is_file() {
        return vec![path.parent().unwrap_or(Path::new(".")).to_path_buf()];
    }
    if path.is_dir() {
        return vec![path.to_path_buf()];
    }

    // For glob inputs like ./manual-testing/**/*.agent, watch from the prefix before wildcard.
    let wildcard_index = input.find(['*', '?', '[']).unwrap_or(input.len());
        let prefix = input[..wildcard_index].trim_end_matches(['/', '\\']);
    if prefix.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        let p = PathBuf::from(prefix);
        if p.is_dir() {
            vec![p]
        } else {
            vec![p.parent().unwrap_or(Path::new(".")).to_path_buf()]
        }
    }
}

/// Expand a glob pattern into matching `.agent` paths.
fn expand_glob(pattern: &str) -> Vec<PathBuf> {
    match glob::glob(pattern) {
        Ok(paths) => paths
            .filter_map(|e| e.ok())
            .filter(|p| p.extension().map_or(false, |e| e == "agent"))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Recursively collect all `.agent` files under `dir`.
fn collect_agent_files(dir: &Path) -> Vec<PathBuf> {
    // Use forward slashes so the glob crate handles them uniformly on all platforms
    let dir_str = dir.to_string_lossy().replace('\\', "/");
    expand_glob(&format!("{}/**/*.agent", dir_str))
}

pub fn compile_file(file: &Path, output: Option<&Path>) -> bool {
    let model = match auwgent_analysis::load_model_with_imports(file) {
        Ok(m) => m,
        Err(e) => {
            report_analysis_error(&e);
            return false;
        }
    };

    let filename = file.display().to_string();
    let source = std::fs::read_to_string(file).unwrap_or_default();
    let diagnostics = auwgent_checker::check(&model);
    if auwgent_errors::render_diagnostics(&diagnostics, &filename, &source) {
        return false;
    }

    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for e in &errs {
                eprintln!("Error: {}", e);
            }
            return false;
        }
    };

    let stem = file.file_stem().unwrap().to_string_lossy();
    let out_dir = resolve_out_dir(file, output);
    let out_path = out_dir.join(format!("{}.agent.json", stem));

    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&out_path, serde_json::to_string_pretty(&ir).unwrap()).unwrap();
    eprintln!("\x1b[32m✓\x1b[0m {} → {}", file.display(), out_path.display());
    true
}

pub fn generate_file(file: &Path, target: &str, output: Option<&Path>) -> bool {
    let model = match auwgent_analysis::load_model_with_imports(file) {
        Ok(m) => m,
        Err(e) => {
            report_analysis_error(&e);
            return false;
        }
    };

    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for e in &errs {
                eprintln!("Error: {}", e);
            }
            return false;
        }
    };

    let stem = file.file_stem().unwrap().to_string_lossy();
    let code = match target {
        "ts" | "typescript" => auwgent_codegen::generate_typescript(&ir, &stem),
        "py" | "python" => auwgent_codegen::generate_python(&ir, &stem),
        _ => {
            eprintln!("Unknown target '{}'. Use 'ts', 'python', or 'both'.", target);
            return false;
        }
    };

    let file_name = if target.starts_with("ts") {
        format!("{}.agent.types.ts", stem)
    } else {
        format!("{}_types.py", stem)
    };

    let out_dir = resolve_out_dir(file, output);
    let out_path = out_dir.join(file_name);

    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&out_path, code).unwrap();
    eprintln!("\x1b[32m✓\x1b[0m {} → {}", file.display(), out_path.display());
    true
}

/// Return the effective output directory: explicit override, or the file's own parent.
fn resolve_out_dir<'a>(file: &'a Path, output: Option<&'a Path>) -> &'a Path {
    if let Some(o) = output {
        return o;
    }
    let parent = file.parent().unwrap_or(Path::new("."));
    if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    }
}

fn report_analysis_error(error: &AnalysisError) {
    match error {
        AnalysisError::Lex {
            path,
            source,
            diagnostics,
        }
        | AnalysisError::Parse {
            path,
            source,
            diagnostics,
        } => {
            auwgent_errors::render_diagnostics(diagnostics, &path.display().to_string(), source);
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
