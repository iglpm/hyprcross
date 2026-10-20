use hyprcross::builder::{Builder, BuilderKind};
use hyprcross::config::Config;
use hyprcross::hyprlnk_driver;
use hyprcross::targets::{TargetArch, TargetInfo, TargetOs};
use std::process::Command;

fn hyprcross() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hyprcross"))
}

#[test]
fn test_target_info_parsing() {
    let t_arm64 = TargetInfo::parse("aarch64-unknown-linux-gnu");
    assert_eq!(t_arm64.arch, TargetArch::Aarch64);
    assert_eq!(t_arm64.os, TargetOs::Linux);
    assert_eq!(t_arm64.qemu_binary, Some("qemu-aarch64-static"));
    assert!(t_arm64.hyprlnk_supported);

    let t_musl = TargetInfo::parse("x86_64-unknown-linux-musl");
    assert_eq!(t_musl.arch, TargetArch::X86_64);
    assert_eq!(t_musl.env, hyprcross::targets::TargetEnv::Musl);
    assert!(hyprcross::sdk::target_needs_sdk(&TargetInfo::parse(
        "aarch64-unknown-linux-musl"
    )));

    assert_eq!(
        TargetInfo::parse("x86_64-pc-windows-msvc").os,
        TargetOs::Windows
    );
    assert_eq!(
        TargetInfo::parse("aarch64-apple-darwin").os,
        TargetOs::MacOS
    );
    assert_eq!(
        TargetInfo::parse("aarch64-linux-android").os,
        TargetOs::Android
    );
}

#[test]
fn test_linker_flavor() {
    assert_eq!(
        hyprlnk_driver::rustc_linker_flavor("aarch64-apple-darwin"),
        Some("ld64.lld")
    );
    #[cfg(unix)]
    assert_eq!(
        hyprlnk_driver::rustc_linker_flavor("aarch64-unknown-linux-gnu"),
        None
    );
    #[cfg(not(unix))]
    assert_eq!(
        hyprlnk_driver::rustc_linker_flavor("aarch64-unknown-linux-gnu"),
        Some("ld.lld")
    );
}

#[test]
fn test_builder_resolution() {
    let builder_cargo = Builder::resolve("cargo");
    assert!(builder_cargo.is_ok());
    assert_eq!(builder_cargo.unwrap().kind, BuilderKind::Cargo);
    assert!(Builder::resolve("make").is_err());
}

#[test]
fn test_config_parsing_defaults() {
    let toml_str = r#"
        [build]
        engine = "direct"
        linker = "hyprlnk"
        daemon = true

        [target.aarch64-unknown-linux-gnu]
        image = "my-custom-image:latest"
        env = { FOO = "BAR" }
    "#;

    let cfg: Config = toml::from_str(toml_str).expect("Failed to parse config");
    assert_eq!(cfg.build.engine, "direct");
    assert_eq!(cfg.build.linker, "hyprlnk");
    assert!(cfg.build.daemon);

    let target_cfg = cfg.get_target("aarch64-unknown-linux-gnu").unwrap();
    assert_eq!(target_cfg.image.as_deref(), Some("my-custom-image:latest"));
    assert_eq!(target_cfg.env.get("FOO").map(|s| s.as_str()), Some("BAR"));
}

#[test]
fn help_lists_the_commands() {
    let out = hyprcross()
        .arg("--help")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for word in ["build", "test", "targets", "doctor", "sdk", "init"] {
        assert!(text.contains(word), "{word} missing from:\n{text}");
    }
}

#[test]
fn cargo_subcommand_form_is_accepted() {
    let out = Command::new(env!("CARGO_BIN_EXE_hyprcross"))
        .args(["hyprcross", "--version"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains(hyprcross::VERSION));
}

#[test]
fn a_build_without_a_target_says_how_to_give_one() {
    let dir = tempfile::tempdir().unwrap();
    let out = hyprcross()
        .args(["build"])
        .current_dir(dir.path())
        .env_remove("HYPRCROSS_TARGET")
        .env("HOME", dir.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("✗ no target given"), "{err}");
    assert!(err.contains("--target"), "{err}");
}

#[test]
fn targets_lists_every_target() {
    let out = hyprcross()
        .arg("targets")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for t in hyprcross::targets::ALL_TARGETS {
        assert!(text.contains(t), "{t} missing");
    }
}

#[cfg(unix)]
#[test]
fn run_as_hyprlnk_it_links_instead_of_parsing_a_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("hyprlnk");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_hyprcross"), &link).unwrap();
    let out = Command::new(&link).arg("--version").output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !text.contains("Usage"),
        "the CLI answered instead of the linker: {text}"
    );
}
