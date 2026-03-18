use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::commands::generate_file;

/// Run an initial generation pass then watch for `.agent` file changes,
/// regenerating any file that is modified or created.
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

    // Collect unique directories to watch.
    // Use explicit roots when provided (from CLI path / config source),
    // otherwise fall back to parent dirs of discovered files.
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
                let relevant =
                    matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_));
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
            let paths: Vec<_> = pending.drain().collect();
            eprintln!();
            for path in &paths {
                for target in &targets_owned {
                    generate_file(path, target, output_owned.as_deref());
                }
            }
        }
    }
}
