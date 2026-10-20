//! hyprcross: cross-compile and cross-test Rust, with Rune or Cargo natively on the host,
//! linking with hyprlnk.

pub mod builder;
pub mod cli;
pub mod config;
pub mod crash;
pub mod direct;
pub mod doctor;
pub mod engine;
pub mod help;
pub mod host;
pub mod hyprlnk_driver;
pub mod macos;
pub mod qemu;
pub mod sdk;
pub mod targets;
pub mod ui;
pub mod workspace;

use anyhow::Result;
use colored::Colorize;
use std::ffi::OsString;

use crate::builder::Builder;
use crate::cli::{Commands, CommonArgs, SharedBuildOpts};
use crate::config::Config;
use crate::engine::{ExecutionContext, auto_engine, init_project};
use crate::targets::{ALL_TARGETS, TargetInfo, TargetOs};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> &'static str {
    VERSION
}

/// Run `hyprcross` (or `cargo hyprcross`) with this process' arguments; the exit code.
pub fn run() -> Result<i32> {
    crash::init();
    let mut args: Vec<OsString> = std::env::args_os().collect();

    if let Some(code) = hyprlnk_driver::linker_invocation(&args) {
        return Ok(code);
    }
    // Cargo runs `cargo-hyprcross hyprcross <args>`.
    if args.get(1).is_some_and(|a| a == "hyprcross") {
        args.remove(1);
    }
    dispatch(args)
}

/// Run hyprcross inside another program (`rune cross`): `args` as hyprcross would get them,
/// the first one naming the command (`rune cross`); the exit code.
pub fn run_hosted(args: Vec<OsString>, host: host::Host) -> Result<i32> {
    host::set(host);
    dispatch(args)
}

fn dispatch(args: Vec<OsString>) -> Result<i32> {
    apply_color_flag(&args);
    if help::intercept(&args) {
        return Ok(0);
    }
    let args = hoist_own_options(args);

    let cli = match cli::try_parse(&args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return Ok(e.exit_code());
        }
    };
    ui::set_quiet(cli.quiet);
    ui::set_verbose(cli.verbose);

    let cwd = std::env::current_dir()?;
    let config = Config::load_from_dir(&cwd)?;

    match cli.command {
        Some(Commands::Build(a)) => build("build", a, &cli.build_opts, config),
        Some(Commands::Check(a)) => build("check", a, &cli.build_opts, config),
        Some(Commands::Test(a)) => build("test", a, &cli.build_opts, config),
        Some(Commands::Run(a)) => build("run", a, &cli.build_opts, config),
        Some(Commands::Bench(a)) => build("bench", a, &cli.build_opts, config),
        Some(Commands::Clippy(a)) => build("clippy", a, &cli.build_opts, config),
        Some(Commands::Doc(a)) => build("doc", a, &cli.build_opts, config),
        Some(Commands::Clean(a)) => clean(a, config),
        Some(Commands::Targets(a)) => {
            list_targets(&config, &a.query);
            Ok(0)
        }
        Some(Commands::Doctor) => {
            doctor::run_doctor();
            Ok(0)
        }
        Some(Commands::Init(a)) => {
            match host::get().init {
                Some(init) => init(a.target.as_deref())?,
                None => init_project(a.target.as_deref())?,
            }
            Ok(0)
        }
        Some(Commands::Sdk(a)) => sdk_command(a.action.unwrap_or(cli::SdkAction::List)),
        None => {
            let args = CommonArgs {
                opts: cli.build_opts.clone(),
                extra_args: Vec::new(),
            };
            build("build", args, &SharedBuildOpts::default(), config)
        }
    }
}

/// Commands whose other arguments go to the builder.
const BUILD_COMMANDS: &[&str] = &[
    "build", "b", "check", "c", "test", "t", "run", "r", "bench", "clippy", "doc", "clean",
];

/// hyprcross' options that take a value, and those that don't.
const OWN_WITH_VALUE: &[&str] = &[
    "-t",
    "--target",
    "--engine",
    "--linker",
    "--builder",
    "--color",
];
const OWN_FLAGS: &[&str] = &["-q", "--quiet", "-v", "--verbose", "-h", "--help"];

