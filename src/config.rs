//! `Hyprcross.toml`, and cross-rs' `Cross.toml` and `[package.metadata.cross]` for projects that
//! come from cross: the same keys, in either `snake_case` or cross' `kebab-case`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::targets::{DEFAULT_IMAGE_REPOSITORY, DEFAULT_IMAGE_TAG};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub target: HashMap<String, TargetConfig>,
    /// The file this came from; `None` for the defaults.
    #[serde(skip)]
    pub source: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// "auto", "direct" or "container".
    #[serde(default = "default_engine")]
    pub engine: String,

    /// "hyprlnk" or "default".
    #[serde(default = "default_linker")]
    pub linker: String,

    /// "auto", "rune" or "cargo".
    #[serde(default = "default_builder")]
    pub builder: String,

    /// Keep a warm container per project and target, and run builds in it.
    #[serde(default = "default_true")]
    pub daemon: bool,

    /// Minutes a warm container waits for the next build before it stops.
    #[serde(default = "default_idle_timeout", alias = "idle-timeout-mins")]
    pub idle_timeout_mins: u32,

    /// "auto", "podman", "docker", "finch" or "nerdctl".
    #[serde(default = "default_container_engine", alias = "container-engine")]
    pub container_engine: String,

    #[serde(default, alias = "default-target")]
    pub default_target: Option<String>,

    /// Where the per-target images live: `<image_repository>/<target>:<image_tag>`.
    #[serde(default = "default_image_repository", alias = "image-repository")]
    pub image_repository: String,

    #[serde(default = "default_image_tag", alias = "image-tag")]
    pub image_tag: String,

    /// Host environment variables passed into containers (`[build.env] passthrough` in Cross.toml).
    #[serde(default)]
    pub env: EnvTable,
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
            image_repository: default_image_repository(),
            image_tag: default_image_tag(),
            env: EnvTable::default(),
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

fn default_image_repository() -> String {
    DEFAULT_IMAGE_REPOSITORY.to_string()
}

fn default_image_tag() -> String {
    DEFAULT_IMAGE_TAG.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    /// The container image to build in, instead of the target's default.
    pub image: Option<String>,

    /// A Dockerfile to build the image from (its directory is the build context).
    pub dockerfile: Option<PathBuf>,

    /// The target's sysroot, for QEMU's dynamic loader in direct mode.
    pub sysroot: Option<PathBuf>,

    /// What runs the target's programs in direct mode, instead of QEMU.
    pub runner: Option<String>,

    /// Variables set for the build: `KEY = "value"`, plus Cross.toml's `passthrough` and
    /// `volumes` lists.
    #[serde(default)]
    pub env: EnvTable,

    /// Extra mounts, `host_path:container_path[:options]`.
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Shell commands run as root in the container when it is created (`apt-get install …`).
    #[serde(default, alias = "pre-build")]
    pub pre_build: Vec<String>,

    /// "hyprlnk" or "default", for this target only.
    pub linker: Option<String>,

    /// "direct" or "container", for this target only.
    pub engine: Option<String>,
}

/// A table of variables. Cross.toml puts two lists in the same table: `passthrough` (names of
/// host variables to pass on) and `volumes` (names of variables holding host paths to mount).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(from = "HashMap<String, EnvValue>", into = "HashMap<String, EnvValue>")]
pub struct EnvTable {
    pub vars: HashMap<String, String>,
    pub passthrough: Vec<String>,
    pub volumes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    One(String),
    Many(Vec<String>),
}

impl From<HashMap<String, EnvValue>> for EnvTable {
    fn from(map: HashMap<String, EnvValue>) -> Self {
        let mut table = EnvTable::default();
        for (key, value) in map {
            match (key.as_str(), value) {
                ("passthrough", EnvValue::Many(names)) => table.passthrough = names,
                ("volumes", EnvValue::Many(names)) => table.volumes = names,
                (_, EnvValue::One(v)) => {
                    table.vars.insert(key, v);
                }
                (_, EnvValue::Many(v)) => {
                    table.vars.insert(key, v.join(" "));
                }
            }
        }
        table
    }
}

