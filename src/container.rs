//! Containers: the engine that runs them, the image a target builds in, and what a build
//! container mounts. The project, `CARGO_HOME`, `RUSTUP_HOME` and `RUNE_HOME` are mounted at
//! the paths they have on the host, so paths in errors are the host's and the builder's caches
//! are shared with builds on the host.

use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::builder::{Builder, BuilderKind, find_executable};
use crate::config::Config;
use crate::targets::TargetInfo;
use crate::ui;

/// Where hyprcross puts its own executables (and Rune's) inside containers.
pub const TOOLS_DIR: &str = "/opt/hyprcross/bin";
/// Where the agent's socket directory is mounted inside containers.
pub const RUN_DIR: &str = "/run/hyprcross";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn service_hint(&self) -> &'static str {
        match (self, std::env::consts::OS) {
            (Self::Podman, "windows" | "macos") => {
                "start its virtual machine (`podman machine start`) or start Podman Desktop"
            }
            (Self::Podman, _) => {
                "verify podman works with `podman version` or start `podman.socket`"
            }
            (Self::Docker, "windows" | "macos") => {
                "start Docker Desktop or verify your container daemon is running"
            }
            (Self::Docker, _) => {
                "start its service (`sudo systemctl start docker`) or add yourself to its group"
            }
            (Self::Finch, _) => "start its virtual machine (`finch vm start`)",
            (Self::Nerdctl, _) => "start containerd (`sudo systemctl start containerd`)",
        }
    }

    pub const ALL: [ContainerEngineKind; 4] =
        [Self::Podman, Self::Docker, Self::Finch, Self::Nerdctl];
}

#[derive(Debug, Clone)]
pub struct ContainerEngine {
    pub kind: ContainerEngineKind,
    pub path: PathBuf,
}

impl ContainerEngine {
    /// The engine to use: the one named, or for `auto` the first of Podman, Docker, Finch and
    /// nerdctl on PATH. When `auto`, prefers an engine whose service is currently answering.
    pub fn detect(preference: &str) -> Result<Self> {
        let candidates: Vec<ContainerEngineKind> = match preference {
            "auto" | "" => ContainerEngineKind::ALL.to_vec(),
            name => match ContainerEngineKind::ALL.iter().find(|k| k.name() == name) {
                Some(k) => vec![*k],
                None => bail!(
                    "unknown container engine `{name}`: expected `auto`, `podman`, `docker`, \
                     `finch` or `nerdctl`"
                ),
            },
        };
        let mut installed = Vec::new();
        for kind in candidates {
            if let Some(path) = find_executable(kind.name()) {
                let engine = Self { kind, path };
                if preference != "auto" && !preference.is_empty() {
                    return Ok(engine);
                }
                if engine.check_service_available() {
                    return Ok(engine);
                }
                installed.push(engine);
            }
        }
        if let Some(engine) = installed.into_iter().next() {
            return Ok(engine);
        }
        bail!("no container engine on PATH (podman, docker, finch or nerdctl)")
    }

    pub fn is_podman(&self) -> bool {
        self.kind == ContainerEngineKind::Podman
    }

    pub fn command(&self) -> Command {
        Command::new(&self.path)
    }

