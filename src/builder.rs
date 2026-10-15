//! The build tool hyprcross drives: Rune when it is installed, Cargo otherwise.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone)]
pub struct Builder {
    pub kind: BuilderKind,
    pub path: PathBuf,
}

impl Builder {
    /// `auto` is Rune when it is on PATH, else Cargo.
    pub fn resolve(preference: &str) -> Result<Self> {
        let found =
            |kind: BuilderKind| find_executable(kind.name()).map(|path| Self { kind, path });
        match preference {
            "rune" => found(BuilderKind::Rune).ok_or_else(|| {
                anyhow::anyhow!("`rune` was chosen as the builder, but it is not on PATH")
            }),
            "cargo" => {
                found(BuilderKind::Cargo).ok_or_else(|| anyhow::anyhow!("`cargo` is not on PATH"))
            }
            "auto" | "" => match found(BuilderKind::Rune).or_else(|| found(BuilderKind::Cargo)) {
                Some(b) => Ok(b),
                None => bail!("neither `rune` nor `cargo` is on PATH"),
            },
            other => bail!("unknown builder `{other}`: expected `auto`, `rune` or `cargo`"),
        }
    }

    pub fn command(&self, subcommand: &str) -> Command {
        let mut cmd = Command::new(&self.path);
        cmd.arg(subcommand);
        cmd
    }

    /// The builder's arguments for `subcommand` on `target`. Rune links with its own hyprlnk
    /// where hyprlnk links the target, unless `prefer_hyprlnk` is off; Cargo gets its linker
    /// through the environment.
    pub fn args(&self, subcommand: &str, target: &str, prefer_hyprlnk: bool) -> Vec<String> {
        let mut args = Vec::new();
        if self.kind == BuilderKind::Rune && !prefer_hyprlnk {
            args.extend([
                "--config".to_string(),
                "build.linker=\"default\"".to_string(),
            ]);
        }
        args.push(subcommand.to_string());
        if subcommand != "clean" || self.kind == BuilderKind::Cargo {
            args.extend(["--target".to_string(), target.to_string()]);
        }
        args
    }
}

/// The first `name` on PATH that is an executable file.
pub fn find_executable(name: &str) -> Option<PathBuf> {
    if name.contains(std::path::MAIN_SEPARATOR) {
        let p = PathBuf::from(name);
        return is_executable(&p).then_some(p);
    }
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
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

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rune_is_told_to_keep_its_default_linker_only_when_asked() {
        let rune = Builder {
            kind: BuilderKind::Rune,
            path: "rune".into(),
        };
        assert_eq!(
            rune.args("build", "aarch64-unknown-linux-gnu", true),
            ["build", "--target", "aarch64-unknown-linux-gnu"]
        );
        assert_eq!(
            rune.args("test", "x", false),
            [
                "--config",
                "build.linker=\"default\"",
                "test",
                "--target",
                "x"
            ]
        );
        assert_eq!(
            rune.args("clean", "x", true),
            ["clean"],
            "rune clean takes no target"
        );
        let cargo = Builder {
            kind: BuilderKind::Cargo,
            path: "cargo".into(),
        };
        assert_eq!(cargo.args("clean", "x", true), ["clean", "--target", "x"]);
    }
}
