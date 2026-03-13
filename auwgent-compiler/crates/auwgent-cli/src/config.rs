use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration loaded from `auwgent.yml` in the project root.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    /// Source selector for .agent files. Can be a file, directory, or glob.
    pub source: Option<String>,
    /// Shared output directory for generated code and compiled IR.
    pub output: Option<String>,
    /// Target languages: ts, python, or both (e.g. ["ts", "python"])
    #[serde(default)]
    pub targets: Vec<String>,
}

impl Config {
    /// Load `auwgent.yml` from the given path, or from the current directory if `None`.
    /// Returns `None` silently if the file doesn't exist, or with a warning if it's malformed.
    pub fn load(explicit_path: Option<&Path>) -> Option<Self> {
        let path = explicit_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("auwgent.yml"));
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let content = std::fs::read_to_string(&path).ok()?;
        match serde_yaml::from_str::<Config>(&content) {
            Ok(mut cfg) => {
                if let Some(source) = cfg.source.as_deref() {
                    cfg.source = Some(resolve_against_base(source, &base_dir, true));
                }
                if let Some(output) = cfg.output.as_deref() {
                    cfg.output = Some(resolve_against_base(output, &base_dir, false));
                }
                Some(cfg)
            }
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {}", path.display(), e);
                None
            }
        }
    }
}

fn resolve_against_base(input: &str, base_dir: &Path, normalize_for_glob: bool) -> String {
    let p = Path::new(input);
    if p.is_absolute() {
        return input.to_string();
    }

    let joined = base_dir.join(p);
    let resolved = joined.to_string_lossy().to_string();
    if normalize_for_glob {
        resolved.replace('\\', "/")
    } else {
        resolved
    }
}
