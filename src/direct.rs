//! Direct mode: the builder runs on the host, with the target's C compiler (when the host has
//! one) as its linker driver and an emulator as its runner.

use anyhow::Result;
use std::process::Command;

use crate::builder::{Builder, BuilderKind, find_executable};
use crate::config::Config;
use crate::hyprlnk_driver;
use crate::qemu;
use crate::targets::TargetInfo;
use crate::ui;

pub struct DirectEngine<'a> {
    pub target: &'a TargetInfo,
    pub config: &'a Config,
    pub builder: &'a Builder,
    pub use_hyprlnk: bool,
}

/// `aarch64-unknown-linux-gnu` → `AARCH64_UNKNOWN_LINUX_GNU`, as in `CARGO_TARGET_<…>_LINKER`.
pub fn target_env_key(triple: &str) -> String {
    triple.replace(['-', '.'], "_").to_uppercase()
}

/// The target's C compiler on this machine: `CC_<triple>` or `CARGO_TARGET_<…>_LINKER` if set,
/// else the first of its usual names on PATH.
pub fn host_c_compiler(target: &TargetInfo) -> Option<String> {
    let key = target_env_key(&target.triple);
    for var in [
        format!("CARGO_TARGET_{key}_LINKER"),
        format!("CC_{}", target.triple.replace(['-', '.'], "_")),
        format!("CC_{}", target.triple),
    ] {
        if let Ok(v) = std::env::var(&var)
            && !v.is_empty()
        {
            return Some(v);
        }
    }
    target
        .c_compiler_candidates()
        .into_iter()
        .find(|name| find_executable(name).is_some())
}

impl<'a> DirectEngine<'a> {
    pub fn new(
        target: &'a TargetInfo,
        config: &'a Config,
        builder: &'a Builder,
        use_hyprlnk: bool,
    ) -> Self {
        Self {
            target,
            config,
            builder,
            use_hyprlnk,
        }
    }

