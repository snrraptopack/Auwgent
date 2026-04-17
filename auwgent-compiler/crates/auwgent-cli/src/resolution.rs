use std::path::{Path, PathBuf};

pub fn resolve_sources(path: Option<&Path>, config_source: Option<&str>) -> Vec<PathBuf> {
    if let Some(p) = path {
        return resolve_input(&p.to_string_lossy());
    }
    if let Some(source) = config_source {
        return resolve_input(source);
    }
    collect_agent_files(Path::new("."))
}

pub fn resolve_watch_roots(path: Option<&Path>, config_source: Option<&str>) -> Vec<PathBuf> {
    if let Some(p) = path {
        return input_watch_roots(&p.to_string_lossy());
    }
    if let Some(source) = config_source {
        return input_watch_roots(source);
    }
    vec![PathBuf::from(".")]
}

pub fn resolve_targets(target_arg: Option<&str>, config_targets: Option<&[String]>) -> Vec<String> {
    match target_arg {
        Some("both") => vec!["ts".to_string(), "python".to_string(), "dart".to_string(), "rust".to_string()],
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

pub fn collect_agent_files(dir: &Path) -> Vec<PathBuf> {
    let dir_str = dir.to_string_lossy().replace('\\', "/");
    expand_glob(&format!("{}/**/*.agent", dir_str))
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

fn expand_glob(pattern: &str) -> Vec<PathBuf> {
    match glob::glob(pattern) {
        Ok(paths) => paths
            .filter_map(|e| e.ok())
            .filter(|p| p.extension().map_or(false, |e| e == "agent"))
            .collect(),
        Err(_) => Vec::new(),
    }
}
