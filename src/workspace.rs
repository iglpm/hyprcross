//! The project on disk: the workspace structure, and whether the toolchain has the
//! target's standard library, both found without running Cargo or rustup.

use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ui;

/// The directory to mount for a build in `cwd`: the root of the workspace it belongs to (the
/// nearest directory above with a `[workspace]` manifest that doesn't `exclude` it), else its
/// package, else `cwd`.
pub fn project_root(cwd: &Path) -> PathBuf {
    let mut package: Option<PathBuf> = None;
    for dir in cwd.ancestors() {
        let manifest = dir.join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        if package.is_none() {
            package = Some(dir.to_path_buf());
        }
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        let Some(workspace) = value.get("workspace") else {
            continue;
        };
        // As for Cargo, a package under an excluded path is a project of its own.
        let excluded = workspace
            .get("exclude")
            .and_then(|e| e.as_array())
            .into_iter()
            .flatten()
            .filter_map(|e| e.as_str())
            .any(|e| {
                let path: PathBuf = Path::new(e)
                    .components()
                    .filter(|c| !matches!(c, std::path::Component::CurDir))
                    .collect();
                package
                    .as_deref()
                    .is_some_and(|p| p != dir && p.starts_with(dir.join(path)))
            });
        if excluded {
            break;
        }
        return dir.to_path_buf();
    }
    package.unwrap_or_else(|| cwd.to_path_buf())
}

/// Every string value of a `path` key, anywhere in a manifest.
fn path_values(val: &toml::Value, out: &mut Vec<String>) {
    match val {
        toml::Value::Table(tbl) => {
            for (k, v) in tbl {
                match (k.as_str(), v) {
                    ("path", toml::Value::String(s)) => out.push(s.clone()),
                    _ => path_values(v, out),
                }
            }
        }
        toml::Value::Array(arr) => arr.iter().for_each(|v| path_values(v, out)),
        _ => {}
    }
}

/// Directories outside `root` that the manifests in `root` and `cwd` point at with `path = …`
/// (path dependencies, `[patch]` entries).
pub fn external_path_dependencies(root: &Path, cwd: &Path) -> Vec<PathBuf> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in [root.clone(), cwd.to_path_buf()] {
        let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let Ok(manifest) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        let mut paths = Vec::new();
        path_values(&manifest, &mut paths);
        for p in paths {
            if let Ok(canon) = dir.join(&p).canonicalize()
                && canon.is_dir()
                && !canon.starts_with(&root)
                && seen.insert(canon.clone())
            {
                out.push(canon);
            }
        }
    }
    // A directory inside another one needs no mount of its own.
    let all = out.clone();
    out.retain(|p| !all.iter().any(|q| q != p && p.starts_with(q)));
    out
}

/// `RUSTUP_HOME`, if rustup manages this machine's Rust.
pub fn rustup_home() -> Option<PathBuf> {
    std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::config::dirs_home().map(|h| h.join(".rustup")))
        .filter(|p| p.join("toolchains").is_dir())
}

pub fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::config::dirs_home().map(|h| h.join(".cargo")))
        .filter(|p| p.is_dir())
}

/// The toolchain rustup runs in `cwd`, found the way rustup finds it: `RUSTUP_TOOLCHAIN`, then
/// per directory upwards an override (`rustup override set`) or a toolchain file, then the
/// default toolchain.
pub fn active_toolchain(cwd: &Path) -> Option<PathBuf> {
    let home = rustup_home()?;
    let settings = std::fs::read_to_string(home.join("settings.toml"))
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok());

    let name = std::env::var("RUSTUP_TOOLCHAIN").ok().or_else(|| {
        cwd.ancestors().find_map(|dir| {
            let over = settings
                .as_ref()
                .and_then(|s| s.get("overrides"))
                .and_then(|o| o.get(dir.to_str()?))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            over.or_else(|| toolchain_file(dir))
        })
    });
    let name = name.or_else(|| {
        settings
            .as_ref()?
            .get("default_toolchain")?
            .as_str()
            .map(str::to_string)
    })?;

    if name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        return Some(PathBuf::from(name));
    }
    let toolchains = home.join("toolchains");
    let host = crate::targets::current_host_triple();
    [
        toolchains.join(&name),
        toolchains.join(format!("{name}-{host}")),
    ]
    .into_iter()
    .find(|p| p.is_dir())
}

