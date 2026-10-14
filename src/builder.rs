use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderKind {
    Rune,
    Cargo,
}

impl BuilderKind {
    pub fn name(&self) -> &'static str {
        match self {
            BuilderKind::Rune => "rune",
            BuilderKind::Cargo => "cargo",
        }
    }
}

pub struct Builder {
    pub kind: BuilderKind,
    pub path: PathBuf,
}

impl Builder {
    /// Detect or resolve the builder based on preferences
    pub fn resolve(preference: &str) -> Result<Self> {
        match preference {
            "rune" => {
                let path = find_executable("rune")
                    .ok_or_else(|| anyhow::anyhow!("`rune` was specified as builder but was not found in PATH"))?;
                Ok(Self {
                    kind: BuilderKind::Rune,
                    path,
                })
            }
            "cargo" => {
                let path = find_executable("cargo")
                    .ok_or_else(|| anyhow::anyhow!("`cargo` was not found in PATH"))?;
                Ok(Self {
                    kind: BuilderKind::Cargo,
                    path,
                })
            }
            "auto" | "" => {
                // Prefer rune if installed, otherwise cargo
                if let Some(path) = find_executable("rune") {
                    Ok(Self {
                        kind: BuilderKind::Rune,
                        path,
                    })
                } else if let Some(path) = find_executable("cargo") {
                    Ok(Self {
                        kind: BuilderKind::Cargo,
                        path,
                    })
                } else {
                    bail!("Neither `rune` nor `cargo` was found in PATH");
                }
            }
            other => bail!("Unknown builder preference `{other}`: expected `auto`, `rune`, or `cargo`"),
        }
    }

    /// Create a process Command for this builder with the specified subcommand
    pub fn command(&self, subcommand: &str) -> Command {
        let mut cmd = Command::new(&self.path);
        cmd.arg(subcommand);
        cmd
    }
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}
