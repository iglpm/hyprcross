use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetArch {
    X86_64,
    X86,
    Aarch64,
    Armv7,
    Arm,
    Riscv64,
    Powerpc64le,
    S390x,
    Wasm32,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOs {
    Linux,
    Windows,
    MacOS,
    None, // Bare metal / WASM
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

#[derive(Debug, Clone)]
pub struct TargetInfo {
    pub triple: String,
    pub arch: TargetArch,
    pub os: TargetOs,
    pub env: TargetEnv,
    pub default_image: String,
    pub qemu_binary: Option<&'static str>,
    pub hyprlnk_supported: bool,
    pub direct_supported: bool,
}

impl TargetInfo {
    pub fn parse(triple: &str) -> Self {
        let parts: Vec<&str> = triple.split('-').collect();

        let arch = match parts.first().copied().unwrap_or("") {
            "x86_64" => TargetArch::X86_64,
            "i686" | "i586" => TargetArch::X86,
            "aarch64" => TargetArch::Aarch64,
            "armv7" | "armv7hf" => TargetArch::Armv7,
            "arm" | "armv5te" | "armv6" => TargetArch::Arm,
            "riscv64" | "riscv64gc" => TargetArch::Riscv64,
            "powerpc64le" | "ppc64le" => TargetArch::Powerpc64le,
            "s390x" => TargetArch::S390x,
            "wasm32" => TargetArch::Wasm32,
            other => TargetArch::Other(other.to_string()),
        };

        let os = if triple.contains("-linux-") {
            TargetOs::Linux
        } else if triple.contains("-windows-") {
            TargetOs::Windows
        } else if triple.contains("-apple-darwin") {
            TargetOs::MacOS
        } else if triple.contains("-none-") || triple.starts_with("wasm") {
            TargetOs::None
        } else {
            TargetOs::Other(triple.to_string())
        };

        let env = if triple.ends_with("musl") || triple.contains("-musl") {
            TargetEnv::Musl
        } else if triple.ends_with("gnu") || triple.contains("-gnu") {
            TargetEnv::Gnu
        } else if triple.ends_with("msvc") {
            TargetEnv::Msvc
        } else {
            TargetEnv::None
        };

        let qemu_binary = match (&os, &arch) {
            (TargetOs::Linux, TargetArch::Aarch64) => Some("qemu-aarch64-static"),
            (TargetOs::Linux, TargetArch::Armv7) | (TargetOs::Linux, TargetArch::Arm) => {
                Some("qemu-arm-static")
            }
            (TargetOs::Linux, TargetArch::Riscv64) => Some("qemu-riscv64-static"),
            (TargetOs::Linux, TargetArch::Powerpc64le) => Some("qemu-ppc64le-static"),
            (TargetOs::Linux, TargetArch::S390x) => Some("qemu-s390x-static"),
            (TargetOs::Linux, TargetArch::X86) => Some("qemu-i386-static"),
            (TargetOs::Linux, TargetArch::X86_64) => Some("qemu-x86_64-static"),
            _ => None,
        };

        let default_image = format!("ghcr.io/cross-rs/{triple}:edge");

        #[cfg(feature = "builtin-hyprlnk")]
        let hyprlnk_supported = hyprlnk::supports_target(triple);
        #[cfg(not(feature = "builtin-hyprlnk"))]
        let hyprlnk_supported = match os {
            TargetOs::Linux => matches!(
                arch,
                TargetArch::X86_64
                    | TargetArch::X86
                    | TargetArch::Aarch64
                    | TargetArch::Armv7
                    | TargetArch::Arm
                    | TargetArch::Riscv64
                    | TargetArch::Powerpc64le
                    | TargetArch::S390x
            ),
            TargetOs::Windows => matches!(env, TargetEnv::Msvc) || matches!(env, TargetEnv::Gnu),
            TargetOs::MacOS => true,
            _ => false,
        };

        // Direct mode can build without a container if it's musl, bare-metal, wasm, or native
        let direct_supported = matches!(env, TargetEnv::Musl)
            || matches!(os, TargetOs::None)
            || is_host_compatible(triple);

        Self {
            triple: triple.to_string(),
            arch,
            os,
            env,
            default_image,
            qemu_binary,
            hyprlnk_supported,
            direct_supported,
        }
    }

    /// Check if this target has an official pre-built container image in ghcr.io/cross-rs
    pub fn has_prebuilt_image(&self) -> bool {
        // MSVC and Apple Darwin cannot be distributed in public container images due to proprietary licenses
        if matches!(self.env, TargetEnv::Msvc) || matches!(self.os, TargetOs::MacOS) {
            return false;
        }
        // Linux ELF targets and Windows GNU targets have official public images
        matches!(self.os, TargetOs::Linux)
            || (matches!(self.os, TargetOs::Windows) && matches!(self.env, TargetEnv::Gnu))
    }

    /// Suggested open-source container-compatible target alternative if available
    pub fn suggested_alternative(&self) -> Option<String> {
        if matches!(self.env, TargetEnv::Msvc) {
            match self.arch {
                TargetArch::X86_64 => Some("x86_64-pc-windows-gnu".to_string()),
                TargetArch::X86 => Some("i686-pc-windows-gnu".to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    /// The target C compiler name inside the cross container (used for HYPRLNK_CC)
    pub fn target_c_compiler(&self) -> Option<&'static str> {
        match self.triple.as_str() {
            "aarch64-unknown-linux-gnu" => Some("aarch64-linux-gnu-gcc"),
            "armv7-unknown-linux-gnueabihf" => Some("arm-linux-gnueabihf-gcc"),
            "arm-unknown-linux-gnueabi" => Some("arm-linux-gnueabi-gcc"),
            "riscv64gc-unknown-linux-gnu" => Some("riscv64-linux-gnu-gcc"),
            "powerpc64le-unknown-linux-gnu" => Some("powerpc64le-linux-gnu-gcc"),
            "s390x-unknown-linux-gnu" => Some("s390x-linux-gnu-gcc"),
            "i686-unknown-linux-gnu" => Some("i686-linux-gnu-gcc"),
            "x86_64-unknown-linux-gnu" => Some("x86_64-linux-gnu-gcc"),
            "x86_64-pc-windows-gnu" => Some("x86_64-w64-mingw32-gcc"),
            "i686-pc-windows-gnu" => Some("i686-w64-mingw32-gcc"),
            _ => None,
        }
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

pub fn is_host_compatible(triple: &str) -> bool {
    let host = current_host_triple();
    host == triple
}

pub fn current_host_triple() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        "unknown"
    }
}
