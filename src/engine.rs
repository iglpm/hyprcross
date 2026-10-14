use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::builder::Builder;
use crate::config::Config;
use crate::container::{ContainerEngine, ContainerRunner};
use crate::daemon::ContainerDaemon;
use crate::direct::DirectEngine;
use crate::targets::TargetInfo;
use crate::ui;

pub struct ExecutionContext {
    pub target: TargetInfo,
    pub config: Config,
    pub builder: Builder,
    pub engine_override: Option<String>,
    pub linker_override: Option<String>,
    pub daemon_override: Option<bool>,
}

impl ExecutionContext {
    pub fn new(
        target_triple: &str,
        config: Config,
        builder: Builder,
        engine_override: Option<String>,
        linker_override: Option<String>,
        daemon_override: Option<bool>,
    ) -> Self {
        let target = TargetInfo::parse(target_triple);
        Self {
            target,
            config,
            builder,
            engine_override,
            linker_override,
            daemon_override,
        }
    }

    /// Determine whether hyprlnk should be used for this build
    pub fn should_use_hyprlnk(&self) -> bool {
        let pref = self
            .linker_override
            .as_deref()
            .or_else(|| {
                self.config
                    .get_target(&self.target.triple)
                    .and_then(|t| t.linker.as_deref())
            })
            .unwrap_or(&self.config.build.linker);

        pref == "hyprlnk" && self.target.hyprlnk_supported
    }

    /// Select execution engine mode: "direct" or "container"
    pub fn decide_engine(&self) -> String {
        if let Some(e) = &self.engine_override {
            return e.clone();
        }

        if let Some(tc) = self.config.get_target(&self.target.triple)
            && let Some(e) = &tc.engine {
                return e.clone();
            }

        match self.config.build.engine.as_str() {
            "direct" => "direct".to_string(),
            "container" => "container".to_string(),
            _ => {
                // If direct mode is natively supported and container engine is not strictly required,
                // prefer direct mode for 0ms startup!
                if self.target.direct_supported {
                    "direct".to_string()
                } else {
                    "container".to_string()
                }
            }
        }
    }

    /// Run the given subcommand with arguments
    pub fn run_command(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        let start = Instant::now();
        let engine_choice = self.decide_engine();
        let use_hyprlnk = self.should_use_hyprlnk();

        let code = match engine_choice.as_str() {
            "direct" => {
                let direct = DirectEngine::new(&self.target, &self.config, &self.builder, use_hyprlnk);
                direct.execute(subcommand, extra_args)?
            }
            "container" => {
                let container_engine = ContainerEngine::detect(&self.config.build.container_engine)?;
                let use_daemon = self
                    .daemon_override
                    .unwrap_or(self.config.build.daemon);

                if use_daemon && container_engine.check_service_available() {
                    let daemon = ContainerDaemon::new(
                        &container_engine,
                        &self.target,
                        &self.config,
                        &self.builder,
                        use_hyprlnk,
                    );
                    daemon.execute(subcommand, extra_args)?
                } else {
                    let runner = ContainerRunner::new(
                        &container_engine,
                        &self.target,
                        &self.config,
                        &self.builder,
                        use_hyprlnk,
                    );
                    runner.execute(subcommand, extra_args)?
                }
            }
            other => bail!("Unknown engine mode `{other}`: expected `direct` or `container`"),
        };

        ui::timing(&format!("{subcommand} for {}", self.target.triple), start.elapsed());
        Ok(code)
    }
}

/// Initialize a new Hyprcross.toml in the current directory
pub fn init_project(target: Option<&str>) -> Result<()> {
    let path = Path::new("Hyprcross.toml");
    if path.exists() {
        ui::warn("Hyprcross.toml already exists in this directory");
        return Ok(());
    }

    let default_target = target.unwrap_or("aarch64-unknown-linux-gnu");
    let content = format!(
r#"# Hyprcross configuration
# Documentation: https://github.com/runepm/hyprcross

[build]
# Execution engine: "auto" (direct when possible, container otherwise), "direct", or "container"
engine = "auto"

# Linker acceleration: "hyprlnk" (2x-4x faster linking) or "default"
linker = "hyprlnk"

# Build orchestrator: "auto" (uses Rune CAS if installed, otherwise Cargo), "rune", or "cargo"
builder = "auto"

# Keep container warm in background for instant sub-20ms invocations
daemon = true

# Default target when --target is omitted
default_target = "{default_target}"

[target.{default_target}]
# Pre-built cross image
image = "ghcr.io/cross-rs/{default_target}:edge"

# Environment variables
# env = {{ MY_VAR = "value" }}

# Custom volume mounts
# volumes = ["/host/path:/container/path:ro"]
"#);

    fs::write(path, content)?;
    ui::success("init", "Created Hyprcross.toml with hyper-accelerated defaults");
    Ok(())
}
