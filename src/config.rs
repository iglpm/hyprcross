use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub target: HashMap<String, TargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Engine to use: "auto", "direct", or "container"
    #[serde(default = "default_engine")]
    pub engine: String,

    /// Linker to accelerate builds with: "hyprlnk" or "default"
    #[serde(default = "default_linker")]
    pub linker: String,

    /// Underlying build orchestrator: "auto", "rune", or "cargo"
    #[serde(default = "default_builder")]
    pub builder: String,

    /// Keep container warm in background for near-zero startup latency
    #[serde(default = "default_true")]
    pub daemon: bool,

    /// Container daemon idle timeout in minutes before auto-stopping
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_mins: u32,

    /// Container runtime to invoke: "docker", "podman", "finch", "nerdctl", or "auto"
    #[serde(default = "default_container_engine")]
    pub container_engine: String,

    /// Default target triple when --target is omitted
    #[serde(default)]
    pub default_target: Option<String>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            linker: default_linker(),
            builder: default_builder(),
            daemon: default_true(),
            idle_timeout_mins: default_idle_timeout(),
            container_engine: default_container_engine(),
            default_target: None,
        }
    }
}

fn default_engine() -> String {
    "auto".to_string()
}

fn default_linker() -> String {
    "hyprlnk".to_string()
}

fn default_builder() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

fn default_idle_timeout() -> u32 {
    15
}

fn default_container_engine() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    /// Container image name or tag
    pub image: Option<String>,

    /// Path to a custom Dockerfile or Containerfile
    pub dockerfile: Option<PathBuf>,

    /// Target sysroot path (for direct mode or QEMU dynamic linker)
    pub sysroot: Option<PathBuf>,

    /// Target runner executable (e.g. custom emulator or wrapper)
    pub runner: Option<String>,

    /// Environment variables injected during build/test
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Additional volume mounts (format: "host_path:container_path[:opts]")
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Commands to execute inside the container before the build
    #[serde(default)]
    pub pre_build: Vec<String>,

    /// Per-target linker override ("hyprlnk", "lld", etc.)
    pub linker: Option<String>,

    /// Per-target engine override ("direct", "container")
    pub engine: Option<String>,
}

impl Config {
    /// Discover and load configuration from the current directory, parent directories, or global path.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        // 1. Check Hyprcross.toml in dir
        let hyprcross_path = dir.join("Hyprcross.toml");
        if hyprcross_path.is_file() {
            let content = fs::read_to_string(&hyprcross_path)?;
            let cfg: Config = toml::from_str(&content)?;
            return Ok(cfg);
        }

        // 2. Check Cross.toml (drop-in compatibility)
        let cross_path = dir.join("Cross.toml");
        if cross_path.is_file() {
            let content = fs::read_to_string(&cross_path)?;
            let cfg: Config = toml::from_str(&content)?;
            return Ok(cfg);
        }

        // 3. Check Cargo.toml [package.metadata.hyprcross] or [package.metadata.cross]
        let cargo_path = dir.join("Cargo.toml");
        if cargo_path.is_file()
            && let Ok(content) = fs::read_to_string(&cargo_path)
                && let Ok(cargo_toml) = toml::from_str::<toml::Value>(&content)
                    && let Some(metadata) = cargo_toml.get("package").and_then(|p| p.get("metadata")) {
                        if let Some(hc) = metadata.get("hyprcross") {
                            if let Ok(cfg) = toml::from_str::<Config>(&hc.to_string()) {
                                return Ok(cfg);
                            }
                        } else if let Some(cross) = metadata.get("cross")
                            && let Ok(cfg) = toml::from_str::<Config>(&cross.to_string()) {
                                return Ok(cfg);
                            }
                    }

        // 4. Check global config ~/.config/hyprcross/config.toml
        if let Some(home) = dirs_home() {
            let global_path = home.join(".config").join("hyprcross").join("config.toml");
            if global_path.is_file()
                && let Ok(content) = fs::read_to_string(&global_path)
                    && let Ok(cfg) = toml::from_str::<Config>(&content) {
                        return Ok(cfg);
                    }
        }

        Ok(Config::default())
    }

    /// Retrieve effective configuration for a target triple
    pub fn get_target(&self, triple: &str) -> Option<&TargetConfig> {
        self.target.get(triple)
    }
}

pub fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