    /// Whether the engine answers.
    pub fn check_service_available(&self) -> bool {
        self.command()
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// The environment of a local image; `None` if it isn't here.
    pub fn image_env(&self, image: &str) -> Option<Vec<(String, String)>> {
        let out = self
            .command()
            .args([
                "image",
                "inspect",
                "--format",
                "{{json .Config.Env}}",
                image,
            ])
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let first = text.lines().next().unwrap_or("null");
        let vars: Option<Vec<String>> = serde_json::from_str(first).ok()?;
        Some(
            vars.unwrap_or_default()
                .into_iter()
                .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.into(), v.into())))
                .collect(),
        )
    }

    /// Pull `image`, with a custom progress bar and clean status updates.
    pub fn pull(&self, image: &str) -> Result<()> {
        let progress = ui::PullProgress::start(image);
        ui::trace(format!("{} pull {image}", self.kind.name()));

        let mut child = self
            .command()
            .args(["pull", image])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("cannot run {}", self.kind.name()))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (tx, rx) = std::sync::mpsc::channel();
        let tx_out = tx.clone();
        let h_out = std::thread::spawn(move || {
            if let Some(out) = stdout {
                let reader = std::io::BufReader::new(out);
                for line in std::io::BufRead::lines(reader).map_while(Result::ok) {
                    let _ = tx_out.send(line);
                }
            }
        });
        let tx_err = tx;
        let h_err = std::thread::spawn(move || {
            if let Some(err) = stderr {
                let reader = std::io::BufReader::new(err);
                for line in std::io::BufRead::lines(reader).map_while(Result::ok) {
                    let _ = tx_err.send(line);
                }
            }
        });

        let mut error_lines: Vec<String> = Vec::new();
        while let Ok(line) = rx.recv() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(update) = parse_pull_progress(trimmed) {
                progress.update(update.percent, update.message);
            }
            error_lines.push(line);
        }
        let _ = h_out.join();
        let _ = h_err.join();

        let status = child.wait()?;
        if status.success() {
            progress.finish();
            return Ok(());
        }
        progress.clear();
        let joined = error_lines.join("\n");
        bail!("{}", last_error_line(joined.as_bytes(), "the pull failed"))
    }

    /// Build `tag` from `dockerfile`, with its directory as the context.
    pub fn build_image(&self, dockerfile: &Path, tag: &str, base: Option<&str>) -> Result<()> {
        let activity = ui::Activity::start("building", format!("image {tag}"));
        let context = dockerfile.parent().unwrap_or(Path::new("."));
        let mut cmd = self.command();
        cmd.arg("build").arg("-f").arg(dockerfile).args(["-t", tag]);
        if let Some(base) = base {
            // Cross-rs calls it CROSS_BASE_IMAGE; Dockerfiles written for it keep working.
            for name in ["HYPRCROSS_BASE_IMAGE", "CROSS_BASE_IMAGE"] {
                cmd.arg("--build-arg").arg(format!("{name}={base}"));
            }
        }
        cmd.arg(context);
        ui::trace(format!("{cmd:?}"));
        let out = cmd.stdin(Stdio::null()).output()?;
        if out.status.success() {
            activity.finish("built", format!("image {tag}"));
            return Ok(());
        }
        activity.clear();
        let log = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = log.lines().rev().take(15).collect();
        bail!(
            "building {} failed:\n{}",
            dockerfile.display(),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        )
    }
}

/// The gist of an engine's error chain (`a: b: c: 403 Forbidden` -> `403 Forbidden`).
fn reason(e: &anyhow::Error) -> String {
    let text = e.to_string();
    let mut parts: Vec<&str> = text.rsplit(": ").take(2).collect();
    parts.reverse();
    match parts.as_slice() {
        [context, last] if last.len() < 12 => format!("{context}: {last}"),
        [.., last] => last.to_string(),
        [] => text.clone(),
    }
}

fn last_error_line(stderr: &[u8], fallback: &str) -> String {
    let s = String::from_utf8_lossy(stderr);
    let meaningful = s.lines().map(|l| l.trim()).rfind(|l| {
        !l.is_empty()
            && !l.starts_with("Run 'docker")
            && !l.starts_with("See 'docker")
            && !l.starts_with("Run 'podman")
            && !l.starts_with("See 'podman")
    });
    meaningful
        .or_else(|| s.lines().rev().find(|l| !l.trim().is_empty()))
        .map(|l| l.trim().trim_start_matches("Error: ").to_string())
        .unwrap_or_else(|| fallback.to_string())
}

struct PullUpdate<'a> {
    percent: Option<u64>,
    message: &'a str,
}

