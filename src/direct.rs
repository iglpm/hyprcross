use anyhow::Result;
use std::process::{Command, Stdio};

use crate::builder::Builder;
use crate::config::Config;
use crate::hyprlnk_driver;
use crate::qemu::QemuRunner;
use crate::targets::TargetInfo;
use crate::ui;

pub struct DirectEngine<'a> {
    pub target: &'a TargetInfo,
    pub config: &'a Config,
    pub builder: &'a Builder,
    pub use_hyprlnk: bool,
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

    /// Check if the target is already installed via rustup
    pub fn check_rustup_target(&self) -> bool {
        let output = Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout.lines().any(|line| line.trim() == self.target.triple);
        }
        false
    }

    /// Execute the build or command directly on the host
    pub fn execute(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        ui::info(
            "direct",
            format!(
                "Running {} via {} on host (0ms container latency)",
                subcommand,
                self.builder.kind.name()
            ),
        );

        if !self.check_rustup_target() {
            ui::warn(format!(
                "Target `{}` does not appear to be installed via rustup. You may need: rustup target add {}",
                self.target.triple, self.target.triple
            ));
        }

        let mut cmd = self.builder.command(subcommand);

        // Inject target flag
        cmd.arg("--target").arg(&self.target.triple);

        // Forward extra arguments
        for arg in extra_args {
            cmd.arg(arg);
        }

        // Configure hyprlnk if enabled
        if self.use_hyprlnk && hyprlnk_driver::supports_target(&self.target.triple) {
            ui::info("hyprlnk", format!("Accelerating linker for target {}", self.target.triple));
            let flags = hyprlnk_driver::get_rustflags(&self.target.triple, true);
            let combined_flags = flags.join(" ");

            if let Ok(existing) = std::env::var("RUSTFLAGS") {
                cmd.env("RUSTFLAGS", format!("{existing} {combined_flags}"));
            } else {
                cmd.env("RUSTFLAGS", combined_flags);
            }
        }

        // Configure runner for test and run commands
        if matches!(subcommand, "test" | "run") {
            let target_cfg = self.config.get_target(&self.target.triple);
            let custom_runner = target_cfg.and_then(|t| t.runner.as_deref());
            let sysroot = target_cfg.and_then(|t| t.sysroot.as_deref());

            if let Some(runner) = QemuRunner::resolve(self.target, custom_runner, sysroot) {
                let env_key = format!(
                    "CARGO_TARGET_{}_RUNNER",
                    self.target.triple.replace('-', "_").to_uppercase()
                );
                let runner_cmd = runner.runner_command();
                ui::info("qemu", format!("Configuring target runner: {runner_cmd}"));
                cmd.env(env_key, runner_cmd);
            }
        }

        // Apply custom target environment variables from config
        if let Some(target_cfg) = self.config.get_target(&self.target.triple) {
            for (k, v) in &target_cfg.env {
                cmd.env(k, v);
            }
        }

        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }
}
