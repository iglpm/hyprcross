use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::builder::{find_executable, Builder};
use crate::config::Config;
use crate::hyprlnk_driver;
use crate::targets::TargetInfo;
use crate::ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerEngineKind {
    Docker,
    Podman,
    Finch,
    Nerdctl,
}

impl ContainerEngineKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
            Self::Finch => "finch",
            Self::Nerdctl => "nerdctl",
        }
    }
}

pub struct ContainerEngine {
    pub kind: ContainerEngineKind,
    pub path: PathBuf,
}

impl ContainerEngine {
    pub fn detect(preference: &str) -> Result<Self> {
        let candidates = match preference {
            "docker" => vec![("docker", ContainerEngineKind::Docker)],
            "podman" => vec![("podman", ContainerEngineKind::Podman)],
            "finch" => vec![("finch", ContainerEngineKind::Finch)],
            "nerdctl" => vec![("nerdctl", ContainerEngineKind::Nerdctl)],
            "auto" | "" => vec![
                ("podman", ContainerEngineKind::Podman),
                ("docker", ContainerEngineKind::Docker),
                ("finch", ContainerEngineKind::Finch),
                ("nerdctl", ContainerEngineKind::Nerdctl),
            ],
            other => bail!("Unknown container engine `{other}`: expected `docker`, `podman`, or `auto`"),
        };

        for (name, kind) in candidates {
            if let Some(path) = find_executable(name) {
                return Ok(Self { kind, path });
            }
        }

        bail!("No container engine (docker, podman, etc.) found in PATH");
    }

    pub fn check_service_available(&self) -> bool {
        if self.kind == ContainerEngineKind::Podman {
            // Podman is daemonless and rootless by design; always immediately available
            return true;
        }

        let output = Command::new(&self.path)
            .args(["version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        matches!(output, Ok(status) if status.success())
    }
}

pub struct ContainerRunner<'a> {
    pub engine: &'a ContainerEngine,
    pub target: &'a TargetInfo,
    pub config: &'a Config,
    pub builder: &'a Builder,
    pub use_hyprlnk: bool,
}

impl<'a> ContainerRunner<'a> {
    pub fn new(
        engine: &'a ContainerEngine,
        target: &'a TargetInfo,
        config: &'a Config,
        builder: &'a Builder,
        use_hyprlnk: bool,
    ) -> Self {
        Self {
            engine,
            target,
            config,
            builder,
            use_hyprlnk,
        }
    }

    /// Resolve container image for the target with validation
    pub fn resolve_image(&self) -> Result<String> {
        if let Some(tc) = self.config.get_target(&self.target.triple)
            && let Some(img) = &tc.image {
                return Ok(img.clone());
            }

        if !self.target.has_prebuilt_image() {
            let alt_msg = if let Some(alt) = self.target.suggested_alternative() {
                format!("\n  • For Windows in a container, use GNU ABI: hyprcross build --target {alt}")
            } else {
                String::new()
            };
            bail!(
                "Pre-built container images for `{}` cannot be hosted publicly due to proprietary vendor SDK licensing terms.{}\n  • For direct builds without containers: hyprcross build --target {} --engine direct\n  • Or specify a custom container image in Hyprcross.toml:\n      [target.{}]\n      image = \"my-custom-image:tag\"",
                self.target.triple,
                alt_msg,
                self.target.triple,
                self.target.triple
            );
        }

        Ok(self.target.default_image.clone())
    }

    /// Execute command inside standard container run (`docker run --rm ...`)
    pub fn execute(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        let image = self.resolve_image()?;
        let cwd = std::env::current_dir()?;

        ui::info(
            "container",
            format!("Spawning {} container using image `{}`", self.engine.kind.name(), image),
        );

        let mut cmd = Command::new(&self.engine.path);
        cmd.arg("run");
        cmd.arg("--rm");
        cmd.arg("-i");

        #[cfg(unix)]
        if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
            cmd.arg("-t");
        }

        let _ = crate::workspace::ensure_rustup_target(&self.target.triple);

        let is_podman = self.engine.kind == ContainerEngineKind::Podman;
        let mount_opt = if is_podman { ":z" } else { "" };
        cmd.arg("-v").arg(format!("{}:/project{}", cwd.display(), mount_opt));
        cmd.arg("-w").arg("/project");

