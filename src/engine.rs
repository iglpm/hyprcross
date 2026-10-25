//! Execution engine: runs builds directly on the host using target SDKs and hyprlnk.

use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Instant;

use crate::builder::Builder;
use crate::config::Config;
use crate::direct::DirectEngine;
use crate::targets::{TargetInfo, TargetOs};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Direct,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Direct => "direct",
        }
    }
}

/// Whether output goes to a terminal.
pub fn on_terminal() -> bool {
    std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
}

/// (rows, columns) of the terminal.
pub fn terminal_size() -> (u16, u16) {
    console::Term::stdout().size_checked().unwrap_or((24, 80))
}

pub struct ExecutionContext {
    pub target: TargetInfo,
    pub config: Config,
    pub builder: Builder,
    pub engine_override: Option<String>,
    pub linker_override: Option<String>,
}

impl ExecutionContext {
    pub fn new(
        target_triple: &str,
        config: Config,
        builder: Builder,
        engine_override: Option<String>,
        linker_override: Option<String>,
    ) -> Self {
        Self {
            target: TargetInfo::parse(target_triple),
            config,
            builder,
            engine_override,
            linker_override,
        }
    }

    /// Whether hyprlnk is the chosen linker (`--linker`, the target's or the build's setting).
    pub fn prefers_hyprlnk(&self) -> bool {
        let pref = self
            .linker_override
            .as_deref()
            .or_else(|| {
                self.config
                    .get_target(&self.target.triple)
                    .and_then(|t| t.linker.as_deref())
            })
            .unwrap_or(&self.config.build.linker);
        pref == "hyprlnk"
    }

    /// Whether hyprlnk links this build: chosen, and able to link the target.
    pub fn should_use_hyprlnk(&self) -> bool {
        self.prefers_hyprlnk() && self.target.hyprlnk_supported
    }

    pub fn decide_engine(&self) -> Result<Engine> {
        Ok(Engine::Direct)
    }

    pub fn run_command(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        let start = Instant::now();
        let engine = self.decide_engine()?;
        let hyprlnk = self.prefers_hyprlnk();

        let mut details = vec![engine.name(), self.builder.kind.name()];
        if self.should_use_hyprlnk() {
            details.push("hyprlnk");
        }
        ui::action(
            verb(subcommand),
            format!(
                "{} {}",
                self.target.triple.bold(),
                ui::dim(format!("· {}", details.join(" · ")))
            ),
        );

        let code = DirectEngine::new(&self.target, &self.config, &self.builder, hyprlnk)
            .execute(subcommand, extra_args)?;

        if code != 0 {
            self.explain_failure(code);
        }
        ui::trace(format!(
            "{subcommand} took {} in all",
            ui::secs(start.elapsed())
        ));
        Ok(code)
    }

    fn explain_failure(&self, _code: i32) {
        let t = &self.target;
        if t.os == TargetOs::MacOS {
            ui::hint(
                "if a C dependency failed to link against a missing framework, set HYPRCROSS_MACOS_SDK_PATH to a full Xcode SDK",
            );
        }
    }
}

fn verb(subcommand: &str) -> &'static str {
    match subcommand {
        "build" => "building",
        "check" => "checking",
        "test" => "testing",
        "run" => "running",
        "bench" => "benchmarking",
        "clippy" => "linting",
        "doc" => "documenting",
        "clean" => "cleaning",
        _ => "running",
    }
}

/// Builds natively on the host using automatic target SDK bootstrapping and hyprlnk linking.
pub fn auto_engine(_target: &TargetInfo, _config: &Config) -> Engine {
    Engine::Direct
}

/// Write a Hyprcross.toml for `target` in the current directory.
pub fn init_project(target: Option<&str>) -> Result<()> {
    let path = Path::new("Hyprcross.toml");
    if path.exists() {
        ui::warn("Hyprcross.toml already exists here; leaving it as it is");
        return Ok(());
    }
    let default_target = target.unwrap_or("aarch64-unknown-linux-gnu");
    let content = format!(
        r#"# hyprcross: https://github.com/runepm/hyprcross

[build]
# "direct" runs cross-compilation directly on host with target SDKs.
engine = "direct"

# "hyprlnk", the linker built into hyprcross and Igel, or "default".
linker = "hyprlnk"

# "auto" (Igel if it is installed, else Cargo), "igl" or "cargo".
builder = "auto"

# The target when --target is left out.
default_target = "{default_target}"

[target.{default_target}]
# Variables for the build:
# env = {{ OPENSSL_STATIC = "1" }}
"#
    );
    fs::write(path, content)?;
    ui::success(format!("created Hyprcross.toml for {default_target}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::current_host_triple;

    #[test]
    fn the_host_and_targets_build_directly() {
        let config = Config::default();
        for t in [
            current_host_triple(),
            "wasm32-unknown-unknown",
            "thumbv7em-none-eabihf",
            "wasm32-wasip1",
            "aarch64-unknown-linux-gnu",
        ] {
            assert_eq!(
                auto_engine(&TargetInfo::parse(t), &config),
                Engine::Direct,
                "{t}"
            );
        }
    }
}