fn parse_pull_progress(line: &str) -> Option<PullUpdate<'_>> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("copying blob") || lower.contains("copying config") {
        let percent = parse_transfer_percent(line);
        return Some(PullUpdate {
            percent,
            message: "copying image layers...",
        });
    }
    if lower.contains("downloading") {
        let percent = parse_transfer_percent(line);
        return Some(PullUpdate {
            percent,
            message: "downloading layers...",
        });
    }
    if lower.contains("extracting") {
        return Some(PullUpdate {
            percent: None,
            message: "extracting layers...",
        });
    }
    if lower.contains("pull complete") || lower.contains("download complete") {
        return Some(PullUpdate {
            percent: None,
            message: "layer complete",
        });
    }
    if lower.contains("verifying checksum") {
        return Some(PullUpdate {
            percent: None,
            message: "verifying layer checksums...",
        });
    }
    if lower.contains("getting image source signatures") {
        return Some(PullUpdate {
            percent: Some(10),
            message: "resolving signatures...",
        });
    }
    if lower.contains("writing manifest") {
        return Some(PullUpdate {
            percent: Some(95),
            message: "writing manifest...",
        });
    }
    if lower.contains("storing signatures") {
        return Some(PullUpdate {
            percent: Some(100),
            message: "storing signatures...",
        });
    }
    if lower.contains("trying to pull") {
        return Some(PullUpdate {
            percent: Some(5),
            message: "connecting to registry...",
        });
    }
    None
}

fn parse_transfer_percent(line: &str) -> Option<u64> {
    if let Some(pos) = line.find('%') {
        let before = &line[..pos];
        let num_str: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let num_str: String = num_str.chars().rev().collect();
        if let Ok(pct) = num_str.parse::<u64>() {
            return Some(pct.min(100));
        }
    }
    if let Some((left, right)) = line.split_once('/') {
        let left_val = extract_first_number(left.trim_end());
        let right_val = extract_first_number(right.trim_start());
        if let (Some(cur), Some(total)) = (left_val, right_val)
            && total > 0.0
        {
            let pct = (cur / total * 100.0) as u64;
            return Some(pct.min(100));
        }
    }

    None
}

fn extract_first_number(s: &str) -> Option<f64> {
    let mut num_str = String::new();
    let mut seen_digit = false;
    for c in s.chars().rev() {
        if c.is_ascii_digit() || (c == '.' && seen_digit) {
            num_str.push(c);
            seen_digit = true;
        } else if seen_digit {
            break;
        }
    }
    let reversed: String = num_str.chars().rev().collect();
    reversed.parse::<f64>().ok()
}

/// Which image a target asks for, before anything is pulled.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageRequest {
    /// `image = "…"` in the configuration.
    Configured(String),
    /// `dockerfile = "…"`: built locally, tagged by the file's contents.
    Dockerfile { path: PathBuf, tag: String },
    /// The target's hyprcross image, with cross-rs' as the fallback.
    Default {
        image: Option<String>,
        fallback: Option<String>,
    },
}

impl ImageRequest {
    pub fn for_target(target: &TargetInfo, config: &Config, cwd: &Path) -> Result<Self> {
        let tc = config.get_target(&target.triple);
        if let Some(image) = tc.and_then(|t| t.image.clone()) {
            return Ok(Self::Configured(image));
        }
        let default = target.image(&config.build.image_repository, &config.build.image_tag);
        let fallback = target.cross_rs_image();
        if let Some(file) = tc.and_then(|t| t.dockerfile.as_ref()) {
            let base = config
                .source
                .as_deref()
                .and_then(Path::parent)
                .unwrap_or(cwd);
            let path = base.join(file);
            let content = std::fs::read(&path)
                .with_context(|| format!("cannot read the Dockerfile {}", path.display()))?;
            let tag = format!(
                "localhost/hyprcross-custom/{}:{:016x}",
                target.triple,
                fnv1a(&[&content[..], default.as_deref().unwrap_or("").as_bytes()])
            );
            return Ok(Self::Dockerfile { path, tag });
        }
        if default.is_none() && fallback.is_none() {
            let mut msg = format!(
                "there is no prebuilt image for {}: its SDK can't be redistributed",
                target.triple
            );
            if let Some(alt) = target.suggested_alternative() {
                msg.push_str(&format!(
                    "\n  • for Windows in a container, use the GNU ABI: --target {alt}"
                ));
            }
            msg.push_str(&format!(
                "\n  • build on this machine instead: --engine direct\n  • or name an image of \
                 your own in {}:\n      [{}target.{}]\n      image = \"my-image:tag\"",
                crate::host::get().target_settings,
                if crate::host::get().builder.is_some() {
                    "cross."
                } else {
                    ""
                },
                target.triple
            ));
            bail!(msg);
        }
        // On non-Linux hosts (Windows, macOS), the host cannot mount its own Linux Rust
        // toolchain into the container. Prefer cross-rs images when available because they
        // contain pre-installed Linux rustc and cargo.
        #[cfg(not(target_os = "linux"))]
        let (primary, secondary) = if fallback.is_some()
            && config.build.image_repository == crate::targets::DEFAULT_IMAGE_REPOSITORY
        {
            (fallback, default)
        } else {
            (default, fallback)
        };
        #[cfg(target_os = "linux")]
        let (primary, secondary) = (default, fallback);

        Ok(Self::Default {
            image: primary,
            fallback: secondary,
        })
    }

