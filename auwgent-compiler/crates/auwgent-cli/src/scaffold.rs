use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn run_scaffolding(target_out_dir: &Path, project_root: &Path, targets: &[String]) {
    let Some(repo_root) = find_repo_root(project_root) else {
        return;
    };
    let registry = load_registry(&repo_root);

    for target in targets {
        let normalized = normalize_target(target);
        let Some(entry) = registry.get(normalized) else {
            continue;
        };

        let local_sdk_path = repo_root.join(&entry.path);

        let scaffold_shared_dir = if let Some(shared) = &entry.shared_root {
            let shared_path = repo_root.join(shared);
            let _ = std::fs::create_dir_all(&shared_path);
            shared_path
        } else {
            target_out_dir.to_path_buf()
        };

        match normalized {
            "typescript" => scaffold_typescript(
                &scaffold_shared_dir,
                project_root,
                &entry.path,
                &entry.package,
            ),
            "python" => scaffold_python(&scaffold_shared_dir, project_root, &entry.path),
            _ => {}
        }

        // Shared: Generate secrets in the SDK itself
        scaffold_secrets(&repo_root, &local_sdk_path, normalized);
    }
}

fn normalize_target(target: &str) -> &str {
    match target {
        "ts" | "typescript" => "typescript",
        "py" | "python" => "python",
        _ => target,
    }
}

fn scaffold_typescript(
    shared_root: &Path,
    project_dir: &Path,
    sdk_rel_path: &str,
    package_name: &str,
) {
    let Some(repo_root) = find_repo_root(project_dir) else {
        return;
    };

    // 1. Shared Root: package.json
    let pkg_path = shared_root.join("package.json");
    if !pkg_path.exists() {
        // Calculate relative path from shared_root to targets/typescript
        let sdk_abs_path = repo_root.join(sdk_rel_path);
        let canon_shared = std::fs::canonicalize(shared_root).unwrap_or(shared_root.to_path_buf());
        let canon_sdk = std::fs::canonicalize(&sdk_abs_path).unwrap_or(sdk_abs_path.clone());

        let rel_sdk = pathdiff::diff_paths(&canon_sdk, &canon_shared).unwrap_or(sdk_abs_path);
        let rel_sdk_str = rel_sdk.to_string_lossy().to_string().replace('\\', "/");
        let rel_sdk_str = if rel_sdk_str.starts_with(r"//?/") {
            rel_sdk_str[4..].to_string()
        } else if rel_sdk_str.starts_with(r"\\?\") {
            rel_sdk_str[4..].to_string()
        } else {
            rel_sdk_str
        };

        let package_json = json!({
            "name": "auwgent-test-project",
            "private": true,
            "type": "module",
            "description": "Shared development environment for Auwgent tests.",
            "scripts": {
                "dev": "bun index.ts || bun index.js || node index.js"
            },
            "dependencies": {
                package_name: format!("link:{}", rel_sdk_str)
            }
        });
        let _ = std::fs::write(
            &pkg_path,
            serde_json::to_string_pretty(&package_json).unwrap(),
        );
        eprintln!(
            "\x1b[34mℹ\x1b[0m Scaffolded {} with local SDK link",
            pkg_path.display()
        );
    }

    // 2. Shared Root: tsconfig.json (workspace-level resolution)
    let shared_tsconfig = shared_root.join("tsconfig.json");
    if !shared_tsconfig.exists() {
        let sdk_abs_path = repo_root.join(sdk_rel_path);
        let canon_shared = std::fs::canonicalize(shared_root).unwrap_or(shared_root.to_path_buf());
        let canon_sdk = std::fs::canonicalize(&sdk_abs_path).unwrap_or(sdk_abs_path.clone());

        if let Some(rel_path) = pathdiff::diff_paths(&canon_sdk, &canon_shared) {
            let mut rel_str = rel_path.to_string_lossy().to_string().replace('\\', "/");
            if rel_str.starts_with(r"//?/") {
                rel_str = rel_str[4..].to_string();
            } else if rel_str.starts_with(r"\\?\") {
                rel_str = rel_str[4..].to_string();
            }
            let entry_point = if rel_str.ends_with('/') {
                format!("{}auwgent.ts", rel_str)
            } else {
                format!("{}/auwgent.ts", rel_str)
            };

            let tsconfig = json!({
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "target": "ESNext",
                    "skipLibCheck": true,
                    "baseUrl": ".",
                    "paths": {
                        package_name: [entry_point],
                        format!("{}/*", package_name): [format!("{}/*", rel_str)]
                    }
                }
            });
            let _ = std::fs::write(
                &shared_tsconfig,
                serde_json::to_string_pretty(&tsconfig).unwrap(),
            );
            eprintln!(
                "\x1b[34mℹ\x1b[0m Scaffolded {} for workspace resolution",
                shared_tsconfig.display()
            );
        }
    }

    // 3. Local Project: tsconfig.json (project-level resolution)
    let tsconfig_path = project_dir.join("tsconfig.json");
    if !tsconfig_path.exists() {
        let sdk_abs_path = repo_root.join(sdk_rel_path);
        let canon_proj = std::fs::canonicalize(project_dir).unwrap_or(project_dir.to_path_buf());
        let canon_sdk = std::fs::canonicalize(&sdk_abs_path).unwrap_or(sdk_abs_path.clone());

        if let Some(rel_path) = pathdiff::diff_paths(&canon_sdk, &canon_proj) {
            let mut rel_str = rel_path.to_string_lossy().to_string().replace('\\', "/");
            if rel_str.starts_with(r"//?/") {
                rel_str = rel_str[4..].to_string();
            } else if rel_str.starts_with(r"\\?\") {
                rel_str = rel_str[4..].to_string();
            }
            let entry_point = if rel_str.ends_with('/') {
                format!("{}auwgent.ts", rel_str)
            } else {
                format!("{}/auwgent.ts", rel_str)
            };

            let tsconfig = json!({
                "compilerOptions": {
                    "module": "ESNext",
                    "moduleResolution": "Bundler",
                    "target": "ESNext",
                    "skipLibCheck": true,
                    "baseUrl": ".",
                    "paths": {
                        package_name: [entry_point],
                        format!("{}/*", package_name): [format!("{}/*", rel_str)]
                    }
                }
            });
            let _ = std::fs::write(
                &tsconfig_path,
                serde_json::to_string_pretty(&tsconfig).unwrap(),
            );
            eprintln!(
                "\x1b[34mℹ\x1b[0m Scaffolded {} for project resolution",
                tsconfig_path.display()
            );
        }
    }

    // 4. Environment: Link .env to BOTH
    link_env_file(&repo_root, shared_root);
    link_env_file(&repo_root, project_dir);
}

