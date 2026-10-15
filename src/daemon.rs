//! Warm containers: one per project, target and builder, kept running between builds.
//!
//! On Linux the container's main process is the hyprcross agent ([`crate::agent`]); a build
//! connects to its socket and runs without asking the container engine anything. Elsewhere, or
//! when the agent can't run in the image, builds go through `exec`.
//!
//! What the container was made from (image, mounts, executables) is hashed into a spec, stored
//! as a label and in a state file next to the socket. A build whose spec matches the state file
//! connects straight away; otherwise the container is started, or replaced.

use anyhow::{Context, Result, bail};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::builder::Builder;
use crate::config::Config;
use crate::container::{ContainerEngine, ContainerSpec, ImageRequest, RUN_DIR, TOOLS_DIR, fnv1a};
use crate::targets::TargetInfo;
use crate::ui;

/// Part of the spec: a new agent protocol needs new containers.
#[cfg(unix)]
const AGENT_PROTOCOL: u32 = crate::agent::PROTOCOL;
#[cfg(not(unix))]
const AGENT_PROTOCOL: u32 = 0;

const LABEL_SPEC: &str = "dev.hyprcross.spec";
const LABEL_MODE: &str = "dev.hyprcross.mode";
const LABEL_TARGET: &str = "dev.hyprcross.target";
const LABEL_PROJECT: &str = "dev.hyprcross.project";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Builds go to the agent's socket.
    Agent,
    /// Builds go through `<engine> exec`.
    Exec,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Agent => "agent",
            Mode::Exec => "exec",
        }
    }

    fn parse(s: &str) -> Option<Mode> {
        match s.trim() {
            "agent" => Some(Mode::Agent),
            "exec" => Some(Mode::Exec),
            _ => None,
        }
    }
}

/// One command to run in the container.
pub struct Job<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub env: Vec<(String, String)>,
    pub cwd: &'a Path,
    pub stdin: bool,
}

pub struct ContainerDaemon<'a> {
    pub engine: &'a ContainerEngine,
    pub target: &'a TargetInfo,
    pub config: &'a Config,
    pub builder: &'a Builder,
    root: PathBuf,
    cwd: PathBuf,
}

