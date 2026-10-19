//! Target triples: what they are, which toolchain builds them, which image has it and what runs
//! their programs.

use std::fmt;

/// The targets hyprcross publishes images for (`docker/Dockerfile.<target>`), at
/// `<image_repository>/<target>:<image_tag>`.
pub const IMAGE_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "i686-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
    "arm-unknown-linux-gnueabi",
    "riscv64gc-unknown-linux-gnu",
    "powerpc64le-unknown-linux-gnu",
    "powerpc64-unknown-linux-gnu",
    "powerpc-unknown-linux-gnu",
    "s390x-unknown-linux-gnu",
    "sparc64-unknown-linux-gnu",
    "loongarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "i686-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "armv7-unknown-linux-musleabihf",
    "arm-unknown-linux-musleabihf",
    "riscv64gc-unknown-linux-musl",
    "powerpc64le-unknown-linux-musl",
    "x86_64-pc-windows-gnu",
    "i686-pc-windows-gnu",
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "x86_64-linux-android",
    "i686-linux-android",
    "x86_64-unknown-freebsd",
    "wasm32-wasip1",
    "wasm32-wasip2",
    "thumbv6m-none-eabi",
    "thumbv7em-none-eabihf",
];

/// All first-class targets supported by hyprcross (both via container images and direct host execution).
pub const ALL_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "i686-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
    "arm-unknown-linux-gnueabi",
    "riscv64gc-unknown-linux-gnu",
    "powerpc64le-unknown-linux-gnu",
    "powerpc64-unknown-linux-gnu",
    "powerpc-unknown-linux-gnu",
    "s390x-unknown-linux-gnu",
    "sparc64-unknown-linux-gnu",
    "loongarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "i686-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "armv7-unknown-linux-musleabihf",
    "arm-unknown-linux-musleabihf",
    "riscv64gc-unknown-linux-musl",
    "powerpc64le-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "i686-pc-windows-msvc",
    "x86_64-pc-windows-gnu",
    "i686-pc-windows-gnu",
    "aarch64-pc-windows-gnullvm",
    "x86_64-pc-windows-gnullvm",
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "x86_64-linux-android",
    "i686-linux-android",
    "x86_64-unknown-freebsd",
    "wasm32-wasip1",
    "wasm32-wasip2",
    "wasm32-unknown-unknown",
    "thumbv6m-none-eabi",
    "thumbv7em-none-eabihf",
    "riscv32imc-unknown-none-elf",
];