impl From<EnvTable> for HashMap<String, EnvValue> {
    fn from(table: EnvTable) -> Self {
        let mut map: HashMap<String, EnvValue> = table
            .vars
            .into_iter()
            .map(|(k, v)| (k, EnvValue::One(v)))
            .collect();
        if !table.passthrough.is_empty() {
            map.insert("passthrough".into(), EnvValue::Many(table.passthrough));
        }
        if !table.volumes.is_empty() {
            map.insert("volumes".into(), EnvValue::Many(table.volumes));
        }
        map
    }
}

impl EnvTable {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.vars.iter()
    }
}

impl Config {
    /// The first configuration found in `dir` or a directory above it: `Hyprcross.toml`,
    /// `Cross.toml`, or a Cargo.toml's `[package.metadata.hyprcross]` /
    /// `[workspace.metadata.hyprcross]` (or `.cross`); then `~/.config/hyprcross/config.toml`.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut cfg = Self::load_file_from_dir(dir)?;
        // The host's settings (Rune's `cross.*`) over the files, under HYPRCROSS_* variables.
        if let Some(configure) = &crate::host::get().configure {
            configure(&mut cfg);
        }
        cfg.apply_env();
        Ok(cfg)
    }

    /// `HYPRCROSS_CONTAINER_ENGINE`, `HYPRCROSS_IMAGE_REPOSITORY` and `HYPRCROSS_IMAGE_TAG` win
    /// over the files: CI points builds at images it just built this way.
    fn apply_env(&mut self) {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        if let Some(v) = var("HYPRCROSS_CONTAINER_ENGINE") {
            self.build.container_engine = v;
        }
        if let Some(v) = var("HYPRCROSS_IMAGE_REPOSITORY") {
            self.build.image_repository = v;
        }
        if let Some(v) = var("HYPRCROSS_IMAGE_TAG") {
            self.build.image_tag = v;
        }
    }

    fn load_file_from_dir(dir: &Path) -> Result<Self> {
        for ancestor in dir.ancestors() {
            for name in ["Hyprcross.toml", "Cross.toml"] {
                let path = ancestor.join(name);
                if path.is_file() {
                    return Self::load_file(&path);
                }
            }
            if let Some(cfg) = Self::from_manifest(&ancestor.join("Cargo.toml"))? {
                return Ok(cfg);
            }
        }

        if let Some(home) = dirs_home() {
            let global = home.join(".config").join("hyprcross").join("config.toml");
            if global.is_file() {
                return Self::load_file(&global);
            }
        }

        Ok(Config::default())
    }

    fn from_manifest(path: &Path) -> Result<Option<Self>> {
        let Ok(content) = fs::read_to_string(path) else {
            return Ok(None);
        };
        // A manifest that doesn't parse is Cargo's to report, not ours.
        let Ok(manifest) = toml::from_str::<toml::Value>(&content) else {
            return Ok(None);
        };
        for section in ["package", "workspace"] {
            let Some(metadata) = manifest.get(section).and_then(|p| p.get("metadata")) else {
                continue;
            };
            for key in ["hyprcross", "cross"] {
                if let Some(table) = metadata.get(key) {
                    let mut cfg = Config::deserialize(table.clone()).with_context(|| {
                        format!("invalid [{section}.metadata.{key}] in {}", path.display())
                    })?;
                    cfg.source = Some(path.to_path_buf());
                    return Ok(Some(cfg));
                }
            }
        }
        Ok(None)
    }

    fn load_file(path: &Path) -> Result<Self> {
        let content =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&content).with_context(|| format!("invalid {}", path.display()))?;
        cfg.source = Some(path.to_path_buf());
        Ok(cfg)
    }

    pub fn get_target(&self, triple: &str) -> Option<&TargetConfig> {
        self.target.get(triple)
    }

    /// Settings for `triple` from elsewhere (Rune's `[cross.target.<triple>]`) over the ones
    /// read from the files, field by field.
    pub fn merge_target(&mut self, triple: &str, other: TargetConfig) {
        self.target
            .entry(triple.to_string())
            .or_default()
            .merge(other);
    }
}