    pub fn command(&self, subcommand: &str, extra_args: &[String]) -> Result<Command> {
        let cwd = std::env::current_dir()?;
        if subcommand != "clean" {
            crate::workspace::ensure_rustup_target(&cwd, &self.target.triple)?;
        }

        let triple = &self.target.triple;
        let key = target_env_key(triple);
        let linker_var = format!("CARGO_TARGET_{key}_LINKER");
        let hyprlnk = self.use_hyprlnk && hyprlnk_driver::supports_target(triple);

        let mut cmd = Command::new(&self.builder.path);
        cmd.args(self.builder.args(subcommand, triple, self.use_hyprlnk));
        cmd.args(extra_args);

        // Without a linker named for it, rustc links a cross target with the host's `cc`, which
        // can't. Name the target's C compiler: Igel makes it hyprlnk's driver, Cargo runs it.
        let driver = (!self.target.is_host())
            .then(|| host_c_compiler(self.target))
            .flatten();
        let fallback_clang = (!self.target.is_host()
            && !hyprlnk
            && driver.is_none()
            && self.target.env != crate::targets::TargetEnv::Msvc
            && self.target.os != crate::targets::TargetOs::None
            && self.target.os != crate::targets::TargetOs::Wasi)
            .then(|| find_executable("clang"))
            .flatten();

        if std::env::var_os(&linker_var).is_none() {
            if let Some(cc) = &driver {
                cmd.env(&linker_var, cc);
            } else if let Some(clang) = &fallback_clang {
                cmd.env(&linker_var, clang);
            }
        }

        let clang_target = match triple.as_str() {
            "x86_64-pc-windows-gnu" => "x86_64-w64-mingw32",
            "i686-pc-windows-gnu" => "i686-w64-mingw32",
            "x86_64-pc-windows-gnullvm" => "x86_64-w64-windows-gnu",
            "aarch64-pc-windows-gnullvm" => "aarch64-w64-windows-gnu",
            "x86_64-unknown-freebsd" => "x86_64-unknown-freebsd14.0",
            "riscv64gc-unknown-linux-musl" => "riscv64-unknown-linux-musl",
            "riscv64gc-unknown-linux-gnu" => "riscv64-unknown-linux-gnu",
            "powerpc64le-unknown-linux-musl" => "powerpc64le-unknown-linux-musl",
            "armv7-unknown-linux-musleabihf" => "armv7-linux-musleabihf",
            "arm-unknown-linux-musleabihf" => "arm-linux-musleabihf",
            _ => triple.as_str(),
        };

        // Ensure the target SDK is available for linking
        let maybe_sdk = if subcommand != "clean" {
            crate::sdk::ensure_sdk(self.target)?
        } else {
            None
        };

        if let Some(sdk) = &maybe_sdk {
            if self.target.os == crate::targets::TargetOs::MacOS {
                cmd.env("SDKROOT", sdk);
            }
            cmd.env("HYPRCROSS_SDK_PATH", sdk);
            let sdk_key = format!("HYPRCROSS_SDK_PATH_{key}");
            cmd.env(sdk_key, sdk);

            let triple_underscore = triple.replace(['-', '.'], "_");
            if let Some(clang) = find_executable("clang") {
                let clang_path = clang.display().to_string();
                let mut cflags = if self.target.os == crate::targets::TargetOs::MacOS {
                    format!("--target={clang_target} -isysroot {}", sdk.display())
                } else {
                    format!("--target={clang_target} --sysroot={}", sdk.display())
                };

                if self.target.os == crate::targets::TargetOs::Windows {
                    // Clang's Windows drivers do not infer a flattened SDK's include paths.
                    for sub in [
                        "usr/include",
                        "include",
                        "crt/include",
                        "sdk/include/ucrt",
                        "sdk/include/shared",
                        "sdk/include/um",
                    ] {
                        let include = sdk.join(sub);
                        if include.is_dir() {
                            cflags.push_str(&format!(" -isystem {}", include.display()));
                        }
                    }
                }

                cmd.env(format!("CC_{triple}"), &clang_path);
                cmd.env(format!("CC_{triple_underscore}"), &clang_path);
                cmd.env(format!("CFLAGS_{triple}"), &cflags);
                cmd.env(format!("CFLAGS_{triple_underscore}"), &cflags);

                if let Some(clang_cpp) = find_executable("clang++") {
                    let cpp_path = clang_cpp.display().to_string();
                    cmd.env(format!("CXX_{triple}"), &cpp_path);
                    cmd.env(format!("CXX_{triple_underscore}"), &cpp_path);
                    cmd.env(format!("CXXFLAGS_{triple}"), &cflags);
                    cmd.env(format!("CXXFLAGS_{triple_underscore}"), &cflags);
                }
            }
            if let Some(ar) = find_executable("llvm-ar").or_else(|| find_executable("ar")) {
                let ar_path = ar.display().to_string();
                cmd.env(format!("AR_{triple}"), &ar_path);
                cmd.env(format!("AR_{triple_underscore}"), &ar_path);
            }
        }

        if hyprlnk && self.builder.kind == BuilderKind::Cargo {
            match hyprlnk_driver::linker_for_cargo() {
                Some(linker) => {
                    cmd.env(&linker_var, linker);
                }
                None => {
                    if let Some(exe) = hyprlnk_driver::current_exe() {
                        cmd.env(&linker_var, exe);
                        cmd.env(hyprlnk_driver::LINKER_ENV, "1");
                    }
                }
            }
        }
        if let Some(cc) = driver.as_ref().filter(|c| c.as_str() != "cc") {
            cmd.env(hyprlnk_driver::CC_ENV, cc);
        } else if let Some(clang) = &fallback_clang {
            cmd.env(hyprlnk_driver::CC_ENV, clang);
        }

        let mut rustc_flags = Vec::new();
        if hyprlnk
            && self.builder.kind == BuilderKind::Cargo
            && let Some(flavor) =
                sdk_linker_flavor(self.target, maybe_sdk.is_some(), driver.is_some())
        {
            rustc_flags.push(format!("-C linker-flavor={flavor}"));
        }
        if let Some(sdk) = &maybe_sdk {
            if hyprlnk && let Some(loader) = self.target.dynamic_linker() {
                // hyprlnk's default is architecture-specific, but not libc/float-ABI-specific.
                rustc_flags.push(format!("-C link-arg=--dynamic-linker={loader}"));
            }
            if self.target.os == crate::targets::TargetOs::MacOS {
                rustc_flags.push(format!(
                    "-C link-arg=-syslibroot -C link-arg={}",
                    sdk.display()
                ));
            } else if self.target.env == crate::targets::TargetEnv::Msvc {
                let arch_candidates = match self.target.arch {
                    crate::targets::TargetArch::X86_64 => &["x86_64", "x64"][..],
                    crate::targets::TargetArch::Aarch64 => &["aarch64", "arm64"][..],
                    crate::targets::TargetArch::X86 => &["x86", "i386"][..],
                    _ => &["x86_64", "x64"][..],
                };

                let mut lib_dirs = Vec::new();
                for &arch in arch_candidates {
                    // xwin splat layout
                    let p1 = sdk.join("crt").join("lib").join(arch);
                    let p2 = sdk.join("sdk").join("lib").join("um").join(arch);
                    let p3 = sdk.join("sdk").join("lib").join("ucrt").join(arch);
                    // flat / stub layout
                    let p4 = sdk.join("lib").join(arch);
                    let p5 = sdk.join("um").join(arch);
                    let p6 = sdk.join("ucrt").join(arch);

                    for p in [p1, p2, p3, p4, p5, p6] {
                        if p.is_dir() && !lib_dirs.contains(&p) {
                            lib_dirs.push(p);
                        }
                    }
                }

                for dir in &lib_dirs {
                    rustc_flags.push(format!("-C link-arg=/LIBPATH:{}", dir.display()));
                }

                if !lib_dirs.is_empty() {
                    let joined = std::env::join_paths(&lib_dirs)
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    cmd.env("LIB", joined);
                }
            } else if self.target.os == crate::targets::TargetOs::Linux
                || self.target.os == crate::targets::TargetOs::Android
                || self.target.os == crate::targets::TargetOs::FreeBsd
            {
                rustc_flags.push(format!("-C link-arg=--sysroot={}", sdk.display()));
                let usr_lib = sdk.join("usr").join("lib");
                if usr_lib.is_dir() {
                    rustc_flags.push(format!("-C link-arg=-L{}", usr_lib.display()));
                }
                let lib_dir = sdk.join("lib");
                if lib_dir.is_dir() {
                    rustc_flags.push(format!("-C link-arg=-L{}", lib_dir.display()));
                }
                if self.target.os == crate::targets::TargetOs::Android {
                    let api21 = usr_lib.join("21");
                    if api21.is_dir() {
                        rustc_flags.push(format!("-C link-arg=-L{}", api21.display()));
                    }
                }
            } else if self.target.os == crate::targets::TargetOs::Windows {
                let usr_lib = sdk.join("usr").join("lib");
                if usr_lib.is_dir() {
                    rustc_flags.push(format!("-C link-arg=-L{}", usr_lib.display()));
                }
            }
        }

        if fallback_clang.is_some() {
            rustc_flags.push(format!("-C link-arg=--target={clang_target}"));
            rustc_flags.push("-C link-arg=-Wno-unused-command-line-argument".to_string());
            if self.target.lld_supported() {
                rustc_flags.push("-C link-arg=-fuse-ld=lld".to_string());
            } else if let Some(ld) = self
                .target
                .tool_prefix()
                .and_then(|prefix| find_executable(&format!("{prefix}-ld")))
            {
                rustc_flags.push(format!("-C link-arg=--ld-path={}", ld.display()));
                if let Some(sdk) = &maybe_sdk {
                    rustc_flags.push(format!(
                        "-C link-arg=-Wl,-rpath-link,{}",
                        sdk.join("usr/lib").display()
                    ));
                }
            } else {
                anyhow::bail!(
                    "{triple} requires GNU binutils: install {}-ld (or configure a target C compiler with CARGO_TARGET_{key}_LINKER)",
                    self.target.tool_prefix().unwrap_or_default()
                );
            }
        }
        if !rustc_flags.is_empty() {
            let flag = rustc_flags.join(" ");
            let combined = match std::env::var("RUSTFLAGS") {
                Ok(existing) if !existing.trim().is_empty() => format!("{existing} {flag}"),
                _ => flag.clone(),
            };
            cmd.env("RUSTFLAGS", combined);
            cmd.env(format!("CARGO_TARGET_{key}_RUSTFLAGS"), flag);
        }

        if matches!(subcommand, "test" | "run" | "bench") {
            let runner_var = format!("CARGO_TARGET_{key}_RUNNER");
            let tc = self.config.get_target(triple);
            let explicit_sysroot = tc.and_then(|t| t.sysroot.as_deref());
            let sysroot_path = explicit_sysroot.or(maybe_sdk.as_deref());
            if std::env::var_os(&runner_var).is_none()
                && let Some(runner) = qemu::host_runner_command(
                    self.target,
                    tc.and_then(|t| t.runner.as_deref()),
                    sysroot_path,
                )
            {
                ui::trace(format!("{runner_var}={runner}"));
                cmd.env(runner_var, runner);
            }
        }

        let tc = self.config.get_target(triple);
        for (k, v) in self
            .config
            .build
            .env
            .iter()
            .chain(tc.map(|t| t.env.iter()).into_iter().flatten())
        {
            cmd.env(k, v);
        }
        Ok(cmd)
    }

