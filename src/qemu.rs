use crate::builder::find_executable;
#[allow(unused_imports)]
use crate::targets::{TargetArch, TargetInfo, TargetOs};
use std::path::{Path, PathBuf};

pub struct QemuRunner {
    pub binary: PathBuf,
    pub ld_prefix: Option<PathBuf>,
}

impl QemuRunner {
    /// Detect or resolve QEMU runner for a given target
    pub fn resolve(target: &TargetInfo, custom_runner: Option<&str>, sysroot: Option<&Path>) -> Option<Self> {
        // If user configured an explicit runner in config, prioritize that
        if let Some(r) = custom_runner
            && let Some(path) = find_executable(r) {
                return Some(Self {
                    binary: path,
                    ld_prefix: sysroot.map(Path::to_path_buf),
                });
            }

        // QEMU user emulation is primarily used for Linux cross targets
        if target.os != TargetOs::Linux {
            return None;
        }

        // Look for static emulator first (qemu-aarch64-static), then dynamic (qemu-aarch64)
        if let Some(binary_name) = target.qemu_binary {
            if let Some(path) = find_executable(binary_name) {
                let ld_prefix = resolve_sysroot_prefix(target, sysroot);
                return Some(Self {
                    binary: path,
                    ld_prefix,
                });
            }

            // Try without `-static` suffix
            let dynamic_name = binary_name.trim_end_matches("-static");
            if let Some(path) = find_executable(dynamic_name) {
                let ld_prefix = resolve_sysroot_prefix(target, sysroot);
                return Some(Self {
                    binary: path,
                    ld_prefix,
                });
            }
        }

        None
    }

    /// Formulate the command string or runner invocation
    pub fn runner_command(&self) -> String {
        let bin_str = self.binary.to_string_lossy();
        if let Some(prefix) = &self.ld_prefix {
            format!("{bin_str} -L {}", prefix.to_string_lossy())
        } else {
            bin_str.to_string()
        }
    }
}

/// Check if binfmt_misc kernel module is active and registered for the target architecture
#[allow(unused_variables)]
pub fn is_binfmt_registered(target: &TargetInfo) -> bool {
    #[cfg(target_os = "linux")]
    {
        let binfmt_dir = Path::new("/proc/sys/fs/binfmt_misc");
        if !binfmt_dir.is_dir() {
            return false;
        }

        let pattern = match target.arch {
            TargetArch::Aarch64 => "qemu-aarch64",
            TargetArch::Armv7 | TargetArch::Arm => "qemu-arm",
            TargetArch::Riscv64 => "qemu-riscv64",
            TargetArch::Powerpc64le => "qemu-ppc64le",
            TargetArch::S390x => "qemu-s390x",
            _ => return false,
        };

        binfmt_dir.join(pattern).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn resolve_sysroot_prefix(target: &TargetInfo, custom_sysroot: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = custom_sysroot
        && p.exists() {
            return Some(p.to_path_buf());
        }

    // Standard Debian / Ubuntu / Fedora cross sysroot paths
    let standard_paths = [
        format!("/usr/{}", target.triple),
        format!("/usr/lib/{}", target.triple),
    ];

    for p_str in standard_paths {
        let p = Path::new(&p_str);
        if p.is_dir() {
            return Some(p.to_path_buf());
        }
    }

    None
}
