use std::path::{Path, PathBuf};
use std::collections::HashSet;
use crate::config::Config;
use crate::resolution;
use crate::scaffold;
use crate::errors;

pub fn run_compile(path: Option<PathBuf>, output: Option<PathBuf>, config: Option<PathBuf>) {
    let cfg = Config::load(config.as_deref());
    let out = output.or_else(|| cfg.as_ref().and_then(|c| c.output.as_deref().map(PathBuf::from)));
    let files = resolution::resolve_sources(path.as_deref(), cfg.as_ref().and_then(|c| c.source.as_deref()));

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
    if had_error { std::process::exit(1); }
}

pub fn run_generate(
    path: Option<PathBuf>,
    target: Option<String>,
    output: Option<PathBuf>,
    watch: bool,
    config: Option<PathBuf>,
) {
    let cfg = Config::load(config.as_deref());
    let out = output.clone().or_else(|| cfg.as_ref().and_then(|c| c.output.as_deref().map(PathBuf::from)));
    let config_source = cfg.as_ref().and_then(|c| c.source.as_deref());
    let files = resolution::resolve_sources(path.as_deref(), config_source);

    if files.is_empty() && !watch {
        eprintln!("No .agent files found.");
        std::process::exit(1);
    }

    if watch {
        let watch_roots = resolution::resolve_watch_roots(path.as_deref(), config_source);
        crate::watch::watch_and_generate(&files, &watch_roots, &resolution::resolve_targets(target.as_deref(), cfg.as_ref().map(|c| c.targets.as_slice())), out.as_deref());
    } else {
        // ── Intelligent Batch Discovery ──
        let root_dir: &Path = path.as_deref().or_else(|| config_source.map(Path::new)).unwrap_or(Path::new("."));
        let mut projects: Vec<(PathBuf, Option<Config>)> = vec![(root_dir.to_path_buf(), cfg.clone())];

        if root_dir.is_dir() {
            for entry in walkdir::WalkDir::new(root_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy();
                    name == "auwgent.yml" || name == "auwgent.yaml"
                })
            {
                let project_dir = entry.path().parent().unwrap().to_path_buf();
                if let Some(project_cfg) = Config::load(Some(entry.path())) {
                    projects.push((project_dir, Some(project_cfg)));
                }
            }
        }

        let mut had_error = false;
        let mut processed_files = HashSet::new();
        projects.sort_by(|a, b| b.0.components().count().cmp(&a.0.components().count()));

        for (p_dir, p_cfg) in projects {
            let p_out = p_cfg.as_ref().and_then(|c| c.output.as_deref().map(PathBuf::from)).or_else(|| out.clone());
            let p_targets = resolution::resolve_targets(target.as_deref(), p_cfg.as_ref().map(|c| c.targets.as_slice()));
            
            let p_all_files = resolution::collect_agent_files(&p_dir);
            let p_files: Vec<_> = p_all_files.into_iter()
                .filter(|f| {
                    let canon = std::fs::canonicalize(f).unwrap_or(f.clone());
                    processed_files.insert(canon)
                })
                .collect();

            if p_files.is_empty() { continue; }

            // Production Path: Generation
            for file in &p_files {
                for t in &p_targets {
                    if !generate_file(file, t, p_out.as_deref()) {
                        had_error = true;
                    }
                }
            }

            // Development Path: Scaffolding
            if p_cfg.as_ref().map_or(false, |c| c.development) {
                let scaffold_out = p_out.clone().unwrap_or_else(|| p_dir.clone());
                scaffold::run_scaffolding(&scaffold_out, &p_dir, &p_targets);
            }
        }

        if had_error { std::process::exit(1); }
    }
}

pub fn compile_file(file: &Path, output: Option<&Path>) -> bool {
    let validation = match auwgent_compile::validate_file_for_compile(file) {
        Ok(v) => v,
        Err(e) => { errors::report_analysis_error(&e); return false; }
    };
    let filename = file.display().to_string();
    let source = std::fs::read_to_string(file).unwrap_or_default();
    if auwgent_errors::render_diagnostics(&validation.diagnostics, &filename, &source) { return false; }

    let Some(ir) = validation.ir else { return true; };
    let stem = file.file_stem().unwrap().to_string_lossy();
    let out_dir = resolve_out_dir(file, output);
    let out_path = out_dir.join(format!("{}.agent.json", stem));

    std::fs::create_dir_all(out_dir).unwrap();
    std::fs::write(&out_path, serde_json::to_string_pretty(&ir).unwrap()).unwrap();
    eprintln!("\x1b[32m✓\x1b[0m {} → {}", file.display(), out_path.display());
    true
}

pub fn generate_file(file: &Path, target: &str, output: Option<&Path>) -> bool {
    let validation = match auwgent_compile::validate_file_for_compile(file) {
        Ok(v) => v,
        Err(e) => { errors::report_analysis_error(&e); return false; }
    };
    let filename = file.display().to_string();
    let source = std::fs::read_to_string(file).unwrap_or_default();
    if auwgent_errors::render_diagnostics(&validation.diagnostics, &filename, &source) { return false; }

    let Some(ir) = validation.ir else { return true; };
    let stem = file.file_stem().unwrap().to_string_lossy();
    let code = match target {
        "ts" | "typescript" => auwgent_codegen::generate_typescript(&ir, &stem),
        "py" | "python" => auwgent_codegen::generate_python(&ir, &stem),
        _ => { eprintln!("Unknown target '{}'. Use 'ts', 'python', or 'both'.", target); return false; }
    };

    let file_name = if target.starts_with("ts") { format!("{}.agent.types.ts", stem) } else { format!("{}_types.py", stem) };
    let out_dir = resolve_out_dir(file, output);
    let out_path = out_dir.join(file_name);

    std::fs::create_dir_all(out_dir).unwrap();
    std::fs::write(&out_path, code).unwrap();
    std::fs::write(out_dir.join(format!("{}.agent.json", stem)), serde_json::to_string_pretty(&ir).unwrap()).unwrap();

    eprintln!("\x1b[32m✓\x1b[0m {} → {} (+ .agent.json)", file.display(), out_path.display());
    true
}

fn resolve_out_dir<'a>(file: &'a Path, output: Option<&'a Path>) -> &'a Path {
    output.unwrap_or_else(|| file.parent().unwrap_or(Path::new(".")))
}