    pub fn execute(&self, subcommand: &str, extra_args: &[String]) -> Result<i32> {
        let mut cmd = self.command(subcommand, extra_args)?;
        ui::trace(format!("{cmd:?}"));
        Ok(cmd.status()?.code().unwrap_or(1))
    }
}

/// An SDK supplies the startup files itself. Without a target C compiler, invoking the
/// host's cc would select host startup objects before hyprlnk sees the target inputs.
fn sdk_linker_flavor(target: &TargetInfo, has_sdk: bool, has_driver: bool) -> Option<&'static str> {
    if has_sdk && !has_driver && target.os == crate::targets::TargetOs::Linux {
        Some("ld.lld")
    } else {
        hyprlnk_driver::rustc_linker_flavor(&target.triple)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_sdk_links_without_the_host_c_runtime() {
        let target = TargetInfo::parse("aarch64-unknown-linux-gnu");
        assert_eq!(sdk_linker_flavor(&target, true, false), Some("ld.lld"));
        assert_eq!(
            sdk_linker_flavor(&target, true, true),
            hyprlnk_driver::rustc_linker_flavor(&target.triple)
        );
        assert_eq!(
            sdk_linker_flavor(&target, false, false),
            hyprlnk_driver::rustc_linker_flavor(&target.triple)
        );
        let apple = TargetInfo::parse("aarch64-apple-darwin");
        assert_eq!(sdk_linker_flavor(&apple, true, false), Some("ld64.lld"));
        let windows = TargetInfo::parse("aarch64-pc-windows-msvc");
        assert_eq!(sdk_linker_flavor(&windows, true, false), None);
    }

    #[test]
    fn env_keys() {
        assert_eq!(
            target_env_key("aarch64-unknown-linux-gnu"),
            "AARCH64_UNKNOWN_LINUX_GNU"
        );
        assert_eq!(
            target_env_key("thumbv8m.main-none-eabi"),
            "THUMBV8M_MAIN_NONE_EABI"
        );
    }

    #[test]
    fn igel_gets_no_linker_flags_from_hyprcross() {
        let target = TargetInfo::parse(crate::targets::current_host_triple());
        let builder = Builder {
            kind: BuilderKind::Igel,
            path: "igl".into(),
        };
        let config = Config::default();
        let engine = DirectEngine::new(&target, &config, &builder, true);
        let cmd = engine.command("build", &[]).unwrap();
        let envs: Vec<_> = cmd
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(
            !envs
                .iter()
                .any(|k| k == "RUSTFLAGS" || k.ends_with("_RUSTFLAGS") || k.ends_with("_LINKER")),
            "Igel links with its own hyprlnk and keeps its cache: {envs:?}"
        );
    }
}