    pub fn key(&self) -> String {
        match self {
            Self::Configured(i) => i.clone(),
            Self::Dockerfile { tag, .. } => tag.clone(),
            Self::Default { image, fallback } => {
                format!(
                    "{}|{}",
                    image.as_deref().unwrap_or(""),
                    fallback.as_deref().unwrap_or("")
                )
            }
        }
    }

    /// Make the image available locally: the name it has, and its environment.
    pub fn resolve(&self, engine: &ContainerEngine) -> Result<(String, Vec<(String, String)>)> {
        let ensure = |image: &str| -> Result<Vec<(String, String)>> {
            if let Some(env) = engine.image_env(image) {
                return Ok(env);
            }
            engine.pull(image)?;
            engine
                .image_env(image)
                .ok_or_else(|| anyhow::anyhow!("{image} was pulled but can't be inspected"))
        };
        match self {
            Self::Configured(image) => Ok((image.clone(), ensure(image)?)),
            Self::Dockerfile { path, tag } => {
                if let Some(env) = engine.image_env(tag) {
                    return Ok((tag.clone(), env));
                }
                engine.build_image(path, tag, None)?;
                Ok((tag.clone(), ensure(tag)?))
            }
            Self::Default { image, fallback } => {
                if let Some(image) = image {
                    match ensure(image) {
                        Ok(env) => return Ok((image.clone(), env)),
                        Err(e) if fallback.is_some() => {
                            if !engine.check_service_available() {
                                return Err(e);
                            }
                            ui::warn(format!("{image} is not available ({})", reason(&e)));
                            ui::hint("using cross-rs' image for this target instead");
                        }
                        Err(e) => return Err(e),
                    }
                }
                let fallback = fallback.as_ref().expect("checked in for_target");
                Ok((fallback.clone(), ensure(fallback)?))
            }
        }
    }
}

/// Translate a host path to its path inside the container.
///
/// On Windows, the workspace root is mounted to `/project`, so paths within `root` are translated
/// to `/project/...` with forward slashes. On Unix, host paths are mounted at their own paths.
pub fn to_container_path(host_path: &Path, root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(rel) = host_path.strip_prefix(root) {
            let s = rel.to_string_lossy().replace('\\', "/");
            if s.is_empty() {
                PathBuf::from("/project")
            } else {
                PathBuf::from(format!("/project/{}", s.trim_start_matches('/')))
            }
        } else {
            PathBuf::from("/project")
        }
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        host_path.to_path_buf()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mount {
    pub host: PathBuf,
    pub container: PathBuf,
    pub read_only: bool,
}

/// What a build container for a project and target is made of, apart from the image.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContainerSpec {
    pub mounts: Vec<Mount>,
    /// `volumes = [...]` from the configuration, passed as given.
    pub volumes: Vec<String>,
    pub env: Vec<(String, String)>,
    pub workdir: PathBuf,
    /// Size and modification time of the executables mounted into the container: a bind mount
    /// keeps showing the old file after a rebuild replaces it.
    pub tools: Vec<(PathBuf, u64, u64)>,
}

