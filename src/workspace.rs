//! Workspace and external path dependency discovery for hyprcross
//! Automatically detects path dependencies outside the project directory
//! and mounts them into containers so Cargo can resolve `path = "../..."` transparently.

use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ui;

/// Normalize a path relative to a container base path (e.g. base = "/project", rel = "../hyprlnk")
pub fn normalize_container_path(base: &str, rel_path: &str) -> PathBuf {
    let mut parts: Vec<String> = base
        .split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    for seg in rel_path.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        } else if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg.to_string());
        }
    }

    let mut p = PathBuf::from("/");
    for seg in parts {
        p.push(seg);
    }
    p
}

/// Recursively find all `path = "..."` dependency strings in a TOML Value
fn extract_path_strings(val: &toml::Value, out: &mut Vec<String>) {
    match val {
        toml::Value::Table(tbl) => {
            for (k, v) in tbl {
                if k == "path" {
                    if let toml::Value::String(s) = v {
                        out.push(s.clone());
                    }
                } else {
                    extract_path_strings(v, out);
                }
            }
        }
        toml::Value::Array(arr) => {
            for item in arr {
                extract_path_strings(item, out);
            }
        }
        _ => {}
    }
}

/// Discover any external path dependencies referenced in Cargo.toml that reside outside `cwd`.
/// Returns a list of `(host_path, container_mount_path)`.
pub fn discover_external_path_dependencies(cwd: &Path) -> Vec<(PathBuf, PathBuf)> {
    let manifest_path = cwd.join("Cargo.toml");
    if !manifest_path.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let parsed: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut raw_paths = Vec::new();
    extract_path_strings(&parsed, &mut raw_paths);

    let canonical_cwd = match cwd.canonicalize() {
        Ok(c) => c,
        Err(_) => cwd.to_path_buf(),
    };
    #[cfg(test)]
    println!("canonical_cwd: {:?}", canonical_cwd);

    let mut mounts = Vec::new();
    let mut seen_hosts = HashSet::new();

    for rel_str in raw_paths {
        // Only inspect relative paths pointing upwards (e.g. "../...")
        let rel_p = Path::new(&rel_str);
        let resolved = cwd.join(rel_p);
        if let Ok(canon) = resolved.canonicalize()
            && !canon.starts_with(&canonical_cwd)
            && canon.is_dir()
            && seen_hosts.insert(canon.clone())
        {
            let container_p = normalize_container_path("/project", &rel_str);
            mounts.push((canon, container_p));
        }
    }

    mounts
}

/// Ensure that the required target standard library is installed via rustup
pub fn ensure_rustup_target(target: &str) -> Result<()> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let is_installed = stdout.lines().any(|l| l.trim() == target);
        if !is_installed {
            ui::info("rustup", format!("Target `{target}` is not installed. Installing via rustup..."));
            let status = Command::new("rustup")
                .args(["target", "add", target])
                .status()?;
            if !status.success() {
                ui::warn(format!("Failed to auto-install target `{target}` via rustup. Build may fail if sysroot is missing."));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_container_path() {
        assert_eq!(
            normalize_container_path("/project", "../hyprlnk"),
            PathBuf::from("/hyprlnk")
        );
        assert_eq!(
            normalize_container_path("/project", "../../shared"),
            PathBuf::from("/shared")
        );
        assert_eq!(
            normalize_container_path("/project", "../vendor/lib"),
            PathBuf::from("/vendor/lib")
        );
    }

    #[test]
    fn test_discover_dependencies() {
        let deps = discover_external_path_dependencies(Path::new("."));
        assert!(!deps.is_empty());
        assert_eq!(deps[0].1, PathBuf::from("/hyprlnk"));
    }
}