pub const DEFAULT_IMAGE_REPOSITORY: &str = "ghcr.io/runepm/hyprcross";
pub const DEFAULT_IMAGE_TAG: &str = "latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetArch {
    X86_64,
    X86,
    Aarch64,
    Armv7,
    Arm,
    Riscv64,
    Powerpc64le,
    Powerpc64,
    Powerpc,
    S390x,
    Sparc64,
    Loongarch64,
    Wasm32,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOs {
    Linux,
    Android,
    Windows,
    MacOS,
    FreeBsd,
    Wasi,
    /// Bare metal and `wasm32-unknown-unknown`.
    None,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetEnv {
    Gnu,
    Musl,
    Msvc,
    None,
    Other(String),
}

/// What runs the target's programs for `test` and `run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    Native,
    Qemu(&'static str),
    Wine,
    Wasmtime,
    None,
}

impl fmt::Display for Runner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Runner::Native => write!(f, "native"),
            Runner::Qemu(arch) => write!(f, "qemu-{arch}"),
            Runner::Wine => write!(f, "wine"),
            Runner::Wasmtime => write!(f, "wasmtime"),
            Runner::None => write!(f, "none"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetInfo {
    pub triple: String,
    pub arch: TargetArch,
    pub os: TargetOs,
    pub env: TargetEnv,
    /// The QEMU user-mode emulator for Linux targets, as distributions name it.
    pub qemu_binary: Option<&'static str>,
    /// Whether hyprlnk links this target.
    pub hyprlnk_supported: bool,
}

impl TargetInfo {
    pub fn parse(triple: &str) -> Self {
        let first = triple.split('-').next().unwrap_or("");
        let arch = match first {
            "x86_64" | "x86_64h" => TargetArch::X86_64,
            "i686" | "i586" | "i386" => TargetArch::X86,
            "aarch64" | "arm64" | "arm64e" => TargetArch::Aarch64,
            a if a.starts_with("armv7") || a.starts_with("thumbv7neon") => TargetArch::Armv7,
            a if a == "arm" || a.starts_with("armv") || a.starts_with("thumbv") => TargetArch::Arm,
            "riscv64gc" | "riscv64" => TargetArch::Riscv64,
            "powerpc64le" | "ppc64le" => TargetArch::Powerpc64le,
            "powerpc64" => TargetArch::Powerpc64,
            "powerpc" => TargetArch::Powerpc,
            "s390x" => TargetArch::S390x,
            "sparc64" => TargetArch::Sparc64,
            "loongarch64" => TargetArch::Loongarch64,
            "wasm32" => TargetArch::Wasm32,
            other => TargetArch::Other(other.to_string()),
        };

        let os = if triple.contains("-android") {
            TargetOs::Android
        } else if triple.contains("-linux-") || triple.ends_with("-linux") {
            TargetOs::Linux
        } else if triple.contains("-windows") {
            TargetOs::Windows
        } else if triple.contains("-apple-darwin") {
            TargetOs::MacOS
        } else if triple.contains("-freebsd") {
            TargetOs::FreeBsd
        } else if triple.contains("-wasi") {
            TargetOs::Wasi
        } else if triple.contains("-none") || triple.starts_with("wasm") {
            TargetOs::None
        } else {
            TargetOs::Other(triple.to_string())
        };

        let last = triple.rsplit('-').next().unwrap_or("");
        let env = if last.starts_with("musl") {
            TargetEnv::Musl
        } else if last.starts_with("gnu") {
            TargetEnv::Gnu
        } else if last == "msvc" {
            TargetEnv::Msvc
        } else if matches!(os, TargetOs::None | TargetOs::Wasi) || last == "freebsd" {
            TargetEnv::None
        } else {
            TargetEnv::Other(last.to_string())
        };

        let qemu_binary = match (&os, &arch) {
            (TargetOs::Linux, arch) => qemu_arch(arch).map(|a| match a {
                "x86_64" => "qemu-x86_64-static",
                "i386" => "qemu-i386-static",
                "aarch64" => "qemu-aarch64-static",
                "arm" => "qemu-arm-static",
                "riscv64" => "qemu-riscv64-static",
                "ppc64le" => "qemu-ppc64le-static",
                "ppc64" => "qemu-ppc64-static",
                "ppc" => "qemu-ppc-static",
                "s390x" => "qemu-s390x-static",
                "sparc64" => "qemu-sparc64-static",
                _ => "qemu-loongarch64-static",
            }),
            _ => None,
        };

        #[cfg(feature = "builtin-hyprlnk")]
        let hyprlnk_supported = hyprlnk::supports_target(triple);
        #[cfg(not(feature = "builtin-hyprlnk"))]
        let hyprlnk_supported = match os {
            TargetOs::Linux => {
                matches!(env, TargetEnv::Gnu | TargetEnv::Musl)
                    && matches!(
                        arch,
                        TargetArch::X86_64
                            | TargetArch::X86
                            | TargetArch::Aarch64
                            | TargetArch::Armv7
                            | TargetArch::Arm
                            | TargetArch::Riscv64
                            | TargetArch::Powerpc64le
                            | TargetArch::S390x
                            | TargetArch::Loongarch64
                    )
            }
            TargetOs::Windows => matches!(env, TargetEnv::Msvc),
            TargetOs::MacOS => true,
            _ => false,
        };

        Self {
            triple: triple.to_string(),
            arch,
            os,
            env,
            qemu_binary,
            hyprlnk_supported,
        }
    }

    pub fn is_host(&self) -> bool {
        self.triple == current_host_triple()
    }

    /// Whether hyprcross publishes an image for this target.
    pub fn has_hyprcross_image(&self) -> bool {
        IMAGE_TARGETS.contains(&self.triple.as_str())
    }

    /// This target's image in `repository`, if hyprcross publishes one.
    pub fn image(&self, repository: &str, tag: &str) -> Option<String> {
        self.has_hyprcross_image()
            .then(|| format!("{}/{}:{tag}", repository.trim_end_matches('/'), self.triple))
    }

    /// cross-rs' image for this target: where hyprcross falls back to for targets it has no image
    /// of its own for, or when its own can't be pulled.
    pub fn cross_rs_image(&self) -> Option<String> {
        let published = match self.os {
            TargetOs::Linux | TargetOs::Android | TargetOs::FreeBsd => true,
            TargetOs::Windows => self.env == TargetEnv::Gnu,
            TargetOs::None => self.triple.starts_with("thumb"),
            _ => false,
        };
        published.then(|| format!("ghcr.io/cross-rs/{}:edge", self.triple))
    }

    pub fn has_prebuilt_image(&self) -> bool {
        self.has_hyprcross_image() || self.cross_rs_image().is_some()
    }

    /// The open target to use in a container instead, for one whose SDK can't be redistributed.
    pub fn suggested_alternative(&self) -> Option<String> {
        if self.env != TargetEnv::Msvc {
            return None;
        }
        match self.arch {
            TargetArch::X86_64 => Some("x86_64-pc-windows-gnu".to_string()),
            TargetArch::X86 => Some("i686-pc-windows-gnu".to_string()),
            _ => None,
        }
    }

    /// The GNU-style prefix of this target's C toolchain (`aarch64-linux-gnu`, `x86_64-w64-mingw32`).
    pub fn tool_prefix(&self) -> Option<String> {
        let arch = self.triple.split('-').next()?;
        match (&self.os, &self.env) {
            (TargetOs::Linux, TargetEnv::Gnu | TargetEnv::Musl) => {
                let env = self.triple.rsplit('-').next()?;
                let arch = match arch {
                    "riscv64gc" => "riscv64",
                    a if a.starts_with("armv") || a.starts_with("thumbv") => "arm",
                    "i586" => "i686",
                    a => a,
                };
                Some(format!("{arch}-linux-{env}"))
            }
            (TargetOs::Windows, TargetEnv::Gnu) => {
                let arch = if self.arch == TargetArch::X86 {
                    "i686"
                } else {
                    arch
                };
                Some(format!("{arch}-w64-mingw32"))
            }
            _ => None,
        }
    }

    /// The C compilers this target's build scripts and hyprlnk can use, most specific first.
    pub fn c_compiler_candidates(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.is_host() {
            names.push("cc".to_string());
        }
        if let Some(prefix) = self.tool_prefix() {
            names.push(format!("{prefix}-gcc"));
            if self.os == TargetOs::Windows {
                names.push(format!("{prefix}-gcc-posix"));
            }
            if let TargetArch::Armv7 = self.arch {
                let env = self.triple.rsplit('-').next().unwrap_or("");
                names.push(format!("armv7-linux-{env}-gcc"));
            }
        }
        if self.env == TargetEnv::Musl && self.arch_matches_host() {
            names.push("musl-gcc".to_string());
        }
        names
    }

    /// The C compiler the image's toolchain is known by (`HYPRLNK_CC` for the cargo builder).
    pub fn target_c_compiler(&self) -> Option<&'static str> {
        match self.triple.as_str() {
            "aarch64-unknown-linux-gnu" => Some("aarch64-linux-gnu-gcc"),
            "armv7-unknown-linux-gnueabihf" => Some("arm-linux-gnueabihf-gcc"),
            "arm-unknown-linux-gnueabi" => Some("arm-linux-gnueabi-gcc"),
            "arm-unknown-linux-gnueabihf" => Some("arm-linux-gnueabihf-gcc"),
            "riscv64gc-unknown-linux-gnu" => Some("riscv64-linux-gnu-gcc"),
            "powerpc64le-unknown-linux-gnu" => Some("powerpc64le-linux-gnu-gcc"),
            "s390x-unknown-linux-gnu" => Some("s390x-linux-gnu-gcc"),
            "loongarch64-unknown-linux-gnu" => Some("loongarch64-linux-gnu-gcc"),
            "i686-unknown-linux-gnu" => Some("i686-linux-gnu-gcc"),
            "x86_64-unknown-linux-gnu" => Some("x86_64-linux-gnu-gcc"),
            "x86_64-unknown-linux-musl" => Some("x86_64-linux-musl-gcc"),
            "i686-unknown-linux-musl" => Some("i686-linux-musl-gcc"),
            "aarch64-unknown-linux-musl" => Some("aarch64-linux-musl-gcc"),
            "armv7-unknown-linux-musleabihf" | "arm-unknown-linux-musleabihf" => {
                Some("arm-linux-musleabihf-gcc")
            }
            "riscv64gc-unknown-linux-musl" => Some("riscv64-linux-musl-gcc"),
            "powerpc64le-unknown-linux-musl" => Some("powerpc64le-linux-musl-gcc"),
            "x86_64-pc-windows-gnu" => Some("x86_64-w64-mingw32-gcc"),
            "i686-pc-windows-gnu" => Some("i686-w64-mingw32-gcc"),
            _ => None,
        }
    }

    /// Whether this target only needs Rust's own toolchain (rust-lld) to build pure-Rust crates.
    pub fn links_without_c_toolchain(&self) -> bool {
        matches!(self.os, TargetOs::None | TargetOs::Wasi)
    }

    /// What runs this target's programs, here on the host.
    pub fn host_runner(&self) -> Runner {
        if self.is_host() || (self.env == TargetEnv::Musl && self.arch_matches_host()) {
            return Runner::Native;
        }
        match self.os {
            TargetOs::Linux => qemu_arch(&self.arch).map_or(Runner::None, Runner::Qemu),
            TargetOs::Windows if self.env == TargetEnv::Gnu => Runner::Wine,
            TargetOs::Wasi => Runner::Wasmtime,
            _ => Runner::None,
        }
    }

    /// What runs this target's programs in its hyprcross image.
    pub fn image_runner(&self) -> Runner {
        if !self.has_hyprcross_image() {
            return Runner::None;
        }
        match self.os {
            TargetOs::Linux if self.arch == TargetArch::X86_64 => Runner::Native,
            TargetOs::Linux => qemu_arch(&self.arch).map_or(Runner::None, Runner::Qemu),
            TargetOs::Windows => Runner::Wine,
            TargetOs::Wasi => Runner::Wasmtime,
            _ => Runner::None,
        }
    }

    pub fn arch_matches_host(&self) -> bool {
        let host = current_host_triple();
        host.split('-').next() == self.triple.split('-').next()
    }

    pub fn matches_query(&self, query: &[String]) -> bool {
        if query.is_empty() {
            return true;
        }
        let terms: Vec<String> = query
            .iter()
            .flat_map(|q| q.split_whitespace())
            .map(|t| t.to_ascii_lowercase())
            .collect();
        if terms.is_empty() {
            return true;
        }
        terms.iter().all(|term| self.matches_term(term))
    }

    fn matches_term(&self, term: &str) -> bool {
        let triple = self.triple.to_ascii_lowercase();
        if triple.contains(term) {
            return true;
        }

        // Match OS aliases
        match (&self.os, term) {
            (TargetOs::Windows, "win" | "windows") => return true,
            (TargetOs::Linux, "linux") => return true,
            (TargetOs::MacOS, "mac" | "macos" | "darwin" | "apple" | "osx") => return true,
            (TargetOs::Android, "android") => return true,
            (TargetOs::FreeBsd, "bsd" | "freebsd") => return true,
            (TargetOs::Wasi, "wasi") => return true,
            (TargetOs::None, "none" | "embedded" | "baremetal" | "bare-metal") => return true,
            _ => {}
        }

        // Match Arch aliases
        match (&self.arch, term) {
            (TargetArch::Aarch64, "arm" | "arm64" | "aarch64") => return true,
            (TargetArch::Armv7 | TargetArch::Arm, "arm" | "arm32" | "armv7") => return true,
            (TargetArch::X86_64, "x86_64" | "x64" | "amd64" | "intel") => return true,
            (TargetArch::X86, "x86" | "i686" | "32bit" | "32-bit") => return true,
            (TargetArch::Riscv64, "riscv" | "riscv64") => return true,
            (TargetArch::Wasm32, "wasm" | "wasm32" | "webassembly") => return true,
            _ => {}
        }

        // Match Env aliases
        match (&self.env, term) {
            (TargetEnv::Gnu, "gnu" | "glibc") => return true,
            (TargetEnv::Musl, "musl" | "static") => return true,
            (TargetEnv::Msvc, "msvc" | "cl") => return true,
            _ => {}
        }

        false
    }

    pub fn display_name(&self) -> &str {
        &self.triple
    }
}

impl fmt::Display for TargetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.triple)
    }
}

