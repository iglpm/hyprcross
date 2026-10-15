use hyprcross::targets::{IMAGE_TARGETS, TargetArch, TargetInfo, TargetOs};
use std::collections::BTreeSet;
use std::path::Path;

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

/// `docker/Dockerfile.<target>` and `IMAGE_TARGETS` must name the same targets: hyprcross pulls
/// images for the targets it lists, and CI builds one image per Dockerfile.
#[test]
fn every_image_target_has_a_dockerfile_and_back() {
    let docker = Path::new(env!("CARGO_MANIFEST_DIR")).join("docker");
    let files: BTreeSet<String> = std::fs::read_dir(&docker)
        .expect("docker/ exists")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_prefix("Dockerfile.")
                .map(str::to_string)
        })
        .collect();
    let listed: BTreeSet<String> = IMAGE_TARGETS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        files, listed,
        "docker/Dockerfile.* and targets::IMAGE_TARGETS differ"
    );

    for target in &files {
        let text = std::fs::read_to_string(docker.join(format!("Dockerfile.{target}"))).unwrap();
        assert!(
            text.contains(&format!("dev.hyprcross.target=\"{target}\"")),
            "Dockerfile.{target} labels another target"
        );
        assert!(
            text.contains("# hyprcross:platforms "),
            "Dockerfile.{target} doesn't say which platforms CI builds it for"
        );
    }
}