/// `hyprcross build --release -t x`: hyprcross' own options may come anywhere before `--`, as
/// with Cargo; the rest is the builder's. Moves ours in front of the builder's.
fn hoist_own_options(args: Vec<OsString>) -> Vec<OsString> {
    let Some(sub) = args
        .iter()
        .skip(1)
        .position(|a| a.to_str().is_some_and(|s| BUILD_COMMANDS.contains(&s)))
        .map(|i| i + 1)
    else {
        return args;
    };
    let mut out: Vec<OsString> = args[..=sub].to_vec();
    let mut theirs: Vec<OsString> = Vec::new();
    let mut rest = args[sub + 1..].iter();
    while let Some(a) = rest.next() {
        let s = a.to_str().unwrap_or("");
        if s == "--" {
            theirs.extend(rest.by_ref().cloned());
            break;
        }
        let name = s.split_once('=').map_or(s, |(n, _)| n);
        if OWN_FLAGS.contains(&s) {
            out.push(a.clone());
        } else if OWN_WITH_VALUE.contains(&name) {
            out.push(a.clone());
            if !s.contains('=')
                && let Some(value) = rest.next()
            {
                out.push(value.clone());
            }
        } else {
            theirs.push(a.clone());
        }
    }
    if !theirs.is_empty() {
        out.push("--".into());
        out.extend(theirs);
    }
    out
}

fn apply_color_flag(args: &[OsString]) {
    let mut words = args.iter().filter_map(|a| a.to_str());
    let mut choice = "auto";
    while let Some(w) = words.next() {
        if w == "--" {
            break;
        }
        if let Some(v) = w.strip_prefix("--color=") {
            choice = v;
        } else if w == "--color" {
            choice = words.next().unwrap_or("auto");
        }
    }
    ui::set_color(choice);
}

/// The target: `--target`, else the one given before the command, else `default_target`.
fn target_of(opts: &SharedBuildOpts, outer: &SharedBuildOpts, config: &Config) -> Result<String> {
    opts.target
        .clone()
        .or_else(|| outer.target.clone())
        .or_else(|| config.build.default_target.clone())
        .ok_or_else(|| {
            ui::Hinted::error(
                "no target given",
                format!(
                    "pass one with `--target <triple>` ({} lists supported targets), or set {}",
                    host::invocation("targets"),
                    host::get().default_target_setting
                ),
            )
        })
}

fn build(
    subcommand: &str,
    args: CommonArgs,
    outer: &SharedBuildOpts,
    config: Config,
) -> Result<i32> {
    let opts = &args.opts;
    let target = target_of(opts, outer, &config)?;
    let pick = |a: &Option<String>, b: &Option<String>| a.clone().or_else(|| b.clone());
    let builder_pref =
        pick(&opts.builder, &outer.builder).unwrap_or_else(|| config.build.builder.clone());
    let builder = Builder::resolve(&builder_pref)?;
    let ctx = ExecutionContext::new(
        &target,
        config,
        builder,
        pick(&opts.engine, &outer.engine),
        pick(&opts.linker, &outer.linker),
    );
    ctx.run_command(subcommand, &args.extra_args)
}

fn clean(args: cli::CleanArgs, config: Config) -> Result<i32> {
    let builder = Builder::resolve(
        args.opts
            .builder
            .as_deref()
            .unwrap_or(&config.build.builder),
    )?;
    let mut cmd = builder.command("clean");
    if let Some(t) = args
        .opts
        .target
        .as_ref()
        .or(config.build.default_target.as_ref())
        && builder.kind == builder::BuilderKind::Cargo
    {
        cmd.arg("--target").arg(t);
    }
    cmd.args(&args.extra_args);
    ui::trace(format!("{cmd:?}"));
    Ok(cmd.status()?.code().unwrap_or(1))
}

