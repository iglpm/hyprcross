//! Direct mode: the builder runs on the host, with the target's C compiler (when the host has
//! one) as its linker driver and an emulator as its runner.

use anyhow::Result;
use std::process::Command;

use crate::builder::{Builder, BuilderKind, find_executable};
use crate::config::Config;
use crate::hyprlnk_driver;
use crate::qemu;
use crate::targets::TargetInfo;
use crate::ui;

pub struct DirectEngine<'a> {
    pub target: &'a TargetInfo,
    pub config: &'a Config,
    pub builder: &'a Builder,
    pub use_hyprlnk: bool,
}

/// `aarch64-unknown-linux-gnu` → `AARCH64_UNKNOWN_LINUX_GNU`, as in `CARGO_TARGET_<…>_LINKER`.
pub fn target_env_key(triple: &str) -> String {
    triple.replace(['-', '.'], "_").to_uppercase()
}

/// The target's C compiler on this machine: `CC_<triple>` or `CARGO_TARGET_<…>_LINKER` if set,
/// else the first of its usual names on PATH.
pub fn host_c_compiler(target: &TargetInfo) -> Option<String> {
    let key = target_env_key(&target.triple);
    for var in [
        format!("CARGO_TARGET_{key}_LINKER"),
        format!("CC_{}", target.triple.replace(['-', '.'], "_")),
        format!("CC_{}", target.triple),
    ] {
        if let Ok(v) = std::env::var(&var)
            && !v.is_empty()
        {
            return Some(v);
        }
    }
    target
        .c_compiler_candidates()
        .into_iter()
        .find(|name| find_executable(name).is_some())
}

impl<'a> DirectEngine<'a> {
    pub fn new(
        target: &'a TargetInfo,
        config: &'a Config,
        builder: &'a Builder,
        use_hyprlnk: bool,
    ) -> Self {
        Self {
            target,
            config,
            builder,
            use_hyprlnk,
        }
    }

    pub fn command(&self, subcommand: &str, extra_args: &[String]) -> Result<Command> {
        let cwd = std::env::current_dir()?;
        if subcommand != "clean" {
            crate::workspace::ensure_rustup_target(&cwd, &self.target.triple)?;
        }

        let triple = &self.target.triple;
        let key = target_env_key(triple);
        let linker_var = format!("CARGO_TARGET_{key}_LINKER");
        let hyprlnk = self.use_hyprlnk && hyprlnk_driver::supports_target(triple);

        let mut cmd = Command::new(&self.builder.path);
        cmd.args(self.builder.args(subcommand, triple, self.use_hyprlnk));
        cmd.args(extra_args);

        // Without a linker named for it, rustc links a cross target with the host's `cc`, which
        // can't. Name the target's C compiler: Rune makes it hyprlnk's driver, Cargo runs it.
        let driver = (!self.target.is_host())
            .then(|| host_c_compiler(self.target))
            .flatten();
        if std::env::var_os(&linker_var).is_none()
            && let Some(cc) = &driver
        {
            cmd.env(&linker_var, cc);
        }

        if hyprlnk && self.builder.kind == BuilderKind::Cargo {
            match hyprlnk_driver::linker_for_cargo() {
                Some(linker) => {
                    cmd.env(&linker_var, linker);
                }
                None => {
                    if let Some(exe) = hyprlnk_driver::current_exe() {
                        cmd.env(&linker_var, exe);
                        cmd.env(hyprlnk_driver::LINKER_ENV, "1");
                    }
                }
            }
            if let Some(cc) = driver.filter(|c| c != "cc") {
                cmd.env(hyprlnk_driver::CC_ENV, cc);
            }
            if let Some(flavor) = hyprlnk_driver::rustc_linker_flavor(triple) {
                let flag = format!("-C linker-flavor={flavor}");
                match std::env::var("RUSTFLAGS") {
                    Ok(existing) => cmd.env("RUSTFLAGS", format!("{existing} {flag}")),
                    Err(_) => cmd.env(format!("CARGO_TARGET_{key}_RUSTFLAGS"), flag),
                };
            }
        }

        if matches!(subcommand, "test" | "run" | "bench") {
            let runner_var = format!("CARGO_TARGET_{key}_RUNNER");
            let tc = self.config.get_target(triple);
            if std::env::var_os(&runner_var).is_none()
                && let Some(runner) = qemu::host_runner_command(
                    self.target,
                    tc.and_then(|t| t.runner.as_deref()),
                    tc.and_then(|t| t.sysroot.as_deref()),
                )
            {
                ui::trace(format!("{runner_var}={runner}"));
                cmd.env(runner_var, runner);
            }
        }

        let tc = self.config.get_target(triple);
        for (k, v) in self
            .config
            .build
            .env
            .iter()
            .chain(tc.map(|t| t.env.iter()).into_iter().flatten())
        {
            cmd.env(k, v);
        }
        Ok(cmd)
    }

    pub fn execute(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        let mut cmd = self.command(subcommand, extra_args)?;
        ui::trace(format!("{cmd:?}"));
        Ok(cmd.status()?.code().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_keys() {
        assert_eq!(
            target_env_key("aarch64-unknown-linux-gnu"),
            "AARCH64_UNKNOWN_LINUX_GNU"
        );
        assert_eq!(
            target_env_key("thumbv8m.main-none-eabi"),
            "THUMBV8M_MAIN_NONE_EABI"
        );
    }

    #[test]
    fn rune_gets_no_linker_flags_from_hyprcross() {
        let target = TargetInfo::parse(crate::targets::current_host_triple());
        let builder = Builder {
            kind: BuilderKind::Rune,
            path: "rune".into(),
        };
        let config = Config::default();
        let engine = DirectEngine::new(&target, &config, &builder, true);
        let cmd = engine.command("build", &[]).unwrap();
        let envs: Vec<_> = cmd
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(
            !envs
                .iter()
                .any(|k| k == "RUSTFLAGS" || k.ends_with("_RUSTFLAGS") || k.ends_with("_LINKER")),
            "Rune links with its own hyprlnk and keeps its cache: {envs:?}"
        );
    }
}
