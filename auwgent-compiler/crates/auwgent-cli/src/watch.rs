use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::commands::generate_file;

// ── Dependency Graph Helpers ─────────────────────────────────────────────────

/// Collect all `.agent` files transitively imported by `root`.
/// Returns a set of canonical absolute paths (excluding `root` itself).
fn collect_import_deps(root: &Path) -> HashSet<PathBuf> {
    let mut deps = HashSet::new();
    let mut visited = HashSet::new();
    collect_import_deps_recursive(root, &mut deps, &mut visited);
    deps.remove(root);
    deps
}

fn collect_import_deps_recursive(
    file: &Path,
    deps: &mut HashSet<PathBuf>,
    visited: &mut HashSet<PathBuf>,
) {
    let Ok(canonical) = std::fs::canonicalize(file) else {
        return;
    };
    if !visited.insert(canonical.clone()) {
        return; // already visited
    }
    deps.insert(canonical.clone());

    // Read and parse the file to find imports
    let Ok(source) = std::fs::read_to_string(&canonical) else {
        return;
    };
    let (tokens, _) = auwgent_lexer::tokenize(&source);
    let (model, _) = auwgent_parser::parse(&tokens);

    for import in &model.imports {
        if let Ok(import_path) =
            auwgent_analysis::resolve_import_path(&canonical, &import.path.value)
        {
            collect_import_deps_recursive(&import_path, deps, visited);
        }
    }
}

/// Build a reverse-dependency map: dep_file → set of root files that depend on it.
///
/// When `dep_file` changes, all files in the mapped set should be re-compiled.
fn build_reverse_deps(roots: &[PathBuf]) -> HashMap<PathBuf, HashSet<PathBuf>> {
    let mut reverse: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    for root in roots {
        let Ok(canonical_root) = std::fs::canonicalize(root) else {
            continue;
        };
        // Root always maps to itself
        reverse
            .entry(canonical_root.clone())
            .or_default()
            .insert(root.clone());

        for dep in collect_import_deps(&canonical_root) {
            reverse.entry(dep).or_default().insert(root.clone());
        }
    }
    reverse
}

// ── Watch Entry Point ────────────────────────────────────────────────────────

/// Run an initial generation pass then watch for `.agent` file changes,
/// regenerating any file that is modified or created (including parent files
/// that import a changed dependency).
pub fn watch_and_generate(
    files: &[PathBuf],
    watch_roots: &[PathBuf],
    targets: &[String],
    output: Option<&Path>,
) {
    // Initial pass
    if !files.is_empty() {
        eprintln!(
            "\x1b[34mGenerating {} file(s) initially...\x1b[0m",
            files.len()
        );
        for file in files {
            for target in targets {
                generate_file(file, target, output);
            }
        }
    }

    // Build the initial reverse-dependency graph so we know which root agents
    // must be re-compiled when a shared import changes.
    let mut reverse_deps = build_reverse_deps(files);

    // Collect unique directories to watch.
    let mut seen = HashSet::new();
    let mut watch_dirs: Vec<PathBuf> = if !watch_roots.is_empty() {
        watch_roots
            .iter()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .filter(|p| seen.insert(p.clone()))
            .collect()
    } else {
        files
            .iter()
            .filter_map(|f| f.parent())
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .filter(|p| seen.insert(p.clone()))
            .collect()
    };

    if watch_dirs.is_empty() {
        watch_dirs.push(PathBuf::from("."));
    }

    eprintln!("\x1b[34mWatching for .agent changes... (Ctrl+C to quit)\x1b[0m");

    let output_owned = output.map(|p| p.to_path_buf());
    let targets_owned = targets.to_vec();
    let files_owned = files.to_vec();

    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = match recommended_watcher(move |ev| {
        let _ = tx.send(ev);
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to start file watcher: {}", e);
            std::process::exit(1);
        }
    };

    for dir in &watch_dirs {
        if let Err(e) = watcher.watch(dir.as_path(), RecursiveMode::Recursive) {
            eprintln!("Warning: could not watch {}: {}", dir.display(), e);
        }
    }

    // Debounce: collect events for 80 ms before acting (handles editors that
    // do a delete + create instead of a plain modify)
    let debounce = Duration::from_millis(80);
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_event = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                let relevant = matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_));
                if relevant {
                    for path in event.paths {
                        if path.extension().map_or(false, |e| e == "agent") {
                            pending.insert(path);
                            last_event = Instant::now();
                        }
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {}", e),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if !pending.is_empty() && last_event.elapsed() >= debounce {
            let changed_paths: Vec<_> = pending.drain().collect();
            eprintln!();

            // Collect the set of root files that need to be re-compiled.
            // This includes:
            //   1. The changed file itself (if it is a root file)
            //   2. All root files that transitively import any changed file
            let mut to_recompile: HashSet<PathBuf> = HashSet::new();

            for changed_path in &changed_paths {
                let canonical =
                    std::fs::canonicalize(changed_path).unwrap_or_else(|_| changed_path.clone());

                eprintln!("\x1b[33m↻\x1b[0m  {} changed", changed_path.display());

                if let Some(dependents) = reverse_deps.get(&canonical) {
                    for root in dependents {
                        to_recompile.insert(root.clone());
                    }
                } else {
                    // Changed file is not yet in the graph (newly created).
                    // Try to add it to the set directly.
                    to_recompile.insert(changed_path.clone());

                    // Rebuild the dep graph to pick up the new file
                    reverse_deps = build_reverse_deps(&files_owned);
                }
            }

            for path in &to_recompile {
                for target in &targets_owned {
                    generate_file(path, target, output_owned.as_deref());
                }
            }

            // Rebuild the dep graph after recompilation in case imports changed
            reverse_deps = build_reverse_deps(&files_owned);
        }
    }
}