impl<'a> ContainerDaemon<'a> {
    pub fn new(
        engine: &'a ContainerEngine,
        target: &'a TargetInfo,
        config: &'a Config,
        builder: &'a Builder,
    ) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let root = crate::workspace::project_root(&cwd);
        Ok(Self {
            engine,
            target,
            config,
            builder,
            root,
            cwd,
        })
    }

    /// `hyprcross-<project>-<target>-<builder>-<hash of the project's path>`.
    pub fn container_name(&self) -> String {
        let project: String = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into())
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let hash = fnv1a(&[self.root.as_os_str().as_encoded_bytes()]);
        format!(
            "hyprcross-{project}-{}-{}-{:06x}",
            self.target.triple.replace('.', "-"),
            self.builder.kind.name(),
            hash & 0xff_ffff
        )
    }

    /// The host directory with the agent's socket and the state file; short, because socket
    /// paths are limited to about 100 bytes.
    fn run_dir(&self) -> Option<PathBuf> {
        Some(run_base()?.join(self.id()))
    }

    fn id(&self) -> String {
        format!(
            "{:012x}",
            fnv1a(&[self.container_name().as_bytes()]) & 0xffff_ffff_ffff
        )
    }

    /// Held while the container is checked, started or replaced: two builds starting at once
    /// would otherwise both create it. Next to the run directory, which replacing removes.
    fn lock(&self) -> Option<std::fs::File> {
        let base = run_base()?;
        std::fs::create_dir_all(&base).ok()?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(base.join(format!("{}.lock", self.id())))
            .ok()?;
        file.lock().ok()?;
        Some(file)
    }

    fn agent_supported(&self) -> bool {
        // Unix sockets don't cross the VM boundary of Docker Desktop and friends.
        cfg!(target_os = "linux")
    }

    fn spec(
        &self,
        request: &ImageRequest,
        run_dir: Option<&Path>,
    ) -> Result<(ContainerSpec, String)> {
        let spec = ContainerSpec::new(
            self.target,
            self.config,
            self.builder,
            &self.root,
            &self.cwd,
            run_dir,
        )?;
        let tc = self.config.get_target(&self.target.triple);
        let fingerprint = format!(
            "{:?}|{:?}|{:?}|{}|{}|{}",
            spec,
            request.key(),
            tc.map(|t| &t.pre_build),
            self.config.build.idle_timeout_mins,
            AGENT_PROTOCOL,
            crate::VERSION,
        );
        Ok((spec, format!("{:016x}", fnv1a(&[fingerprint.as_bytes()]))))
    }

    /// Run `job` in the warm container, starting or replacing the container first if needed.
    pub fn execute(&self, job: Job<'_>) -> Result<i32> {
        let request = ImageRequest::for_target(self.target, self.config, &self.cwd)?;
        let run_dir = self.agent_supported().then(|| self.run_dir()).flatten();
        let (spec, hash) = self.spec(&request, run_dir.as_deref())?;

        // The fast path: the container this state file describes is (most likely) running. An
        // agent that stopped for being idle just as we connected answers nothing: then start
        // it again below.
        #[cfg(unix)]
        if let Some(dir) = &run_dir
            && read_state(dir).is_some_and(|(s, m)| s == hash && m == Mode::Agent)
            && let Ok(stream) = crate::agent::connect(&dir.join("agent.sock"))
        {
            match self.call(stream, &job) {
                Err(e) if e.kind() == std::io::ErrorKind::NotConnected => {}
                result => return Ok(result?),
            }
        }

        let mode = {
            let _lock = self.lock();
            self.ensure_running(&request, &spec, &hash, run_dir.as_deref())?
        };
        #[cfg(unix)]
        if let (Mode::Agent, Some(dir)) = (mode, &run_dir) {
            let stream = crate::agent::connect(&dir.join("agent.sock"))
                .context("the container's agent stopped answering")?;
            return Ok(self.call(stream, &job)?);
        }
        let _ = mode;
        self.exec(&job)
    }

    #[cfg(unix)]
    fn call(&self, stream: std::os::unix::net::UnixStream, job: &Job<'_>) -> std::io::Result<i32> {
        let pty = crate::engine::on_terminal().then(crate::engine::terminal_size);
        let req = crate::agent::Request {
            protocol: crate::agent::PROTOCOL,
            program: job.program.to_string(),
            args: job.args.to_vec(),
            env: job.env.clone(),
            cwd: job.cwd.display().to_string(),
            pty,
            stdin: job.stdin,
        };
        ui::trace(format!("agent: {} {}", req.program, req.args.join(" ")));
        crate::agent::call(stream, &req)
    }

    fn exec(&self, job: &Job<'_>) -> Result<i32> {
        let mut cmd = self.engine.command();
        cmd.arg("exec");
        if job.stdin {
            cmd.arg("-i");
        }
        if crate::engine::on_terminal() && crate::container::stdin_is_terminal() {
            cmd.arg("-t");
        }
        cmd.arg("-w").arg(job.cwd);
        for (k, v) in &job.env {
            cmd.arg("-e").arg(format!("{k}={v}"));
        }
        cmd.arg(self.container_name())
            .arg(job.program)
            .args(job.args);
        if !job.stdin {
            cmd.stdin(Stdio::null());
        }
        ui::trace(format!("{cmd:?}"));
        Ok(cmd.status()?.code().unwrap_or(1))
    }

    /// The container's state: (running, spec label, mode label), or `None` if there is none.
    fn inspect(&self) -> Option<(bool, String, Option<Mode>)> {
        let format = format!(
            "{{{{.State.Running}}}}|{{{{index .Config.Labels \"{LABEL_SPEC}\"}}}}|{{{{index .Config.Labels \"{LABEL_MODE}\"}}}}"
        );
        let out = self
            .engine
            .command()
            .args([
                "container",
                "inspect",
                "--format",
                &format,
                &self.container_name(),
            ])
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut parts = text.trim().splitn(3, '|');
        let running = parts.next() == Some("true");
        let spec = parts.next().unwrap_or("").to_string();
        let mode = parts.next().and_then(Mode::parse);
        Some((running, spec, mode))
    }

    fn ensure_running(
        &self,
        request: &ImageRequest,
        spec: &ContainerSpec,
        hash: &str,
        run_dir: Option<&Path>,
    ) -> Result<Mode> {
        let name = self.container_name();
        match self.inspect() {
            Some((running, label, Some(mode))) if label == hash => {
                if !running || (mode == Mode::Agent && !self.agent_answers(run_dir)) {
                    let activity = ui::Activity::start("starting", format!("container {name}"));
                    let verb = if running { "restart" } else { "start" };
                    let mut cmd = self.engine.command();
                    cmd.arg(verb);
                    if running {
                        cmd.args(["-t", "0"]);
                    }
                    let out = cmd.arg(&name).stdin(Stdio::null()).output()?;
                    if !out.status.success() {
                        activity.clear();
                        self.remove();
                        return self.create(request, spec, hash, run_dir);
                    }
                    if mode == Mode::Agent && !self.wait_for_agent(run_dir) {
                        activity.clear();
                        self.remove();
                        return self.create(request, spec, hash, run_dir);
                    }
                    activity.finish("started", format!("container {name}"));
                }
                if let Some(dir) = run_dir {
                    write_state(dir, hash, mode);
                }
                Ok(mode)
            }
            Some(_) => {
                ui::action(
                    "replacing",
                    format!("container {name} {}", ui::dim("(its setup changed)")),
                );
                self.remove();
                self.create(request, spec, hash, run_dir)
            }
            None => self.create(request, spec, hash, run_dir),
        }
    }

    fn agent_answers(&self, run_dir: Option<&Path>) -> bool {
        #[cfg(unix)]
        {
            run_dir.is_some_and(|d| crate::agent::connect(&d.join("agent.sock")).is_ok())
        }
        #[cfg(not(unix))]
        {
            let _ = run_dir;
            false
        }
    }

    fn wait_for_agent(&self, run_dir: Option<&Path>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.agent_answers(run_dir) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn create(
        &self,
        request: &ImageRequest,
        spec: &ContainerSpec,
        hash: &str,
        run_dir: Option<&Path>,
    ) -> Result<Mode> {
        let (image, image_env) = request.resolve(self.engine)?;
        let image_path = image_env
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.clone());
        let name = self.container_name();

        let mut mode = if run_dir.is_some() {
            Mode::Agent
        } else {
            Mode::Exec
        };
        loop {
            if let Some(dir) = run_dir {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
                let _ = std::fs::remove_file(dir.join("agent.sock"));
                let _ = std::fs::remove_file(dir.join("state"));
            }
            let activity = ui::Activity::start("starting", &image);
            let mut cmd = self.engine.command();
            cmd.args(["run", "-d", "--init", "--name", &name]);
            for (k, v) in [
                (LABEL_SPEC, hash),
                (LABEL_MODE, mode.name()),
                (LABEL_TARGET, self.target.triple.as_str()),
                (LABEL_PROJECT, &self.root.display().to_string()),
            ] {
                cmd.arg("--label").arg(format!("{k}={v}"));
            }
            cmd.args(spec.run_args(self.engine, image_path.as_deref()));
            match mode {
                Mode::Agent => {
                    let idle = self.config.build.idle_timeout_mins.to_string();
                    cmd.arg("--entrypoint")
                        .arg(format!("{TOOLS_DIR}/hyprcross"));
                    cmd.arg(&image);
                    cmd.args([
                        "__agent",
                        &format!("{RUN_DIR}/agent.sock"),
                        "--idle-mins",
                        &idle,
                    ]);
                }
                Mode::Exec => {
                    cmd.args(["--entrypoint", "sleep", &image, "infinity"]);
                }
            }
            ui::trace(format!("{cmd:?}"));
            let out = cmd.stdin(Stdio::null()).output()?;
            if !out.status.success() {
                activity.clear();
                let msg = String::from_utf8_lossy(&out.stderr);
                bail!(
                    "cannot start a container from {image}: {}",
                    msg.lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("unknown error")
                        .trim()
                );
            }
            if mode == Mode::Agent && !self.wait_for_agent(run_dir) {
                activity.clear();
                let logs = self
                    .engine
                    .command()
                    .args(["logs", "--tail", "5", &name])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
                    .unwrap_or_default();
                ui::warn(format!("hyprcross' agent did not start in {image}"));
                if !logs.is_empty() {
                    ui::hint(logs);
                }
                ui::hint("builds will use `exec` in this container, which is slower");
                self.remove();
                mode = Mode::Exec;
                continue;
            }
            activity.finish("started", format!("container {name}"));
            break;
        }

        self.pre_build(&name)?;
        if let Some(dir) = run_dir {
            write_state(dir, hash, mode);
        }
        Ok(mode)
    }

    /// `pre_build` commands, as root, once per container.
    fn pre_build(&self, name: &str) -> Result<()> {
        let Some(tc) = self.config.get_target(&self.target.triple) else {
            return Ok(());
        };
        for command in &tc.pre_build {
            let activity = ui::Activity::start("preparing", command);
            let out = self
                .engine
                .command()
                .args(["exec", "--user", "0", "-w", "/", name, "sh", "-c", command])
                .stdin(Stdio::null())
                .output()?;
            if !out.status.success() {
                activity.clear();
                self.remove();
                let log = String::from_utf8_lossy(&out.stderr);
                let tail: Vec<&str> = log.lines().rev().take(10).collect();
                bail!(
                    "pre_build command failed: {command}\n{}",
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                );
            }
            activity.finish("prepared", command);
        }
        Ok(())
    }

    fn remove(&self) {
        let _ = self
            .engine
            .command()
            .args(["rm", "-f", "-t", "0", &self.container_name()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Some(dir) = self.run_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Start the container without running anything in it.
    pub fn warm(&self) -> Result<()> {
        let request = ImageRequest::for_target(self.target, self.config, &self.cwd)?;
        let run_dir = self.agent_supported().then(|| self.run_dir()).flatten();
        let (spec, hash) = self.spec(&request, run_dir.as_deref())?;
        let _lock = self.lock();
        self.ensure_running(&request, &spec, &hash, run_dir.as_deref())?;
        Ok(())
    }

    pub fn stop(&self) -> bool {
        let existed = self.inspect().is_some();
        self.remove();
        existed
    }
}

/// `$XDG_RUNTIME_DIR/hyprcross`, else `~/.cache/hyprcross/run` (CI runners set the variable
/// without creating the directory).
fn run_base() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
        .map(|d| d.join("hyprcross"))
        .or_else(|| crate::config::cache_dir().map(|d| d.join("run")))
}

#[cfg_attr(not(unix), allow(dead_code))]
fn read_state(dir: &Path) -> Option<(String, Mode)> {
    let text = std::fs::read_to_string(dir.join("state")).ok()?;
    let (spec, mode) = text.trim().split_once(' ')?;
    Some((spec.to_string(), Mode::parse(mode)?))
}

fn write_state(dir: &Path, hash: &str, mode: Mode) {
    let _ = std::fs::write(dir.join("state"), format!("{hash} {}\n", mode.name()));
}

/// A warm container, as `daemon status` lists it.
pub struct Listed {
    pub name: String,
    pub state: String,
    pub image: String,
    pub target: String,
    pub project: String,
}

/// Every hyprcross container, including ones from before containers had labels.
pub fn list(engine: &ContainerEngine) -> Result<Vec<Listed>> {
    let ids = engine
        .command()
        .args(["ps", "-a", "-q", "--filter", "name=hyprcross-"])
        .stderr(Stdio::null())
        .output()
        .context("cannot list containers")?;
    let ids: Vec<String> = String::from_utf8_lossy(&ids.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let format = format!(
        "{{{{.Name}}}}|{{{{.State.Status}}}}|{{{{.Config.Image}}}}|{{{{index .Config.Labels \"{LABEL_TARGET}\"}}}}|{{{{index .Config.Labels \"{LABEL_PROJECT}\"}}}}"
    );
    let out = engine
        .command()
        .args(["container", "inspect", "--format", &format])
        .args(&ids)
        .stderr(Stdio::null())
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut p = l.split('|');
            Some(Listed {
                name: p.next()?.trim_start_matches('/').to_string(),
                state: p.next()?.to_string(),
                image: p.next()?.to_string(),
                target: p.next().unwrap_or("").replace("<no value>", ""),
                project: p.next().unwrap_or("").replace("<no value>", ""),
            })
        })
        .collect())
}

/// Remove containers by name; how many went.
pub fn remove_all(engine: &ContainerEngine, names: &[String]) -> usize {
    if names.is_empty() {
        return 0;
    }
    let ok = engine
        .command()
        .args(["rm", "-f", "-t", "0"])
        .args(names)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if ok { names.len() } else { 0 }
}

/// The runtime directories of containers that no longer exist (or all of them).
pub fn clean_run_dirs() -> io::Result<()> {
    if let Some(base) = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join("hyprcross"))
    {
        let _ = std::fs::remove_dir_all(base);
    }
    if let Some(base) = crate::config::cache_dir().map(|d| d.join("run")) {
        let _ = std::fs::remove_dir_all(base);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_files_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_state(dir.path()).is_none());
        write_state(dir.path(), "00ff", Mode::Agent);
        assert_eq!(
            read_state(dir.path()),
            Some(("00ff".to_string(), Mode::Agent))
        );
        write_state(dir.path(), "00ff", Mode::Exec);
        assert_eq!(read_state(dir.path()).unwrap().1, Mode::Exec);
    }
}
