//! Who runs hyprcross: its own executable, or a program it is built into (Igel, as `igl cross`).
//! Messages name the command users type there, and the host can bring the builder (itself), its
//! settings, and where those are written.

use std::sync::OnceLock;

use anyhow::Result;

use crate::builder::Builder;
use crate::config::Config;

/// Applies a host's settings to the configuration hyprcross read.
pub type Configure = Box<dyn Fn(&mut Config) + Send + Sync>;

/// Writes the host's configuration for `init`, with the default target if given.
pub type Init = fn(Option<&str>) -> Result<()>;

pub struct Host {
    /// The command users type: `hyprcross`, or `igl cross`.
    pub command: &'static str,
    /// Where the default target is set, for hints.
    pub default_target_setting: &'static str,
    /// Where per-target settings go, for hints.
    pub target_settings: &'static str,
    /// The builder, when the host is one: it runs itself.
    pub builder: Option<Builder>,
    /// Whether the host links with a hyprlnk of its own (Igel does), for `doctor`.
    pub links_with_hyprlnk: bool,
    /// The host's settings, applied over the ones hyprcross reads from its files.
    pub configure: Option<Configure>,
    /// `init`: the host writes its own configuration, for a default target if one is given.
    pub init: Option<Init>,
}

impl Host {
    pub fn standalone() -> Self {
        Host {
            command: "hyprcross",
            default_target_setting: "`default_target` in Hyprcross.toml",
            target_settings: "Hyprcross.toml",
            builder: None,
            links_with_hyprlnk: false,
            configure: None,
            init: None,
        }
    }
}

static HOST: OnceLock<Host> = OnceLock::new();

/// Install the host before anything runs; the first one installed stays.
pub fn set(host: Host) {
    let _ = HOST.set(host);
}

pub fn get() -> &'static Host {
    HOST.get_or_init(Host::standalone)
}

/// The command users type: `hyprcross`, or `igl cross`.
pub fn command() -> &'static str {
    get().command
}

/// `` `<command> <rest>` ``, for hints.
pub fn invocation(rest: &str) -> String {
    format!("`{} {rest}`", command())
}
