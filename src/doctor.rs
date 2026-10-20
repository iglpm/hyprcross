//! `hyprcross doctor`: what this machine has for cross-compiling, and what to do about the rest.

use colored::Colorize;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::builder::find_executable;
use crate::targets::{ALL_TARGETS, TargetInfo, current_host_triple};
use crate::ui;

struct Report {
    problems: usize,
}

impl Report {
    fn ok(&self, message: impl std::fmt::Display) {
        ui::err(format!("  {} {message}", "✓".green().bold()));
    }

    fn info(&self, message: impl std::fmt::Display) {
        ui::err(format!("  {} {message}", ui::dim("·")));
    }

    fn problem(&mut self, message: impl std::fmt::Display, fix: impl std::fmt::Display) {
        self.problems += 1;
        ui::err(format!(
            "  {} {}",
            "!".yellow().bold(),
            message.to_string().yellow()
        ));
        for line in ui::wrap(
            &fix.to_string(),
            ui::term_width().saturating_sub(12).max(30),
        ) {
            ui::err(format!("    {}", ui::dim(line)));
        }
    }
}

fn version_of(program: &Path) -> Option<String> {
    let out = Command::new(program)
        .arg("--version")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.trim().to_string();
    (!line.is_empty()).then_some(line)
}

pub fn run_doctor() {
    let mut r = Report { problems: 0 };
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    ui::err(format!(
        "{} {}",
        ui::brand(format!("{} doctor", crate::host::command())),
        ui::dim(format!("· host {}", current_host_triple()))
    ));

    // The Rust toolchain and its standard libraries.
    match find_executable("rustc").and_then(|p| version_of(&p)) {
        Some(v) => {
            let targets = crate::workspace::active_toolchain(&cwd)
                .and_then(|tc| std::fs::read_dir(tc.join("lib/rustlib")).ok())
                .map(|d| {
                    d.filter_map(|e| e.ok())
                        .filter(|e| e.path().join("lib").is_dir())
                        .count()
                })
                .unwrap_or(0);
            if targets > 0 {
                r.ok(format!(
                    "{v}, with {}",
                    ui::plural(targets, "standard library")
                ));
            } else {
                r.ok(v);
            }
        }
        None => r.problem(
            "rustc is not on PATH",
            "install Rust with rustup: https://rustup.rs",
        ),
    }

    // The builder: the host itself (`rune cross`), else Rune or Cargo on PATH.
    let host = crate::host::get();
    match host
        .builder
        .as_ref()
        .map(|b| b.path.clone())
        .or_else(|| find_executable("rune"))
    {
        Some(rune) => {
            let v = version_of(&rune).unwrap_or_else(|| "rune".into());
            r.ok(format!(
                "{v} builds {}",
                ui::dim("(content-addressed cache)")
            ));
            if rune.to_string_lossy().contains("/target/debug/") {
                r.problem(
                    format!("the rune on PATH is a debug build: {}", ui::tilde(&rune)),
                    "debug builds of rune are several times slower, cached builds included; \
                     put a release build (target/release) first on PATH",
                );
            }
        }
        None => match find_executable("cargo") {
            Some(cargo) => r.ok(format!(
                "{} builds {}",
                version_of(&cargo).unwrap_or_else(|| "cargo".into()),
                ui::dim("(install Rune for content-addressed build cache)")
            )),
            None => r.problem(
                "neither rune nor cargo is on PATH",
                "install Rust with rustup: https://rustup.rs",
            ),
        },
    }

    if host.links_with_hyprlnk {
        let program = host.command.split(' ').next().unwrap_or(host.command);
        r.ok(format!(
            "hyprlnk links ELF, Mach-O and PE/COFF {}",
            ui::dim(format!("(built into {program})"))
        ));
    } else {
        #[cfg(feature = "builtin-hyprlnk")]
        r.ok(format!(
            "hyprlnk {} links ELF, Mach-O and PE/COFF {}",
            hyprlnk::VERSION,
            ui::dim("(built in)")
        ));
        #[cfg(not(feature = "builtin-hyprlnk"))]
        match crate::hyprlnk_driver::find_external_hyprlnk() {
            Some(p) => r.ok(format!("hyprlnk at {}", ui::tilde(&p))),
            None => {
                r.info("hyprlnk is not built in and not on PATH: builds use the default linkers")
            }
        }
    }

    // Target SDKs and sysroots.
    let sdks_dir = crate::sdk::default_sdks_dir();
    let installed_sdks: usize = crate::sdk::KNOWN_SDKS
        .iter()
        .filter(|s| crate::sdk::is_valid_sdk(&crate::sdk::sdk_path_for_target(s.triple)))
        .count();
    if installed_sdks > 0 {
        r.ok(format!(
            "target SDKs: {} installed in {}",
            ui::plural(installed_sdks, "SDK"),
            ui::dim(ui::tilde(&sdks_dir))
        ));
    } else {
        r.ok(format!(
            "target SDKs: {} available for automatic download to {}",
            crate::sdk::KNOWN_SDKS.len(),
            ui::dim(ui::tilde(&sdks_dir))
        ));
    }

    // Clang / host C compiler for cross-linking driver
    match find_executable("clang") {
        Some(clang) => {
            let v = version_of(&clang).unwrap_or_else(|| "clang".into());
            r.ok(format!("{v} available as universal cross-linking driver"));
        }
        None => {
            r.info("clang not found: native GCC cross-compilers or hyprlnk will be used");
        }
    }

    // Emulators for tests.
    let qemu: Vec<&str> = [
        "aarch64",
        "arm",
        "riscv64",
        "ppc64le",
        "s390x",
        "i386",
        "loongarch64",
        "ppc64",
        "ppc",
        "sparc64",
    ]
    .into_iter()
    .filter(|a| {
        find_executable(&format!("qemu-{a}-static")).is_some()
            || find_executable(&format!("qemu-{a}")).is_some()
    })
    .collect();
    if qemu.is_empty() {
        r.info("no QEMU on this machine: cross-architecture tests require QEMU or binfmt_misc");
    } else {
        let binfmt = Path::new("/proc/sys/fs/binfmt_misc/status").exists();
        r.ok(format!(
            "QEMU runs {} programs {}",
            qemu.join(", "),
            ui::dim(if binfmt { "(binfmt_misc on)" } else { "" })
        ));
    }
    for (program, what) in [("wine", "Windows"), ("wasmtime", "WASI")] {
        if find_executable(program).is_some() {
            r.ok(format!("{program} runs {what} programs"));
        }
    }

    // C toolchains for direct builds.
    let direct: Vec<String> = ALL_TARGETS
        .iter()
        .map(|t| TargetInfo::parse(t))
        .filter(|t| !t.is_host() && !t.links_without_c_toolchain())
        .filter(|t| crate::direct::host_c_compiler(t).is_some())
        .map(|t| t.triple)
        .collect();
    if !direct.is_empty() {
        r.ok(format!(
            "{} native host C cross-compilers found {}",
            ui::plural(direct.len(), "toolchain"),
            ui::dim(format!("({})", direct.join(", ")))
        ));
    }

    ui::err("");
    if r.problems == 0 {
        ui::err(format!("{} ready to cross-compile", "✓".green().bold()));
    } else {
        ui::err(format!(
            "{} {} to look at",
            "!".yellow().bold(),
            ui::plural(r.problems, "thing")
        ));
    }
}
