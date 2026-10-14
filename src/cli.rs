use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "hyprcross",
    bin_name = "hyprcross",
    author = "Jonas <me@byjonas.dev>",
    version,
    about = "Blazing-fast, zero-overhead cross-compilation and cross-testing with hyprlnk acceleration",
    long_about = "A next-generation replacement for `cross` offering direct host execution, persistent container daemons, and 2x-4x faster linking via hyprlnk."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub build_opts: SharedBuildOpts,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile the current package for the target architecture
    Build(CommonArgs),

    /// Analyze the current package and report errors without code generation
    Check(CommonArgs),

    /// Execute unit and integration tests using QEMU target runner
    Test(CommonArgs),

    /// Build and execute the target binary with QEMU user emulation
    Run(CommonArgs),

    /// Run linter checks for the target architecture
    Clippy(CommonArgs),

    /// Remove target build artifacts
    Clean(CleanArgs),

    /// Diagnose host cross-compilation environment and dependencies
    Doctor,

    /// Manage persistent warm container daemon sessions
    Daemon(DaemonArgs),

    /// Manage cross container images
    Images(ImagesArgs),

    /// Initialize Hyprcross.toml configuration in the current directory
    Init(InitArgs),

    /// Internal linker entry point dispatching directly to hyprlnk
    #[command(name = "__hyprlnk", hide = true)]
    Hyprlnk {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Args, Debug, Clone, Default)]
pub struct SharedBuildOpts {
    /// Target triple (e.g. aarch64-unknown-linux-gnu, x86_64-unknown-linux-musl)
    #[arg(short, long, env = "HYPRCROSS_TARGET")]
    pub target: Option<String>,

    /// Execution engine: "auto", "direct", or "container"
    #[arg(long, env = "HYPRCROSS_ENGINE")]
    pub engine: Option<String>,

    /// Linker acceleration: "hyprlnk" or "default"
    #[arg(long, env = "HYPRCROSS_LINKER")]
    pub linker: Option<String>,

    /// Build orchestrator: "auto", "rune", or "cargo"
    #[arg(long, env = "HYPRCROSS_BUILDER")]
    pub builder: Option<String>,

    /// Keep container warm in background for instant sub-20ms invocations
    #[arg(long, conflicts_with = "no_daemon")]
    pub daemon: bool,

    /// Disable persistent warm container daemon (use one-off docker run)
    #[arg(long, conflicts_with = "daemon")]
    pub no_daemon: bool,

    /// Enable verbose diagnostic logs
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct CommonArgs {
    #[command(flatten)]
    pub opts: SharedBuildOpts,

    /// Additional arguments forwarded directly to rune / cargo
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct CleanArgs {
    #[arg(short, long)]
    pub target: Option<String>,

    /// Additional arguments forwarded to rune / cargo clean
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: Option<DaemonAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonAction {
    /// Show running daemon container sessions
    Status,
    /// Warm up daemon container for a specific target
    Start {
        #[arg(short, long)]
        target: Option<String>,
    },
    /// Stop running daemon container for a target
    Stop {
        #[arg(short, long)]
        target: Option<String>,
        /// Stop all running hyprcross daemon containers
        #[arg(long)]
        all: bool,
    },
    /// Remove stopped or orphaned hyprcross daemon containers
    Prune,
}

#[derive(Args, Debug, Clone)]
pub struct ImagesArgs {
    #[command(subcommand)]
    pub action: ImagesAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ImagesAction {
    /// List cached cross container images
    List,
    /// Pull image for target
    Pull {
        #[arg(short, long)]
        target: String,
    },
}

#[derive(Args, Debug, Clone, Default)]
pub struct InitArgs {
    /// Default target to configure in Hyprcross.toml
    #[arg(short, long)]
    pub target: Option<String>,
}
