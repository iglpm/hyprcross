use hyprcross::builder::{Builder, BuilderKind};
use hyprcross::config::Config;
use hyprcross::hyprlnk_driver;
use hyprcross::targets::{TargetArch, TargetInfo, TargetOs};

#[test]
fn test_target_info_parsing() {
    let t_arm64 = TargetInfo::parse("aarch64-unknown-linux-gnu");
    assert_eq!(t_arm64.arch, TargetArch::Aarch64);
    assert_eq!(t_arm64.os, TargetOs::Linux);
    assert_eq!(t_arm64.qemu_binary, Some("qemu-aarch64-static"));
    assert!(t_arm64.hyprlnk_supported);

    let t_musl = TargetInfo::parse("x86_64-unknown-linux-musl");
    assert_eq!(t_musl.arch, TargetArch::X86_64);
    assert!(t_musl.direct_supported);

    let t_win = TargetInfo::parse("x86_64-pc-windows-msvc");
    assert_eq!(t_win.os, TargetOs::Windows);

    let t_mac = TargetInfo::parse("aarch64-apple-darwin");
    assert_eq!(t_mac.os, TargetOs::MacOS);
}

#[test]
fn test_hyprlnk_flags_generation() {
    let flags_linux = hyprlnk_driver::get_rustflags("aarch64-unknown-linux-gnu", false);
    assert!(flags_linux.iter().any(|f| f.contains("linker=")));

    let flags_mac = hyprlnk_driver::get_rustflags("aarch64-apple-darwin", false);
    assert!(flags_mac.iter().any(|f| f.contains("linker-flavor=ld64.lld")));
}

#[test]
fn test_builder_resolution() {
    let builder_cargo = Builder::resolve("cargo");
    assert!(builder_cargo.is_ok());
    assert_eq!(builder_cargo.unwrap().kind, BuilderKind::Cargo);
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