fn scaffold_python(shared_root: &Path, project_dir: &Path, sdk_rel_path: &str) {
    let Some(repo_root) = find_repo_root(project_dir) else {
        return;
    };
    let sdk_abs_path = repo_root.join(sdk_rel_path);

    // 1. Shared Root: requirements.txt
    let req_path = shared_root.join("requirements.txt");
    if !req_path.exists() {
        let canon_shared = std::fs::canonicalize(shared_root).unwrap_or(shared_root.to_path_buf());
        let canon_sdk = std::fs::canonicalize(&sdk_abs_path).unwrap_or(sdk_abs_path.clone());

        let rel_sdk = pathdiff::diff_paths(&canon_sdk, &canon_shared).unwrap_or(sdk_abs_path);
        let rel_sdk_str = rel_sdk.to_string_lossy().to_string().replace('\\', "/");
        let rel_sdk_str = if rel_sdk_str.starts_with(r"//?/") {
            rel_sdk_str[4..].to_string()
        } else if rel_sdk_str.starts_with(r"\\?\") {
            rel_sdk_str[4..].to_string()
        } else {
            rel_sdk_str
        };

        let content = format!("-e {}", rel_sdk_str);
        let _ = std::fs::write(&req_path, content);
        eprintln!(
            "\x1b[34mℹ\x1b[0m Scaffolded {} with local SDK link",
            req_path.display()
        );
    }

    // 2. Environment: Link .env to BOTH
    link_env_file(&repo_root, shared_root);
    link_env_file(&repo_root, project_dir);
}

struct RegistryEntry {
    package: String,
    path: String,
    shared_root: Option<String>,
}

fn load_registry(repo_root: &Path) -> HashMap<String, RegistryEntry> {
    let mut map = HashMap::new();
    let reg_dir = repo_root.join(".auwgent");
    if let Ok(entries) = std::fs::read_dir(reg_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let (Some(pkg), Some(p)) =
                            (val["package"].as_str(), val["path"].as_str())
                        {
                            map.insert(
                                stem,
                                RegistryEntry {
                                    package: pkg.to_string(),
                                    path: p.to_string(),
                                    shared_root: val["shared_root"].as_str().map(|s| s.to_string()),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    map
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut curr = if start.exists() {
        std::fs::canonicalize(start).ok()?
    } else {
        std::fs::canonicalize(start.parent().unwrap_or(Path::new("."))).ok()?
    };
    loop {
        if curr.join(".auwgent").exists() {
            return Some(curr);
        }
        if let Some(parent) = curr.parent() {
            curr = parent.to_path_buf();
        } else {
            return None;
        }
    }
}

fn link_env_file(repo_root: &Path, target_dir: &Path) {
    let repo_env = repo_root.join(".env");
    if repo_env.exists() {
        let target_env = target_dir.join(".env");
        if !target_env.exists() {
            // Prefer symlink on non-windows or if possible, otherwise copy
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink(&repo_env, &target_env);
            }
            #[cfg(windows)]
            {
                // Fallback to copy for simplicity on Windows permissions
                let _ = std::fs::copy(&repo_env, &target_env);
            }
            eprintln!("\x1b[34mℹ\x1b[0m Linked .env from repository root");
        }
    }
}

fn scaffold_secrets(repo_root: &Path, sdk_path: &Path, target: &str) {
    let env_path = repo_root.join(".env");
    if !env_path.exists() {
        return;
    }

    let Ok(content) = std::fs::read_to_string(env_path) else {
        return;
    };
    let mut env_vars = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let mut value = v.trim().to_string();
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = value[1..value.len() - 1].to_string();
            }
            env_vars.insert(key.to_string(), value);
        }
    }

    match target {
        "typescript" => {
            let mut ts_file =
                String::from("// Auto-generated by Auwgent CLI for development. DO NOT COMMIT.\n");
            for (k, v) in &env_vars {
                ts_file.push_str(&format!("export const {} = \"{}\";\n", k, v));
            }
            let path = sdk_path.join("secrets.ts");
            let _ = std::fs::write(&path, ts_file);
            eprintln!(
                "\x1b[34mℹ\x1b[0m Generated {} with embedded secrets",
                path.display()
            );
        }
        "python" => {
            let mut py_file =
                String::from("# Auto-generated by Auwgent CLI for development. DO NOT COMMIT.\n");
            for (k, v) in &env_vars {
                py_file.push_str(&format!("{} = \"{}\"\n", k, v));
            }
            let path = sdk_path.join("secrets.py");
            let _ = std::fs::write(&path, py_file);
            eprintln!(
                "\x1b[34mℹ\x1b[0m Generated {} with embedded secrets",
                path.display()
            );
        }
        _ => {}
    }
}