fn list_targets(config: &Config, query: &[String]) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let all: Vec<&str> = ALL_TARGETS
        .iter()
        .copied()
        .filter(|t| {
            let info = TargetInfo::parse(t);
            info.matches_query(query)
        })
        .collect();

    if all.is_empty() {
        ui::warn(format!("no targets matched query: {}", query.join(" ")));
        ui::hint("try broader search terms like 'windows', 'linux', 'arm', 'macos', 'musl'");
        return;
    }

    let width = all.iter().map(|t| t.len()).max().unwrap_or(0) + 2;
    let query_desc = if query.is_empty() {
        String::new()
    } else {
        format!(" matching `{}`", query.join(" "))
    };
    ui::out(format!(
        "{} {}",
        ui::brand("Targets"),
        ui::dim(format!("· {} available{}", all.len(), query_desc))
    ));
    let mut missing = 0;
    for t in &all {
        let info = TargetInfo::parse(t);
        let engine = match config.get_target(t).and_then(|c| c.engine.clone()) {
            Some(e) => e,
            None if config.build.engine != "auto" => config.build.engine.clone(),
            None => auto_engine(&info, config).name().to_string(),
        };
        let std = crate::workspace::has_target_std(&cwd, t);
        let mark = match std {
            Some(true) => "✓".green().bold().to_string(),
            Some(false) => {
                missing += 1;
                " ".to_string()
            }
            None => ui::dim("?"),
        };
        let runner = match info.host_runner() {
            targets::Runner::None => {
                if info.os == TargetOs::MacOS {
                    "macOS SDK · hyprlnk".to_string()
                } else if info.env == targets::TargetEnv::Msvc {
                    "MSVC SDK · hyprlnk".to_string()
                } else if info.os == TargetOs::Linux {
                    "Linux SDK · hyprlnk".to_string()
                } else {
                    "hyprlnk".to_string()
                }
            }
            r => format!("tests: {r}"),
        };
        let sdk_tag = if crate::sdk::target_needs_sdk(&info) {
            let path = crate::sdk::sdk_path_for_target(t);
            if crate::sdk::is_valid_sdk(&path) {
                ui::dim(" · SDK ready")
            } else {
                ui::dim(" · auto-SDK")
            }
        } else {
            ui::dim(" · native")
        };
        ui::out(format!(
            "  {mark} {t:<width$}{:<11}{}{}",
            engine,
            ui::dim(runner),
            sdk_tag
        ));
    }
    ui::out("");
    ui::out(ui::dim(format!(
        "✓ standard library installed{}. Other targets rustc knows build too, on this machine.",
        if missing > 0 {
            "; the others are installed on first use"
        } else {
            ""
        }
    )));
}

fn sdk_command(action: cli::SdkAction) -> Result<i32> {
    match action {
        cli::SdkAction::List => {
            sdk::list_sdks()?;
            Ok(0)
        }
        cli::SdkAction::Install { targets, all } => {
            sdk::install_sdks(&targets, all)?;
            Ok(0)
        }
        cli::SdkAction::Remove { targets, all } => {
            sdk::remove_sdks(&targets, all)?;
            Ok(0)
        }
        cli::SdkAction::Path { target } => {
            let path = sdk::sdk_path_for_target(&target);
            println!("{}", path.display());
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoist(args: &[&str]) -> Vec<String> {
        hoist_own_options(args.iter().map(OsString::from).collect())
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect()
    }

    #[test]
    fn own_options_are_found_among_the_builders() {
        assert_eq!(
            hoist(&[
                "hyprcross",
                "build",
                "--release",
                "-t",
                "x",
                "-p",
                "app",
                "--engine=direct",
                "-q"
            ]),
            [
                "hyprcross",
                "build",
                "-t",
                "x",
                "--engine=direct",
                "-q",
                "--",
                "--release",
                "-p",
                "app"
            ]
        );
        assert_eq!(
            hoist(&["hyprcross", "test", "-t", "x", "--", "-v", "--nocapture"]),
            ["hyprcross", "test", "-t", "x", "--", "-v", "--nocapture"],
            "after -- everything is the builder's"
        );
        assert_eq!(
            hoist(&["hyprcross", "sdk", "install", "-t", "x"]),
            ["hyprcross", "sdk", "install", "-t", "x"]
        );
        assert_eq!(hoist(&["hyprcross", "build"]), ["hyprcross", "build"]);
    }
}
