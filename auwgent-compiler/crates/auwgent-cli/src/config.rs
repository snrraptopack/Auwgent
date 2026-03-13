use serde::Deserialize;
use std::path::Path;

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

        let content = std::fs::read_to_string(&path).ok()?;
        match serde_yaml::from_str(&content) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {}", path.display(), e);
                None
            }
        }
    }
}
