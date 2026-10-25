use clap::{Args, Parser, Subcommand};

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
        "hyprcross build -t aarch64-apple-darwin",
        "Apple Silicon macOS",
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
    about = "Cross-compile and cross-test Rust natively, with Igel or Cargo",
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

/// The command line, named after the host (`hyprcross`, or `igl cross`).
pub fn command() -> clap::Command {
    use clap::CommandFactory;
    let name = crate::host::command();
    Cli::command().name(name).bin_name(name)
}

/// Parse `args` (the first naming the command) with [`command`].
pub fn try_parse(args: &[std::ffi::OsString]) -> Result<Cli, clap::Error> {
    use clap::FromArgMatches;
    let matches = command().try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
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

    /// List supported targets, and how each builds here
    Targets(TargetsArgs),

    /// Check what cross-compiling needs on this machine
    Doctor,

    /// Manage target SDKs and sysroots for native cross-linking
    Sdk(SdkArgs),

    /// Write a configuration with a default target
    Init(InitArgs),
}

#[derive(Args, Debug, Clone, Default)]
pub struct SharedBuildOpts {
    /// Target triple (aarch64-unknown-linux-gnu, x86_64-pc-windows-gnu, …)
    #[arg(short, long, env = "HYPRCROSS_TARGET", value_name = "TRIPLE")]
    pub target: Option<String>,

    /// Where to build: direct
    #[arg(long, env = "HYPRCROSS_ENGINE", value_name = "ENGINE")]
    pub engine: Option<String>,

    /// Linker: hyprlnk or default
    #[arg(long, env = "HYPRCROSS_LINKER", value_name = "LINKER")]
    pub linker: Option<String>,

    /// Build tool: auto, igl or cargo
    #[arg(long, env = "HYPRCROSS_BUILDER", value_name = "TOOL")]
    pub builder: Option<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct CommonArgs {
    #[command(flatten)]
    pub opts: SharedBuildOpts,

    /// Arguments for igl or cargo (`--release`, `-p app`, …)
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

    /// Arguments for `igl clean` or `cargo clean`
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub extra_args: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct TargetsArgs {
    /// Optional search query to filter targets (e.g. "windows arm", "linux aarch64", "musl")
    #[arg(trailing_var_arg = true, value_name = "QUERY")]
    pub query: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct InitArgs {
    /// The default target to write
    #[arg(short, long, value_name = "TRIPLE")]
    pub target: Option<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct SdkArgs {
    #[command(subcommand)]
    pub action: Option<SdkAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SdkAction {
    /// List available and installed target SDKs
    List,
    /// Download and install SDKs for one or more targets
    #[command(alias = "pull")]
    Install {
        /// Target triple(s) to install (e.g. aarch64-unknown-linux-gnu)
        #[arg(value_name = "TARGETS")]
        targets: Vec<String>,
        /// Install SDKs for all known targets
        #[arg(long)]
        all: bool,
    },
    /// Remove installed SDKs for one or more targets
    Remove {
        /// Target triple(s) to remove
        #[arg(value_name = "TARGETS")]
        targets: Vec<String>,
        /// Remove all installed SDKs
        #[arg(long)]
        all: bool,
    },
    /// Print the filesystem path of a target's SDK
    Path {
        /// Target triple
        #[arg(value_name = "TARGET")]
        target: String,
    },
}