/// The toolchain a `rust-toolchain.toml` (or legacy `rust-toolchain`) in `dir` names.
fn toolchain_file(dir: &Path) -> Option<String> {
    for name in ["rust-toolchain.toml", "rust-toolchain"] {
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
            continue;
        };
        if let Ok(v) = toml::from_str::<toml::Value>(&text)
            && let Some(tc) = v.get("toolchain")
        {
            return tc
                .get("path")
                .or_else(|| tc.get("channel"))
                .and_then(|c| c.as_str())
                .map(str::to_string);
        }
        let line = text.trim();
        if !line.is_empty() && !line.contains('\n') {
            return Some(line.to_string());
        }
    }
    None
}

/// Whether the active toolchain has `target`'s standard library: `None` when that can't be
/// told without asking rustup (no rustup, or a toolchain it doesn't keep in RUSTUP_HOME).
pub fn has_target_std(cwd: &Path, target: &str) -> Option<bool> {
    let toolchain = active_toolchain(cwd)?;
    Some(
        toolchain
            .join("lib/rustlib")
            .join(target)
            .join("lib")
            .is_dir(),
    )
}

/// Install `target`'s standard library with rustup when the active toolchain lacks it. Costs a
/// few file lookups when it is installed.
pub fn ensure_rustup_target(cwd: &Path, target: &str) -> Result<()> {
    if has_target_std(cwd, target) != Some(false) {
        return Ok(());
    }
    let activity = ui::Activity::start("installing", format!("the {target} standard library"));
    ui::trace(format!("rustup target add {target}"));
    let out = Command::new("rustup")
        .args(["target", "add", target])
        .current_dir(cwd)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            activity.finish("installed", format!("the {target} standard library"));
        }
        Ok(o) => {
            activity.clear();
            ui::warn(format!(
                "rustup could not install the {target} standard library"
            ));
            let stderr = String::from_utf8_lossy(&o.stderr);
            if let Some(line) = stderr.lines().find(|l| !l.trim().is_empty()) {
                ui::hint(line.trim().trim_start_matches("error: "));
            }
        }
        Err(e) => {
            activity.clear();
            ui::warn(format!("cannot run rustup to install {target}: {e}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn the_workspace_root_is_what_gets_mounted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"app\"]\n",
        );
        write(&root.join("app/Cargo.toml"), "[package]\nname = \"app\"\n");
        assert_eq!(project_root(&root.join("app")), root);
        assert_eq!(project_root(&root.join("app/src")), root);

        let lone = tempfile::tempdir().unwrap();
        write(
            &lone.path().join("Cargo.toml"),
            "[package]\nname = \"lone\"\n",
        );
        assert_eq!(project_root(&lone.path().join("src")), lone.path());
    }

    #[test]
    fn excluded_packages_are_projects_of_their_own() {
        // hyprcross inside the Igel repository: under its directory, but excluded.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"app\"]\nexclude = [\"other\", \"./tool\"]\n",
        );
        write(&root.join("app/Cargo.toml"), "[package]\nname = \"app\"\n");
        write(
            &root.join("tool/Cargo.toml"),
            "[package]\nname = \"tool\"\n",
        );
        write(
            &root.join("tool/dep/Cargo.toml"),
            "[package]\nname = \"dep\"\n",
        );
        assert_eq!(project_root(&root.join("tool/src")), root.join("tool"));
        assert_eq!(project_root(&root.join("tool/dep")), root.join("tool/dep"));
        assert_eq!(project_root(&root.join("app/src")), root);
        assert_eq!(project_root(root), root);
    }

    #[test]
    fn path_dependencies_outside_the_project_are_found_once() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        write(
            &base.join("app/Cargo.toml"),
            "[package]\nname = \"app\"\n[lib]\npath = \"src/lib.rs\"\n\
             [dependencies]\nshared = { path = \"../shared\" }\ninner = { path = \"../shared/inner\" }\n\
             [patch.crates-io]\nfoo = { path = \"../vendor/foo\" }\nlocal = { path = \"local\" }\n",
        );
        for d in ["app/src", "app/local", "shared/inner", "vendor/foo"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        let app = base.join("app");
        let mut deps = external_path_dependencies(&app, &app);
        deps.sort();
        assert_eq!(deps, vec![base.join("shared"), base.join("vendor/foo")]);
    }

    #[test]
    fn toolchain_files_in_both_formats() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85\"\n",
        );
        assert_eq!(toolchain_file(dir.path()).as_deref(), Some("1.85"));
        let legacy = tempfile::tempdir().unwrap();
        write(
            &legacy.path().join("rust-toolchain"),
            "nightly-2026-01-01\n",
        );
        assert_eq!(
            toolchain_file(legacy.path()).as_deref(),
            Some("nightly-2026-01-01")
        );
    }
}