impl TargetConfig {
    /// `other`'s settings over these: what it sets replaces, what it leaves out stays.
    pub fn merge(&mut self, other: TargetConfig) {
        let TargetConfig {
            image,
            dockerfile,
            sysroot,
            runner,
            env,
            volumes,
            pre_build,
            linker,
            engine,
        } = other;
        fn keep<T>(mine: &mut Option<T>, theirs: Option<T>) {
            if theirs.is_some() {
                *mine = theirs;
            }
        }
        keep(&mut self.image, image);
        keep(&mut self.dockerfile, dockerfile);
        keep(&mut self.sysroot, sysroot);
        keep(&mut self.runner, runner);
        keep(&mut self.linker, linker);
        keep(&mut self.engine, engine);
        if !volumes.is_empty() {
            self.volumes = volumes;
        }
        if !pre_build.is_empty() {
            self.pre_build = pre_build;
        }
        self.env.vars.extend(env.vars);
        for name in env.passthrough {
            if !self.env.passthrough.contains(&name) {
                self.env.passthrough.push(name);
            }
        }
        for name in env.volumes {
            if !self.env.volumes.contains(&name) {
                self.env.volumes.push(name);
            }
        }
    }
}

pub fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// `$XDG_CACHE_HOME/hyprcross`, else `~/.cache/hyprcross`.
pub fn cache_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join(".cache")))
        .map(|d| d.join("hyprcross"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_toml_parses_with_its_own_spelling() {
        let cfg: Config = toml::from_str(
            r#"
            [build]
            default-target = "aarch64-unknown-linux-gnu"

            [build.env]
            passthrough = ["RUST_LOG"]

            [target.aarch64-unknown-linux-gnu]
            image = "my/image:1"
            pre-build = ["apt-get install -y libssl-dev:arm64"]

            [target.aarch64-unknown-linux-gnu.env]
            passthrough = ["FOO"]
            volumes = ["DATA_DIR"]
            "#,
        )
        .unwrap();
        assert_eq!(
            cfg.build.default_target.as_deref(),
            Some("aarch64-unknown-linux-gnu")
        );
        assert_eq!(cfg.build.env.passthrough, ["RUST_LOG"]);
        let t = cfg.get_target("aarch64-unknown-linux-gnu").unwrap();
        assert_eq!(t.pre_build.len(), 1);
        assert_eq!(t.env.passthrough, ["FOO"]);
        assert_eq!(t.env.volumes, ["DATA_DIR"]);
        assert!(t.env.vars.is_empty());
    }

    #[test]
    fn hyprcross_toml_variables_and_defaults() {
        let cfg: Config = toml::from_str(
            r#"
            [target.x86_64-pc-windows-gnu]
            env = { FOO = "bar" }
            "#,
        )
        .unwrap();
        assert_eq!(cfg.build.image_repository, DEFAULT_IMAGE_REPOSITORY);
        assert_eq!(cfg.build.idle_timeout_mins, 15);
        assert_eq!(
            cfg.get_target("x86_64-pc-windows-gnu")
                .unwrap()
                .env
                .get("FOO")
                .map(String::as_str),
            Some("bar")
        );
    }

    #[test]
    fn configuration_is_found_in_a_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Hyprcross.toml"),
            "[build]\ndefault_target = \"s390x-unknown-linux-gnu\"\n",
        )
        .unwrap();
        let member = dir.path().join("crates").join("a");
        std::fs::create_dir_all(&member).unwrap();
        let cfg = Config::load_from_dir(&member).unwrap();
        assert_eq!(
            cfg.build.default_target.as_deref(),
            Some("s390x-unknown-linux-gnu")
        );
        assert_eq!(cfg.source.unwrap(), dir.path().join("Hyprcross.toml"));
    }
}
