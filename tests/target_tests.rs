use hyprcross::targets::{TargetArch, TargetInfo, TargetOs};

#[test]
fn test_all_primary_targets() {
    let targets = [
        ("x86_64-unknown-linux-gnu", TargetArch::X86_64, TargetOs::Linux),
        ("x86_64-unknown-linux-musl", TargetArch::X86_64, TargetOs::Linux),
        ("aarch64-unknown-linux-gnu", TargetArch::Aarch64, TargetOs::Linux),
        ("aarch64-unknown-linux-musl", TargetArch::Aarch64, TargetOs::Linux),
        ("armv7-unknown-linux-gnueabihf", TargetArch::Armv7, TargetOs::Linux),
        ("riscv64gc-unknown-linux-gnu", TargetArch::Riscv64, TargetOs::Linux),
        ("powerpc64le-unknown-linux-gnu", TargetArch::Powerpc64le, TargetOs::Linux),
        ("s390x-unknown-linux-gnu", TargetArch::S390x, TargetOs::Linux),
        ("x86_64-pc-windows-msvc", TargetArch::X86_64, TargetOs::Windows),
        ("aarch64-apple-darwin", TargetArch::Aarch64, TargetOs::MacOS),
        ("wasm32-unknown-unknown", TargetArch::Wasm32, TargetOs::None),
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
    let t_riscv = TargetInfo::parse("riscv64gc-unknown-linux-gnu");
    assert_eq!(t_riscv.qemu_binary, Some("qemu-riscv64-static"));

    let t_ppc = TargetInfo::parse("powerpc64le-unknown-linux-gnu");
    assert_eq!(t_ppc.qemu_binary, Some("qemu-ppc64le-static"));

    let t_s390x = TargetInfo::parse("s390x-unknown-linux-gnu");
    assert_eq!(t_s390x.qemu_binary, Some("qemu-s390x-static"));

    let t_arm = TargetInfo::parse("armv7-unknown-linux-gnueabihf");
    assert_eq!(t_arm.qemu_binary, Some("qemu-arm-static"));
}

#[test]
fn test_prebuilt_image_support() {
    let linux_gnu = TargetInfo::parse("aarch64-unknown-linux-gnu");
    assert!(linux_gnu.has_prebuilt_image());
    assert_eq!(linux_gnu.target_c_compiler(), Some("aarch64-linux-gnu-gcc"));

    let win_msvc = TargetInfo::parse("i686-pc-windows-msvc");
    assert!(!win_msvc.has_prebuilt_image());
    assert_eq!(win_msvc.suggested_alternative(), Some("i686-pc-windows-gnu".to_string()));

    let win_msvc64 = TargetInfo::parse("x86_64-pc-windows-msvc");
    assert!(!win_msvc64.has_prebuilt_image());
    assert_eq!(win_msvc64.suggested_alternative(), Some("x86_64-pc-windows-gnu".to_string()));

    let win_gnu = TargetInfo::parse("x86_64-pc-windows-gnu");
    assert!(win_gnu.has_prebuilt_image());
    assert_eq!(win_gnu.target_c_compiler(), Some("x86_64-w64-mingw32-gcc"));

    let macos = TargetInfo::parse("aarch64-apple-darwin");
    assert!(!macos.has_prebuilt_image());
}

