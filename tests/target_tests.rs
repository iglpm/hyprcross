use hyprcross::targets::{IMAGE_TARGETS, TargetArch, TargetInfo, TargetOs};

#[test]
fn test_all_primary_targets() {
    let targets = [
        (
            "x86_64-unknown-linux-gnu",
            TargetArch::X86_64,
            TargetOs::Linux,
        ),
        (
            "x86_64-unknown-linux-musl",
            TargetArch::X86_64,
            TargetOs::Linux,
        ),
        (
            "aarch64-unknown-linux-gnu",
            TargetArch::Aarch64,
            TargetOs::Linux,
        ),
        (
            "aarch64-unknown-linux-musl",
            TargetArch::Aarch64,
            TargetOs::Linux,
        ),
        (
            "armv7-unknown-linux-gnueabihf",
            TargetArch::Armv7,
            TargetOs::Linux,
        ),
        (
            "riscv64gc-unknown-linux-gnu",
            TargetArch::Riscv64,
            TargetOs::Linux,
        ),
        (
            "powerpc64le-unknown-linux-gnu",
            TargetArch::Powerpc64le,
            TargetOs::Linux,
        ),
        (
            "powerpc64-unknown-linux-gnu",
            TargetArch::Powerpc64,
            TargetOs::Linux,
        ),
        (
            "s390x-unknown-linux-gnu",
            TargetArch::S390x,
            TargetOs::Linux,
        ),
        (
            "loongarch64-unknown-linux-gnu",
            TargetArch::Loongarch64,
            TargetOs::Linux,
        ),
        (
            "x86_64-pc-windows-msvc",
            TargetArch::X86_64,
            TargetOs::Windows,
        ),
        ("aarch64-apple-darwin", TargetArch::Aarch64, TargetOs::MacOS),
        (
            "x86_64-unknown-freebsd",
            TargetArch::X86_64,
            TargetOs::FreeBsd,
        ),
        ("wasm32-unknown-unknown", TargetArch::Wasm32, TargetOs::None),
        ("wasm32-wasip1", TargetArch::Wasm32, TargetOs::Wasi),
        ("thumbv7em-none-eabihf", TargetArch::Arm, TargetOs::None),
    ];

    for (triple, expected_arch, expected_os) in targets {
        let info = TargetInfo::parse(triple);
        assert_eq!(info.arch, expected_arch, "arch mismatch for {triple}");
        assert_eq!(info.os, expected_os, "os mismatch for {triple}");
        assert_eq!(info.display_name(), triple);
    }
}

#[test]
fn test_qemu_mapping() {
    let q = |t: &str| TargetInfo::parse(t).qemu_binary;
    assert_eq!(
        q("riscv64gc-unknown-linux-gnu"),
        Some("qemu-riscv64-static")
    );
    assert_eq!(
        q("powerpc64le-unknown-linux-gnu"),
        Some("qemu-ppc64le-static")
    );
    assert_eq!(q("s390x-unknown-linux-gnu"), Some("qemu-s390x-static"));
    assert_eq!(q("armv7-unknown-linux-gnueabihf"), Some("qemu-arm-static"));
    assert_eq!(q("i686-unknown-linux-gnu"), Some("qemu-i386-static"));
    assert_eq!(q("x86_64-pc-windows-gnu"), None);
}

#[test]
fn test_prebuilt_image_support() {
    let linux_gnu = TargetInfo::parse("aarch64-unknown-linux-gnu");
    assert!(linux_gnu.has_prebuilt_image());
    assert_eq!(linux_gnu.target_c_compiler(), Some("aarch64-linux-gnu-gcc"));

    let win_msvc = TargetInfo::parse("i686-pc-windows-msvc");
    assert!(!win_msvc.has_prebuilt_image());
    assert_eq!(
        win_msvc.suggested_alternative(),
        Some("i686-pc-windows-gnu".to_string())
    );

    let win_msvc64 = TargetInfo::parse("x86_64-pc-windows-msvc");
    assert!(!win_msvc64.has_prebuilt_image());
    assert_eq!(
        win_msvc64.suggested_alternative(),
        Some("x86_64-pc-windows-gnu".to_string())
    );

    let win_gnu = TargetInfo::parse("x86_64-pc-windows-gnu");
    assert!(win_gnu.has_prebuilt_image());
    assert_eq!(win_gnu.target_c_compiler(), Some("x86_64-w64-mingw32-gcc"));

    assert!(!TargetInfo::parse("aarch64-apple-darwin").has_prebuilt_image());
}

/// Every target in `IMAGE_TARGETS` has an SDK definition in `KNOWN_SDKS` or links without a C toolchain.
#[test]
fn every_target_has_sdk_support() {
    for target in IMAGE_TARGETS {
        let info = TargetInfo::parse(target);
        if info.links_without_c_toolchain() {
            continue;
        }
        assert!(
            hyprcross::sdk::find_sdk_def(target).is_some(),
            "target {target} missing SDK definition in KNOWN_SDKS"
        );
    }
}

#[test]
fn test_macos_targets_auto_engine_is_direct() {
    let config = hyprcross::config::Config::default();
    for target in ["aarch64-apple-darwin", "x86_64-apple-darwin"] {
        let info = TargetInfo::parse(target);
        assert_eq!(
            hyprcross::engine::auto_engine(&info, &config),
            hyprcross::engine::Engine::Direct,
            "{target} should build direct without containers"
        );
    }
}

#[test]
fn test_targets_search_matching() {
    let arm_win = TargetInfo::parse("aarch64-pc-windows-msvc");
    assert!(arm_win.matches_query(&["windows".to_string(), "arm".to_string()]));
    assert!(arm_win.matches_query(&["windows arm".to_string()]));
    assert!(arm_win.matches_query(&["msvc".to_string()]));

    let mac = TargetInfo::parse("aarch64-apple-darwin");
    assert!(mac.matches_query(&["macos".to_string()]));
    assert!(mac.matches_query(&["apple".to_string()]));
    assert!(mac.matches_query(&["darwin".to_string()]));
    assert!(!mac.matches_query(&["windows".to_string()]));

    let musl = TargetInfo::parse("x86_64-unknown-linux-musl");
    assert!(musl.matches_query(&["musl".to_string()]));
    assert!(musl.matches_query(&["linux x64".to_string()]));
}
