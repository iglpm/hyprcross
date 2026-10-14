//! hyprcross: Next-generation, high-performance cross-compilation and cross-testing
//! engine for Rust and Rune.

pub mod builder;
pub mod cli;
pub mod config;
pub mod container;
pub mod daemon;
pub mod direct;
pub mod doctor;
pub mod engine;
pub mod hyprlnk_driver;
pub mod qemu;
pub mod targets;
pub mod ui;
pub mod workspace;

use anyhow::{bail, Result};
use clap::Parser;
use std::ffi::OsString;
use std::process::Command;

use crate::builder::Builder;
use crate::cli::{Cli, Commands, DaemonAction, ImagesAction};
use crate::config::Config;
use crate::container::ContainerEngine;
use crate::engine::{init_project, ExecutionContext};
use crate::targets::TargetInfo;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> &'static str {
    VERSION
}

/// Main entry point for the hyprcross CLI and cargo-hyprcross binary
pub fn run() -> Result<i32> {
    let mut raw_args: Vec<OsString> = std::env::args_os().collect();

    // Check if invoked as `__hyprlnk` or if HYPRCROSS_HYPRLNK is set (acting as the linker)
    if raw_args.len() > 1 && raw_args[1] == "__hyprlnk" {
        let linker_args = raw_args.into_iter().skip(1);
        std::process::exit(hyprlnk_driver::run_linker(linker_args));
    }
    if std::env::var_os("HYPRCROSS_HYPRLNK").is_some() {
        std::process::exit(hyprlnk_driver::run_linker(raw_args));
    }

    // Support `cargo hyprcross ...` invocation where Cargo passes "hyprcross" as argv[1]
    if raw_args.len() > 1 && raw_args[1] == "hyprcross" {
        raw_args.remove(1);
    }

    let cli = Cli::parse_from(raw_args);

    let cwd = std::env::current_dir()?;
    let config = Config::load_from_dir(&cwd)?;

    match cli.command {
        Some(Commands::Doctor) => {
            doctor::run_doctor();
            Ok(0)
        }
        Some(Commands::Init(args)) => {
            init_project(args.target.as_deref())?;
            Ok(0)
        }
        Some(Commands::Hyprlnk { args }) => {
            std::process::exit(hyprlnk_driver::run_linker(args));
        }
        Some(Commands::Daemon(args)) => {
            let container_engine = ContainerEngine::detect(&config.build.container_engine)?;
            let action = args.action.unwrap_or(DaemonAction::Status);
            match action {
                DaemonAction::Status => {
                    ui::info("daemon", "Checking active daemon container sessions...");
                    let mut cmd = Command::new(&container_engine.path);
                    cmd.args(["ps", "--filter", "name=hyprcross-"]);
                    let status = cmd.status()?;
                    Ok(status.code().unwrap_or(0))
                }
                DaemonAction::Start { target } => {
                    let target_str = target
                        .or_else(|| config.build.default_target.clone())
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Please specify a target to warm (e.g. hyprcross daemon start --target aarch64-unknown-linux-gnu)"
                            )
                        })?;
                    let target_info = TargetInfo::parse(&target_str);
                    let builder = Builder::resolve(&config.build.builder)?;
                    let daemon = daemon::ContainerDaemon::new(
                        &container_engine,
                        &target_info,
                        &config,
                        &builder,
                        true,
                    );
                    daemon.ensure_started()?;
                    ui::success("daemon", format!("Container `{}` is warm and running", daemon.container_name()));
                    Ok(0)
                }
                DaemonAction::Stop { target, all } => {
                    if all {
                        ui::info("daemon", "Stopping all hyprcross daemon containers...");
                        let output = Command::new(&container_engine.path)
                            .args(["ps", "-q", "--filter", "name=hyprcross-"])
                            .output()?;
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for id in stdout.lines() {
                            let _ = Command::new(&container_engine.path).args(["rm", "-f", "-t", "0", id.trim()]).status();
                        }
                        ui::success("daemon", "All hyprcross containers stopped");
                    } else if let Some(t) = target.or_else(|| config.build.default_target.clone()) {
                        let target_info = TargetInfo::parse(&t);
                        let builder = Builder::resolve(&config.build.builder)?;
                        let daemon = daemon::ContainerDaemon::new(
                            &container_engine,
                            &target_info,
                            &config,
                            &builder,
                            true,
                        );
                        daemon.stop()?;
                    } else {
                        bail!("Please specify --target <triple> or --all to stop daemon containers");
                    }
                    Ok(0)
                }
                DaemonAction::Prune => {
                    ui::info("daemon", "Pruning exited hyprcross containers...");
                    let output = Command::new(&container_engine.path)
                        .args(["ps", "-a", "-q", "--filter", "name=hyprcross-", "--filter", "status=exited"])
                        .output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for id in stdout.lines() {
                        let _ = Command::new(&container_engine.path).args(["rm", "-f", "-t", "0", id.trim()]).status();
                    }
                    ui::success("daemon", "Prune complete");
                    Ok(0)
                }
            }
        }
        Some(Commands::Images(args)) => {
            let container_engine = ContainerEngine::detect(&config.build.container_engine)?;
            match args.action {
                ImagesAction::List => {
                    ui::info("images", "Listing cached cross-compilation images:");
                    let mut cmd = Command::new(&container_engine.path);
                    cmd.args(["images", "--filter", "reference=*cross*"]);
                    let status = cmd.status()?;
                    Ok(status.code().unwrap_or(0))
                }
                ImagesAction::Pull { target } => {
                    let target_info = TargetInfo::parse(&target);
                    let image = if let Some(tc) = config.get_target(&target) {
                        tc.image.clone().unwrap_or(target_info.default_image)
                    } else {
                        target_info.default_image
                    };
                    ui::info("images", format!("Pulling image `{image}`..."));
                    let mut cmd = Command::new(&container_engine.path);
                    cmd.args(["pull", &image]);
                    let status = cmd.status()?;
                    Ok(status.code().unwrap_or(0))
                }
            }
        }
        Some(Commands::Clean(args)) => {
            let builder = Builder::resolve(&config.build.builder)?;
            let mut cmd = builder.command("clean");
            if let Some(t) = args.target
                && builder.kind == crate::builder::BuilderKind::Cargo {
                    cmd.arg("--target").arg(t);
                }
            for a in args.extra_args {
                cmd.arg(a);
            }
            let status = cmd.status()?;
            Ok(status.code().unwrap_or(0))
        }
        Some(Commands::Build(args)) => {
            dispatch_build_command("build", args, config)
        }
        Some(Commands::Check(args)) => {
            dispatch_build_command("check", args, config)
        }
        Some(Commands::Test(args)) => {
            dispatch_build_command("test", args, config)
        }
        Some(Commands::Run(args)) => {
            dispatch_build_command("run", args, config)
        }
        Some(Commands::Clippy(args)) => {
            dispatch_build_command("clippy", args, config)
        }
        None => {
            // Default to `build` if options were provided or show help
            if cli.build_opts.target.is_some() || config.build.default_target.is_some() {
                let args = crate::cli::CommonArgs {
                    opts: cli.build_opts,
                    extra_args: Vec::new(),
                };
                dispatch_build_command("build", args, config)
            } else {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                cmd.print_help()?;
                println!();
                Ok(0)
            }
        }
    }
}

fn dispatch_build_command(
    subcommand: &str,
    args: crate::cli::CommonArgs,
    config: Config,
) -> Result<i32> {
    let target_str = args
        .opts
        .target
        .or_else(|| config.build.default_target.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No cross-compilation target specified. Pass `--target <triple>` or configure `default_target` in Hyprcross.toml"
            )
        })?;

    let builder_pref = args
        .opts
        .builder
        .unwrap_or_else(|| config.build.builder.clone());
    let builder = Builder::resolve(&builder_pref)?;

    let daemon_override = if args.opts.daemon {
        Some(true)
    } else if args.opts.no_daemon {
        Some(false)
    } else {
        None
    };

    let ctx = ExecutionContext::new(
        &target_str,
        config,
        builder,
        args.opts.engine,
        args.opts.linker,
        daemon_override,
    );

    ctx.run_command(subcommand, &args.extra_args)
}