impl ContainerSpec {
    pub fn new(
        target: &TargetInfo,
        config: &Config,
        builder: &Builder,
        root: &Path,
        cwd: &Path,
        run_dir: Option<&Path>,
    ) -> Result<Self> {
        let mut mounts = Vec::new();
        let mut env = Vec::new();
        #[allow(unused_mut)]
        let mut tools = Vec::new();
        #[cfg(not(target_os = "linux"))]
        let _ = builder;

        #[cfg(not(windows))]
        {
            let same = |p: &Path, ro: bool| Mount {
                host: p.to_path_buf(),
                container: p.to_path_buf(),
                read_only: ro,
            };

            mounts.push(same(root, false));
            for dep in crate::workspace::external_path_dependencies(root, cwd) {
                mounts.push(same(&dep, false));
            }
            if let Some(cargo) = crate::workspace::cargo_home() {
                mounts.push(same(&cargo, false));
                env.push(("CARGO_HOME".into(), cargo.display().to_string()));
            }
            #[cfg(target_os = "linux")]
            if let Some(rustup) = crate::workspace::rustup_home() {
                mounts.push(same(&rustup, false));
                env.push(("RUSTUP_HOME".into(), rustup.display().to_string()));
            }
            if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)
                && dir.is_absolute()
                && !dir.starts_with(root)
            {
                std::fs::create_dir_all(&dir).ok();
                mounts.push(same(&dir, false));
            }
        }

