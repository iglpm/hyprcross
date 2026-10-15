use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const BUILD_EXAMPLES: &[(&str, &str)] = &[
    (
        "hyprcross build -t aarch64-unknown-linux-gnu",
        "64-bit Arm Linux",
    ),
    (
        "hyprcross build -t x86_64-pc-windows-gnu --release",
        "Windows, optimized",
    ),
    (
        "hyprcross build -t riscv64gc-unknown-linux-gnu -p app",
        "One package",
    ),
    (
        "hyprcross build -t aarch64-unknown-linux-musl --engine container",
        "In the image",
    ),
];

const TEST_EXAMPLES: &[(&str, &str)] = &[
    ("hyprcross test -t aarch64-unknown-linux-gnu", "Under QEMU"),
    ("hyprcross test -t x86_64-pc-windows-gnu", "Under Wine"),
    ("hyprcross test -t wasm32-wasip1 --lib", "Under Wasmtime"),
];

#[derive(Parser, Debug)]
#[command(
    name = "hyprcross",
    bin_name = "hyprcross",
    version,
    about = "Cross-compile and cross-test Rust, with Rune or Cargo",
    styles = crate::help::styles(),
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub build_opts: SharedBuildOpts,

    /// When to use colours: auto, always or never
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto",
          value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// Only print errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Print the commands hyprcross runs
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile the package for a target
    #[command(visible_alias = "b", after_help = crate::help::examples(BUILD_EXAMPLES))]
    Build(CommonArgs),

    /// Type-check for a target, without generating code
    #[command(visible_alias = "c")]
    Check(CommonArgs),

    /// Build and run the tests, under QEMU, Wine or Wasmtime
    #[command(visible_alias = "t", after_help = crate::help::examples(TEST_EXAMPLES))]
    Test(CommonArgs),

    /// Build and run a binary on the target
    #[command(visible_alias = "r")]
    Run(CommonArgs),

    /// Build and run the benchmarks on the target
    Bench(CommonArgs),

    /// Lint with clippy for a target
    Clippy(CommonArgs),

    /// Build the documentation for a target
    Doc(CommonArgs),

    /// Remove build output
    Clean(CleanArgs),

    /// List the targets with images, and how each builds here
    Targets,

    /// Check what cross-compiling needs on this machine
    Doctor,

    /// Manage the warm build containers
    Daemon(DaemonArgs),

    /// Pull and list the build images
    Images(ImagesArgs),

    /// Write a Hyprcross.toml in this directory
    Init(InitArgs),

    /// The agent inside a warm container
    #[command(name = "__agent", hide = true)]
    Agent {
        socket: PathBuf,
        /// Stop after this many idle minutes; 0 for never
        #[arg(long, default_value_t = 15)]
        idle_mins: u32,
    },
}

#[derive(Args, Debug, Clone, Default)]
pub struct SharedBuildOpts {
    /// Target triple (aarch64-unknown-linux-gnu, x86_64-pc-windows-gnu, …)
    #[arg(short, long, env = "HYPRCROSS_TARGET", value_name = "TRIPLE")]
    pub target: Option<String>,

    /// Where to build: auto, direct or container
    #[arg(long, env = "HYPRCROSS_ENGINE", value_name = "ENGINE")]
    pub engine: Option<String>,

    /// Linker: hyprlnk or default
    #[arg(long, env = "HYPRCROSS_LINKER", value_name = "LINKER")]
    pub linker: Option<String>,

    /// Build tool: auto, rune or cargo
    #[arg(long, env = "HYPRCROSS_BUILDER", value_name = "TOOL")]
    pub builder: Option<String>,

    /// Build in a warm container that stays up between builds
    #[arg(long, conflicts_with = "no_daemon")]
    pub daemon: bool,

    /// Build in a fresh container that is removed afterwards
    #[arg(long, conflicts_with = "daemon")]
    pub no_daemon: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct CommonArgs {
    #[command(flatten)]
    pub opts: SharedBuildOpts,

    /// Arguments for rune or cargo (`--release`, `-p app`, …)
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub extra_args: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct CleanArgs {
    #[command(flatten)]
    pub opts: SharedBuildOpts,

    /// Arguments for `rune clean` or `cargo clean`
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub extra_args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: Option<DaemonAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonAction {
    /// List the warm containers
    Status,
    /// Start the warm container for a target
    Start {
        #[arg(short, long, value_name = "TRIPLE")]
        target: Option<String>,
    },
    /// Stop and remove warm containers
    Stop {
        #[arg(short, long, value_name = "TRIPLE")]
        target: Option<String>,
        /// Every hyprcross container, of every project
        #[arg(long)]
        all: bool,
    },
    /// Remove warm containers that have stopped
    Prune,
}

#[derive(Args, Debug, Clone)]
pub struct ImagesArgs {
    #[command(subcommand)]
    pub action: ImagesAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ImagesAction {
    /// List the build images on this machine
    List,
    /// Pull a target's image (warm containers then restart on it)
    Pull {
        #[arg(short, long, value_name = "TRIPLE")]
        target: String,
    },
}

#[derive(Args, Debug, Clone, Default)]
pub struct InitArgs {
    /// The default target to write
    #[arg(short, long, value_name = "TRIPLE")]
    pub target: Option<String>,
}
