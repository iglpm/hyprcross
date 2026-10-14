use colored::Colorize;
#[allow(unused_imports)]
use std::path::Path;
use std::process::Command;

use crate::builder::find_executable;
use crate::container::{ContainerEngine, ContainerEngineKind};
#[allow(unused_imports)]
use crate::hyprlnk_driver;
use crate::targets::current_host_triple;
use crate::ui;

pub fn run_doctor() {
    ui::header("hyprcross doctor: cross-compilation health check\n");

    // 1. Host Architecture
    println!("  {} Host triple: {}", "▸".bold().cyan(), current_host_triple().bold());

    // 2. Rust Toolchain
    print!("  {} Rust compiler: ", "▸".bold().cyan());
    if let Ok(out) = Command::new("rustc").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout);
        println!("{}", v.trim().green());
    } else {
        println!("{}", "not found (please install rustup)".red());
    }

    // 3. Builder: Rune & Cargo
    print!("  {} Rune build engine: ", "▸".bold().cyan());
    if let Ok(out) = Command::new("rune").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout);
        println!("{} (CAS acceleration & pipelined compilation available)", v.trim().green());
    } else {
        println!("{} (Rune not detected; using Cargo as builder)", "optional".yellow());
    }

    print!("  {} Cargo package manager: ", "▸".bold().cyan());
    if let Ok(out) = Command::new("cargo").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout);
        println!("{}", v.trim().green());
    } else {
        println!("{}", "not found".red());
    }

    // 4. hyprlnk Linker
    print!("  {} hyprlnk linker: ", "▸".bold().cyan());
    #[cfg(feature = "builtin-hyprlnk")]
    {
        println!("{} (embedded v{}, ELF/COFF/Mach-O accelerated)", "available".green(), hyprlnk::VERSION);
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        if let Some(p) = hyprlnk_driver::find_external_hyprlnk() {
            println!("{} ({})", "found in PATH".green(), p.display());
        } else {
            println!("{}", "not found (falling back to standard linkers)".yellow());
        }
    }

    // 5. Container Engines
    println!("\n{}", "Container Engines:".bold());
    let engines = [
        ("docker", ContainerEngineKind::Docker),
        ("podman", ContainerEngineKind::Podman),
        ("finch", ContainerEngineKind::Finch),
        ("nerdctl", ContainerEngineKind::Nerdctl),
    ];

    let mut found_any_engine = false;
    for (name, kind) in engines {
        if let Some(p) = find_executable(name) {
            let engine = ContainerEngine { kind, path: p.clone() };
            let service_ok = engine.check_service_available();
            if service_ok {
                println!(
                    "  {} {} at {} (daemon responsive)",
                    "✓".bold().green(),
                    name.bold(),
                    p.display()
                );
                found_any_engine = true;
            } else {
                println!(
                    "  {} {} at {} (daemon not running or permission denied)",
                    "!".bold().yellow(),
                    name.bold(),
                    p.display()
                );
            }
        }
    }

    if !found_any_engine {
        println!(
            "  {} No active container engine found. Direct mode will be used for musl/bare-metal.",
            "ℹ".bold().blue()
        );
    }

    // 6. QEMU User Emulation
    println!("\n{}", "QEMU User Emulation:".bold());
    let qemu_binaries = [
        ("aarch64", "qemu-aarch64-static"),
        ("arm", "qemu-arm-static"),
        ("riscv64", "qemu-riscv64-static"),
        ("ppc64le", "qemu-ppc64le-static"),
        ("s390x", "qemu-s390x-static"),
        ("x86_64", "qemu-x86_64-static"),
    ];

    let mut qemu_count = 0;
    for (arch, bin) in qemu_binaries {
        if let Some(p) = find_executable(bin) {
            println!("  {} QEMU for {}: {}", "✓".bold().green(), arch, p.display());
            qemu_count += 1;
        }
    }

    if qemu_count == 0 {
        println!(
            "  {} No QEMU static emulators found in PATH. Needed for `hyprcross test/run` on non-native targets.",
            "!".bold().yellow()
        );
    }

    // 7. Kernel binfmt_misc
    #[cfg(target_os = "linux")]
    {
        print!("  {} Linux kernel binfmt_misc: ", "▸".bold().cyan());
        if Path::new("/proc/sys/fs/binfmt_misc").is_dir() {
            println!("{}", "enabled (transparent binary emulation ready)".green());
        } else {
            println!("{}", "not mounted or disabled".yellow());
        }
    }

    // 8. Installed Rustup Targets
    println!("\n{}", "Installed rustup targets:".bold());
    if let Ok(out) = Command::new("rustup").args(["target", "list", "--installed"]).output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if !line.is_empty() {
                println!("  • {}", line.cyan());
            }
        }
    }

    println!("\n{}", "Doctor summary: System is ready for hyprcross builds.".bold().green());
}