        #[cfg(windows)]
        {
            // Mount workspace root to /project
            mounts.push(Mount {
                host: root.to_path_buf(),
                container: PathBuf::from("/project"),
                read_only: false,
            });

            // External path dependencies outside root
            for (i, dep) in crate::workspace::external_path_dependencies(root, cwd)
                .into_iter()
                .enumerate()
            {
                let container_dep = if let Ok(rel) = dep.strip_prefix(root.parent().unwrap_or(root)) {
                    let s = rel.to_string_lossy().replace('\\', "/");
                    PathBuf::from(format!("/{s}"))
                } else {
                    PathBuf::from(format!("/dependencies/{i}"))
                };
                mounts.push(Mount {
                    host: dep,
                    container: container_dep,
                    read_only: false,
                });
            }

            // Share Cargo cache (crates.io registry & git checkouts)
            if let Some(cargo) = crate::workspace::cargo_home() {
                mounts.push(Mount {
                    host: cargo,
                    container: PathBuf::from("/cargo"),
                    read_only: false,
                });
                env.push(("CARGO_HOME".into(), "/cargo".into()));
            }

            // Share Rune cache
            if let Some(rune) = std::env::var_os("RUNE_HOME")
                .map(PathBuf::from)
                .or_else(|| crate::config::dirs_home().map(|h| h.join(".rune")))
            {
                std::fs::create_dir_all(&rune).ok();
                mounts.push(Mount {
                    host: rune,
                    container: PathBuf::from("/rune"),
                    read_only: false,
                });
                env.push(("RUNE_HOME".into(), "/rune".into()));
            }

            if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)
                && dir.is_absolute()
                && !dir.starts_with(root)
            {
                std::fs::create_dir_all(&dir).ok();
                mounts.push(Mount {
                    host: dir,
                    container: PathBuf::from("/target"),
                    read_only: false,
                });
                env.push(("CARGO_TARGET_DIR".into(), "/target".into()));
            }
        }

        // Host binaries (hyprcross, hyprlnk, rune) are only mounted on Linux, where host
        // binaries match the container's Linux ELF format. On Windows and macOS, host executables
        // are PE/Mach-O and cannot be run by Linux containers.
        #[cfg(target_os = "linux")]
        {
            let tool = |host: &Path, name: &str, mounts: &mut Vec<Mount>| {
                mounts.push(Mount {
                    host: host.to_path_buf(),
                    container: Path::new(TOOLS_DIR).join(name),
                    read_only: true,
                });
            };
            let exe = std::env::current_exe().context("cannot find hyprcross' own executable")?;
            tool(&exe, "hyprcross", &mut mounts);
            tool(&exe, "hyprlnk", &mut mounts);
            tools.push(identity(&exe));

            if builder.kind == BuilderKind::Rune {
                tool(&builder.path, "rune", &mut mounts);
                tools.push(identity(&builder.path));
                let rune_home = std::env::var_os("RUNE_HOME")
                    .map(PathBuf::from)
                    .or_else(|| crate::config::dirs_home().map(|h| h.join(".rune")));
                if let Some(home) = rune_home {
                    std::fs::create_dir_all(&home).ok();
                    mounts.push(Mount {
                        host: home.clone(),
                        container: home,
                        read_only: false,
                    });
                    env.push(("RUNE_HOME".into(), home.display().to_string()));
                }
            }
        }

        if let Some(dir) = run_dir {
            mounts.push(Mount {
                host: dir.to_path_buf(),
                container: PathBuf::from(RUN_DIR),
                read_only: false,
            });
        }

        let tc = config.get_target(&target.triple);
        let mut volumes = Vec::new();
        let names = config
            .build
            .env
            .volumes
            .iter()
            .chain(tc.map(|t| t.env.volumes.iter()).into_iter().flatten());
        for name in names {
            if let Some(value) = std::env::var_os(name) {
                let path = PathBuf::from(&value);
                let container_path = to_container_path(&path, root);
                mounts.push(Mount {
                    host: path,
                    container: container_path.clone(),
                    read_only: false,
                });
                env.push((name.clone(), container_path.display().to_string()));
            }
        }
        if let Some(tc) = tc {
            volumes.extend(tc.volumes.iter().cloned());
        }
        env.push(("HYPRCROSS_CONTAINER".into(), "1".into()));

        Ok(Self {
            mounts,
            volumes,
            env,
            workdir: to_container_path(cwd, root),
            tools,
        })
    }

    /// Arguments for `run`: the user, mounts, environment and working directory. `image_path`
    /// is the image's own PATH, which ours extends.
    pub fn run_args(&self, engine: &ContainerEngine, image_path: Option<&str>) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::new();
        #[cfg(unix)]
        {
            if engine.is_podman() {
                args.push("--userns=keep-id".into());
            } else {
                // SAFETY: getuid and getgid can't fail.
                let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
                args.push("--user".into());
                args.push(format!("{uid}:{gid}").into());
            }
        }
        // Podman on Linux SELinux systems needs mounts relabelled to be readable.
        let relabel = if engine.is_podman() && cfg!(target_os = "linux") { ",z" } else { "" };
        for m in &self.mounts {
            let mode = if m.read_only { "ro" } else { "rw" };
            let mut v = OsString::from(&m.host);
            v.push(":");
            v.push(&m.container);
            v.push(format!(":{mode}{relabel}"));
            args.push("-v".into());
            args.push(v);
        }
        for v in &self.volumes {
            args.push("-v".into());
            args.push(v.into());
        }
        // Images built by rootless Podman keep a root-owned /run/user/<uid> from the build, where
        // Wine and others then fail to make their runtime directory: start from an empty one
        // that anyone can make theirs in, like /tmp.
        args.push("--tmpfs".into());
        args.push(if engine.is_podman() {
            "/run/user:rw,mode=1777,notmpcopyup".into()
        } else {
            "/run/user:rw,mode=1777".into()
        });
        let cargo_bin = self
            .env
            .iter()
            .find(|(k, _)| k == "CARGO_HOME")
            .map(|(_, v)| format!(":{v}/bin"))
            .unwrap_or_default();
        let path = format!(
            "{TOOLS_DIR}{cargo_bin}:{}",
            image_path.unwrap_or("/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
        );
        for (k, v) in self
            .env
            .iter()
            .chain(std::iter::once(&("PATH".to_string(), path)))
        {
            args.push("-e".into());
            args.push(format!("{k}={v}").into());
        }
        args.push("-w".into());
        args.push(self.workdir.clone().into());
        args
    }
}

#[allow(dead_code)]
fn identity(path: &Path) -> (PathBuf, u64, u64) {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map_or(0, |m| m.len());
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64);
    (path.to_path_buf(), size, mtime)
}

