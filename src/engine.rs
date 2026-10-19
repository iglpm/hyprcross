//! Where a build runs (on the host, or in a container) and running it there.

use anyhow::{Result, bail};
use colored::Colorize;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Instant;

use crate::builder::{Builder, find_executable};
use crate::config::Config;
use crate::container::{ContainerEngine, ContainerRunner, command_env};
use crate::daemon::{ContainerDaemon, Job};
use crate::direct::{DirectEngine, host_c_compiler};
use crate::targets::{TargetInfo, TargetOs};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Direct,
    Container,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Direct => "direct",
            Engine::Container => "container",
        }
    }
}

/// Whether output goes to a terminal: then builds in containers run on a pseudo-terminal too.
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
        Self {
            target: TargetInfo::parse(target_triple),
            config,
            builder,
            engine_override,
            linker_override,
            daemon_override,
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
        let pref = self
            .engine_override
            .clone()
            .or_else(|| {
                self.config
                    .get_target(&self.target.triple)
                    .and_then(|t| t.engine.clone())
            })
            .unwrap_or_else(|| self.config.build.engine.clone());
        match pref.as_str() {
            "direct" => Ok(Engine::Direct),
            "container" => Ok(Engine::Container),
            "auto" | "" => Ok(auto_engine(&self.target, &self.config)),
            other => bail!("unknown engine `{other}`: expected `auto`, `direct` or `container`"),
        }
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

        let code = match engine {
            Engine::Direct => DirectEngine::new(&self.target, &self.config, &self.builder, hyprlnk)
                .execute(subcommand, extra_args)?,
            Engine::Container => self.run_in_container(subcommand, extra_args, hyprlnk)?,
        };

        if code != 0 {
            self.explain_failure(engine, code);
        }
        ui::trace(format!(
            "{subcommand} took {} in all",
            ui::secs(start.elapsed())
        ));
        Ok(code)
    }

    fn run_in_container(
        &self,
        subcommand: &str,
        extra_args: &[String],
        hyprlnk: bool,
    ) -> Result<i32> {
        let engine = ContainerEngine::detect(&self.config.build.container_engine)?;
        if !engine.check_service_available() {
            bail!(
                "{} is installed but not answering: {}",
                engine.kind.name(),
                engine.kind.service_hint()
            );
        }
        let cwd = std::env::current_dir()?;
        if subcommand != "clean" && cfg!(target_os = "linux") {
            crate::workspace::ensure_rustup_target(&cwd, &self.target.triple)?;
        }
        let (program, args) = if cfg!(target_os = "linux") {
            let mut a = self.builder.args(subcommand, &self.target.triple, hyprlnk);
            a.extend(extra_args.iter().cloned());
            (self.builder.kind.name().to_string(), a)
        } else {
            let cargo_builder = Builder {
                kind: crate::builder::BuilderKind::Cargo,
                path: "cargo".into(),
            };
            let mut a = cargo_builder.args(subcommand, &self.target.triple, false);
            a.extend(extra_args.iter().cloned());
            ("cargo".to_string(), a)
        };
        // Programs and tests may read their input; builds never do.
        let stdin = matches!(subcommand, "run" | "test" | "bench");

        let container_builder = if cfg!(target_os = "linux") {
            self.builder.clone()
        } else {
            Builder {
                kind: crate::builder::BuilderKind::Cargo,
                path: "cargo".into(),
            }
        };

        let use_daemon = self.daemon_override.unwrap_or(self.config.build.daemon);
        if use_daemon && engine.check_service_available() {
            let daemon =
                ContainerDaemon::new(&engine, &self.target, &self.config, &container_builder)?;
            let env = command_env(
                &self.target,
                &self.config,
                &container_builder,
                hyprlnk,
                on_terminal(),
            );
            daemon.execute(Job {
                program: &program,
                args: &args,
                env,
                cwd: &cwd,
                stdin,
            })
        } else {
            ContainerRunner::new(
                &engine,
                &self.target,
                &self.config,
                &container_builder,
                hyprlnk,
            )
            .execute(&program, &args, stdin)
        }
    }

    fn explain_failure(&self, engine: Engine, code: i32) {
        let t = &self.target;
        if engine == Engine::Direct && t.os == TargetOs::MacOS {
            ui::hint(
                "if a C dependency failed to link against a missing framework, set HYPRCROSS_MACOS_SDK_PATH to a full Xcode SDK",
            );
        } else if engine == Engine::Direct
            && !t.is_host()
            && host_c_compiler(t).is_none()
            && !t.links_without_c_toolchain()
        {
            ui::hint(format!(
                "this machine has no C compiler for {} (it would be {}); `--engine container` \
                 builds in an image that has one",
                t.triple,
                t.c_compiler_candidates()
                    .first()
                    .map_or("unknown", String::as_str),
            ));
        } else if code == 127 && engine == Engine::Container {
            ui::hint(format!(
                "`{}` could not be run in the container: the image may lack what it needs",
                self.builder.kind.name()
            ));
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

/// `auto`: builds natively on the host using automatic target SDK bootstrapping
/// and hyprlnk linking. Container execution is used only when a custom image or Dockerfile
/// is configured for the target.
pub fn auto_engine(target: &TargetInfo, config: &Config) -> Engine {
    let tc = config.get_target(&target.triple);
    if tc.is_some_and(|t| t.image.is_some() || t.dockerfile.is_some())
        && ContainerEngine::detect(&config.build.container_engine).is_ok()
    {
        return Engine::Container;
    }
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
# "auto": on this machine when it can build the target, in a container otherwise.
# "direct" or "container" to always use one.
engine = "auto"

# "hyprlnk", the linker built into hyprcross and Rune, or "default".
linker = "hyprlnk"

# "auto" (Rune if it is installed, else Cargo), "rune" or "cargo".
builder = "auto"

# Keep a warm container per target, so container builds start instantly.
daemon = true

# The target when --target is left out.
default_target = "{default_target}"

# Where the per-target images come from: <image_repository>/<target>:<image_tag>.
# image_repository = "ghcr.io/runepm/hyprcross"
# image_tag = "latest"

[target.{default_target}]
# An image of your own instead of hyprcross':
# image = "my-image:tag"
# or one built from a Dockerfile (FROM $HYPRCROSS_BASE_IMAGE to extend hyprcross'):
# dockerfile = "docker/Dockerfile.cross"

# Commands run as root when the warm container is created:
# pre_build = ["apt-get update && apt-get install -y libssl-dev:arm64"]

# Variables for the build:
# env = {{ OPENSSL_STATIC = "1" }}
"#
    );
    fs::write(path, content)?;
    ui::success(format!("created Hyprcross.toml for {default_target}"));
    Ok(())
}

/// Whether a container engine is on PATH at all.
pub fn any_container_engine() -> bool {
    crate::container::ContainerEngineKind::ALL
        .iter()
        .any(|k| find_executable(k.name()).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::current_host_triple;

    #[test]
    fn the_host_and_targets_without_c_toolchains_build_directly() {
        let config = Config::default();
        for t in [
            current_host_triple(),
            "wasm32-unknown-unknown",
            "thumbv7em-none-eabihf",
            "wasm32-wasip1",
        ] {
            assert_eq!(
                auto_engine(&TargetInfo::parse(t), &config),
                Engine::Direct,
                "{t}"
            );
        }
    }

    #[test]
    fn explicit_engines_win() {
        let builder = Builder {
            kind: crate::builder::BuilderKind::Cargo,
            path: "cargo".into(),
        };
        let ctx = ExecutionContext::new(
            "aarch64-unknown-linux-gnu",
            Config::default(),
            builder,
            Some("container".into()),
            Some("default".into()),
            None,
        );
        assert_eq!(ctx.decide_engine().unwrap(), Engine::Container);
        assert!(!ctx.should_use_hyprlnk());
    }
}