        // Discover and mount external path dependencies (e.g. `../hyprlnk`)
        for (host_path, container_path) in crate::workspace::discover_external_path_dependencies(&cwd) {
            ui::info(
                "workspace",
                format!(
                    "Mounting external path dependency {} -> {}",
                    host_path.display(),
                    container_path.display()
                ),
            );
            cmd.arg("-v").arg(format!(
                "{}:{}{}",
                host_path.display(),
                container_path.display(),
                mount_opt
            ));
        }

        #[cfg(unix)]
        {
            if !is_podman {
                let uid = unsafe { libc::getuid() };
                let gid = unsafe { libc::getgid() };
                cmd.arg("--user").arg(format!("{uid}:{gid}"));
            } else {
                cmd.arg("--userns=keep-id");
            }
        }

        // Mount host cargo cache
        if let Some(cargo_home) = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| {
            crate::config::dirs_home().map(|h| h.join(".cargo"))
        })
            && cargo_home.exists() {
                cmd.arg("-v").arg(format!("{}:/cargo{}", cargo_home.display(), mount_opt));
                cmd.arg("-e").arg("CARGO_HOME=/cargo");
            }

        // Mount host rustup toolchain directory so cargo & rustc are available
        if let Some(rustup_home) = std::env::var_os("RUSTUP_HOME").map(PathBuf::from).or_else(|| {
            crate::config::dirs_home().map(|h| h.join(".rustup"))
        })
            && rustup_home.exists() {
                cmd.arg("-v").arg(format!("{}:/rustup{}", rustup_home.display(), mount_opt));
                cmd.arg("-e").arg("RUSTUP_HOME=/rustup");
            }

        // Ensure host cargo binaries are in PATH
        cmd.arg("-e").arg("PATH=/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");

        let ro_mount_opt = if is_podman { ":ro,z" } else { ":ro" };

        // Mount hyprlnk binary into container if available and supported
        if self.use_hyprlnk && self.target.hyprlnk_supported
            && let Some(hyprlnk_path) = hyprlnk_driver::find_external_hyprlnk()
                .or_else(hyprlnk_driver::current_exe)
            {
                cmd.arg("-v").arg(format!(
                    "{}:/usr/local/bin/hyprlnk{}",
                    hyprlnk_path.display(),
                    ro_mount_opt
                ));
                let env_key = format!(
                    "CARGO_TARGET_{}_LINKER",
                    self.target.triple.replace('-', "_").to_uppercase()
                );
                cmd.arg("-e").arg(format!("{env_key}=/usr/local/bin/hyprlnk"));

                if let Some(cc) = self.target.target_c_compiler() {
                    cmd.arg("-e").arg(format!("HYPRLNK_CC={cc}"));
                }

                ui::info("hyprlnk", "Mounted hyprlnk into container at /usr/local/bin/hyprlnk");
            }

        // Mount host rune cache and binary into container if using rune
        if self.builder.kind == crate::builder::BuilderKind::Rune {
            if self.builder.path.exists() {
                cmd.arg("-v").arg(format!(
                    "{}:/usr/local/bin/rune{}",
                    self.builder.path.display(),
                    ro_mount_opt
                ));
            }
            if let Some(rune_home) = std::env::var_os("RUNE_HOME").map(PathBuf::from).or_else(|| {
                crate::config::dirs_home().map(|h| h.join(".rune"))
            })
                && rune_home.exists() {
                    cmd.arg("-v").arg(format!("{}:/rune{}", rune_home.display(), mount_opt));
                    cmd.arg("-e").arg("RUNE_HOME=/rune");
                }
            ui::info("rune", format!("Using Rune build engine inside container ({})", self.builder.path.display()));
        }

        if let Ok(backtrace) = std::env::var("RUST_BACKTRACE") {
            cmd.arg("-e").arg(format!("RUST_BACKTRACE={backtrace}"));
        }

        if let Some(tc) = self.config.get_target(&self.target.triple) {
            for vol in &tc.volumes {
                cmd.arg("-v").arg(vol);
            }
            for (k, v) in &tc.env {
                cmd.arg("-e").arg(format!("{k}={v}"));
            }
        }

        cmd.arg(&image);

        // Command to execute inside container
        cmd.arg(self.builder.kind.name());
        cmd.arg(subcommand);
        cmd.arg("--target").arg(&self.target.triple);

        for arg in extra_args {
            cmd.arg(arg);
        }

        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }
}