/// Host variables builds see in containers too, besides the configured `passthrough` ones.
const PASSTHROUGH: &[&str] = &[
    "RUST_BACKTRACE",
    "RUST_LOG",
    "RUST_MIN_STACK",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_INCREMENTAL",
    "CARGO_BUILD_JOBS",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_RELEASE_DEBUG",
    "CARGO_TERM_VERBOSE",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
];

/// The environment of one build command in a container: passed-through and configured variables,
/// colour, and the linker for Cargo.
pub fn command_env(
    target: &TargetInfo,
    config: &Config,
    builder: &Builder,
    hyprlnk: bool,
    terminal: bool,
) -> Vec<(String, String)> {
    let mut env = Vec::new();
    let tc = config.get_target(&target.triple);
    let names = PASSTHROUGH
        .iter()
        .map(|s| s.to_string())
        .chain(config.build.env.passthrough.iter().cloned())
        .chain(tc.map(|t| t.env.passthrough.clone()).unwrap_or_default());
    for name in names {
        if let Ok(v) = std::env::var(&name) {
            env.push((name, v));
        }
    }
    // Rune's own settings (`RUNE_BUILD_JOBS`…), but not where its cache is: that's mounted.
    for (k, v) in std::env::vars() {
        if k.starts_with("RUNE_") && k != "RUNE_HOME" {
            env.push((k, v));
        }
    }
    if ui::colors() && !terminal {
        env.push(("CLICOLOR_FORCE".into(), "1".into()));
        env.push(("CARGO_TERM_COLOR".into(), "always".into()));
    }
    if builder.kind == BuilderKind::Cargo {
        #[cfg(target_os = "linux")]
        if hyprlnk && target.hyprlnk_supported {
            let key = crate::direct::target_env_key(&target.triple);
            env.push((
                format!("CARGO_TARGET_{key}_LINKER"),
                format!("{TOOLS_DIR}/hyprlnk"),
            ));
            if let Some(cc) = target.target_c_compiler() {
                env.push((crate::hyprlnk_driver::CC_ENV.into(), cc.into()));
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = hyprlnk;
            if let Some(cc) = target.target_c_compiler() {
                let key = crate::direct::target_env_key(&target.triple);
                env.push((format!("CARGO_TARGET_{key}_LINKER"), cc.into()));
            }
        }
    }
    for (k, v) in config
        .build
        .env
        .iter()
        .chain(tc.map(|t| t.env.iter()).into_iter().flatten())
    {
        env.push((k.clone(), v.clone()));
    }
    env
}

/// FNV-1a over byte strings: a hash that stays the same across Rust releases, for names and
/// fingerprints stored outside this process.
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.iter().chain(&[0xff]) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// A one-off container for one command (`--no-daemon`): `run --rm`.
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

    pub fn execute(&self, program: &str, args: &[String], stdin: bool) -> Result<i32> {
        let cwd = std::env::current_dir()?;
        let root = crate::workspace::project_root(&cwd);
        let request = ImageRequest::for_target(self.target, self.config, &cwd)?;
        let (image, image_env) = request.resolve(self.engine)?;
        let spec = ContainerSpec::new(self.target, self.config, self.builder, &root, &cwd, None)?;
        if self
            .config
            .get_target(&self.target.triple)
            .is_some_and(|t| !t.pre_build.is_empty())
        {
            ui::warn(
                "pre_build runs when a warm container is created: it is skipped with --no-daemon",
            );
        }

        let terminal = crate::engine::on_terminal();
        let mut cmd = self.engine.command();
        cmd.args(["run", "--rm", "--init"]);
        if stdin {
            cmd.arg("-i");
        }
        if terminal && stdin_is_terminal() {
            cmd.arg("-t");
        }
        let image_path = image_env
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.as_str());
        cmd.args(spec.run_args(self.engine, image_path));
        cmd.arg("-w").arg(&spec.workdir);
        for (k, v) in command_env(
            self.target,
            self.config,
            self.builder,
            self.use_hyprlnk,
            terminal,
        ) {
            cmd.arg("-e").arg(format!("{k}={v}"));
        }
        cmd.arg("--entrypoint").arg(program).arg(&image).args(args);
        ui::trace(format!("{cmd:?}"));
        Ok(cmd.status()?.code().unwrap_or(1))
    }
}