/// QEMU's name for an architecture (`qemu-<name>`).
fn qemu_arch(arch: &TargetArch) -> Option<&'static str> {
    Some(match arch {
        TargetArch::X86_64 => "x86_64",
        TargetArch::X86 => "i386",
        TargetArch::Aarch64 => "aarch64",
        TargetArch::Armv7 | TargetArch::Arm => "arm",
        TargetArch::Riscv64 => "riscv64",
        TargetArch::Powerpc64le => "ppc64le",
        TargetArch::Powerpc64 => "ppc64",
        TargetArch::Powerpc => "ppc",
        TargetArch::S390x => "s390x",
        TargetArch::Sparc64 => "sparc64",
        TargetArch::Loongarch64 => "loongarch64",
        TargetArch::Wasm32 | TargetArch::Other(_) => return None,
    })
}

pub fn is_host_compatible(triple: &str) -> bool {
    current_host_triple() == triple
}

/// The triple rustc builds natively on this machine. A musl build of hyprcross still reports
/// glibc on Linux: that is what rustup installs as the host toolchain.
pub fn current_host_triple() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "riscv64")) {
        "riscv64gc-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "powerpc64")) {
        "powerpc64le-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "s390x")) {
        "s390x-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "loongarch64")) {
        "loongarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "aarch64-pc-windows-msvc"
    } else if cfg!(all(target_os = "freebsd", target_arch = "x86_64")) {
        "x86_64-unknown-freebsd"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_targets_are_unique_and_parse() {
        let mut seen = std::collections::HashSet::new();
        for t in IMAGE_TARGETS {
            assert!(seen.insert(t), "{t} listed twice");
            let info = TargetInfo::parse(t);
            assert!(info.has_hyprcross_image());
            assert!(
                !matches!(info.os, TargetOs::Other(_)),
                "{t}: unknown operating system"
            );
        }
    }

    #[test]
    fn tool_prefixes_follow_gnu_naming() {
        let p = |t: &str| TargetInfo::parse(t).tool_prefix();
        assert_eq!(
            p("aarch64-unknown-linux-gnu").as_deref(),
            Some("aarch64-linux-gnu")
        );
        assert_eq!(
            p("armv7-unknown-linux-gnueabihf").as_deref(),
            Some("arm-linux-gnueabihf")
        );
        assert_eq!(
            p("riscv64gc-unknown-linux-musl").as_deref(),
            Some("riscv64-linux-musl")
        );
        assert_eq!(
            p("i686-pc-windows-gnu").as_deref(),
            Some("i686-w64-mingw32")
        );
        assert_eq!(p("aarch64-linux-android"), None);
        assert_eq!(p("aarch64-apple-darwin"), None);
    }

    #[test]
    fn images_come_from_the_repository_given() {
        let t = TargetInfo::parse("aarch64-unknown-linux-gnu");
        assert_eq!(
            t.image("ghcr.io/me/imgs/", "v1").as_deref(),
            Some("ghcr.io/me/imgs/aarch64-unknown-linux-gnu:v1")
        );
        let none = TargetInfo::parse("mips-unknown-linux-gnu");
        assert_eq!(
            none.image(DEFAULT_IMAGE_REPOSITORY, DEFAULT_IMAGE_TAG),
            None
        );
        assert!(none.cross_rs_image().is_some(), "cross-rs is the fallback");
    }

    #[test]
    fn runners() {
        assert_eq!(
            TargetInfo::parse("aarch64-unknown-linux-gnu").image_runner(),
            Runner::Qemu("aarch64")
        );
        assert_eq!(
            TargetInfo::parse("x86_64-pc-windows-gnu").image_runner(),
            Runner::Wine
        );
        assert_eq!(
            TargetInfo::parse("wasm32-wasip1").image_runner(),
            Runner::Wasmtime
        );
        assert_eq!(
            TargetInfo::parse("aarch64-linux-android").image_runner(),
            Runner::None
        );
    }
}