pub fn stdin_is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(&[b"hyprcross"]), fnv1a(&[b"hyprcross"]));
        assert_ne!(
            fnv1a(&[b"ab", b"c"]),
            fnv1a(&[b"a", b"bc"]),
            "parts are delimited"
        );
    }

    #[test]
    fn mounts_keep_host_paths_and_tools_are_read_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"p\"\n").unwrap();
        let target = TargetInfo::parse("aarch64-unknown-linux-gnu");
        let builder = Builder {
            kind: BuilderKind::Cargo,
            path: "/usr/bin/cargo".into(),
        };
        let spec = ContainerSpec::new(
            &target,
            &Config::default(),
            &builder,
            dir.path(),
            dir.path(),
            Some(Path::new("/run/user/1/hyprcross/x")),
        )
        .unwrap();
        #[cfg(target_os = "linux")]
        {
            assert!(spec.mounts.contains(&Mount {
                host: dir.path().into(),
                container: dir.path().into(),
                read_only: false,
            }));
            let hyprlnk = spec
                .mounts
                .iter()
                .find(|m| m.container == Path::new(TOOLS_DIR).join("hyprlnk"))
                .unwrap();
            assert!(hyprlnk.read_only);
            assert!(
                spec.mounts
                    .iter()
                    .any(|m| m.container == Path::new(RUN_DIR))
            );
        }
        #[cfg(windows)]
        {
            assert!(spec.mounts.contains(&Mount {
                host: dir.path().into(),
                container: PathBuf::from("/project"),
                read_only: false,
            }));
            assert_eq!(spec.workdir, Path::new("/project"));
        }

        let engine = ContainerEngine {
            kind: ContainerEngineKind::Docker,
            path: "docker".into(),
        };
        let args: Vec<String> = spec
            .run_args(&engine, Some("/usr/bin:/bin"))
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let path = args.iter().find(|a| a.starts_with("PATH=")).unwrap();
        assert!(path.starts_with(&format!("PATH={TOOLS_DIR}")), "{path}");
        assert!(
            path.ends_with(":/usr/bin:/bin"),
            "the image's PATH is kept: {path}"
        );
    }

    #[test]
    fn cargo_links_with_hyprlnk_in_containers_and_rune_with_its_own() {
        let target = TargetInfo::parse("aarch64-unknown-linux-gnu");
        let config = Config::default();
        let cargo = Builder {
            kind: BuilderKind::Cargo,
            path: "cargo".into(),
        };
        let env = command_env(&target, &config, &cargo, true, false);
        #[cfg(target_os = "linux")]
        {
            assert!(env.contains(&(
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER".into(),
                format!("{TOOLS_DIR}/hyprlnk")
            )));
            assert!(env.contains(&("HYPRLNK_CC".into(), "aarch64-linux-gnu-gcc".into())));
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(env.contains(&(
                "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER".into(),
                "aarch64-linux-gnu-gcc".into()
            )));
        }

        let rune = Builder {
            kind: BuilderKind::Rune,
            path: "rune".into(),
        };
        let env = command_env(&target, &config, &rune, true, false);
        assert!(
            !env.iter()
                .any(|(k, _)| k.ends_with("_LINKER") || k == "HYPRLNK_CC")
        );
    }

    #[test]
    fn test_to_container_path() {
        #[cfg(windows)]
        {
            let root = Path::new(r"C:\workspace\project");
            assert_eq!(to_container_path(root, root), Path::new("/project"));
            assert_eq!(
                to_container_path(&root.join("src").join("lib"), root),
                Path::new("/project/src/lib")
            );
        }
        #[cfg(not(windows))]
        {
            let root = Path::new("/workspace/project");
            assert_eq!(to_container_path(root, root), root);
        }
    }
}
