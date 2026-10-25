//! Running target programs on the host for `test` and `run` in direct mode: QEMU user-mode
//! emulation for Linux targets, Wine for Windows, Wasmtime for WASI.

use crate::builder::find_executable;
use crate::targets::{Runner, TargetInfo, TargetOs};
use std::path::{Path, PathBuf};

pub struct QemuRunner {
    pub binary: PathBuf,
    pub ld_prefix: Option<PathBuf>,
}

impl QemuRunner {
    /// The configured runner, else QEMU for Linux targets (`qemu-<arch>-static`, then
    /// `qemu-<arch>`), with the target's sysroot for its dynamic loader.
    pub fn resolve(
        target: &TargetInfo,
        custom_runner: Option<&str>,
        sysroot: Option<&Path>,
    ) -> Option<Self> {
        if let Some(r) = custom_runner
            && let Some(path) = find_executable(r)
        {
            return Some(Self {
                binary: path,
                ld_prefix: sysroot.map(Path::to_path_buf),
            });
        }
        if target.os != TargetOs::Linux {
            return None;
        }
        let name = target.qemu_binary?;
        let binary =
            find_executable(name).or_else(|| find_executable(name.trim_end_matches("-static")))?;
        Some(Self {
            binary,
            ld_prefix: resolve_sysroot_prefix(target, sysroot),
        })
    }

    pub fn runner_command(&self) -> String {
        match &self.ld_prefix {
            Some(prefix) => format!(
                "{} -L {} -E LD_LIBRARY_PATH={}/lib:{}/usr/lib:{}/lib64:{}/usr/lib64",
                self.binary.display(),
                prefix.display(),
                prefix.display(),
                prefix.display(),
                prefix.display(),
                prefix.display()
            ),
            None => self.binary.display().to_string(),
        }
    }
}

/// What to set as `CARGO_TARGET_<TRIPLE>_RUNNER` for direct builds, if anything runs the
/// target's programs here.
pub fn host_runner_command(
    target: &TargetInfo,
    custom: Option<&str>,
    sysroot: Option<&Path>,
) -> Option<String> {
    if let Some(r) = custom {
        return Some(r.to_string());
    }
    match target.host_runner() {
        Runner::Qemu(_) => QemuRunner::resolve(target, None, sysroot).map(|q| q.runner_command()),
        Runner::Wine => find_executable("wine").map(|p| p.display().to_string()),
        Runner::Wasmtime => {
            find_executable("wasmtime").map(|p| format!("{} run --dir=.", p.display()))
        }
        Runner::Native | Runner::None => None,
    }
}

/// Whether the kernel runs this architecture's binaries through QEMU by itself.
#[allow(unused_variables)]
pub fn is_binfmt_registered(target: &TargetInfo) -> bool {
    #[cfg(target_os = "linux")]
    {
        use crate::targets::TargetArch;
        let dir = Path::new("/proc/sys/fs/binfmt_misc");
        let names: &[&str] = match target.arch {
            TargetArch::Aarch64 => &["qemu-aarch64"],
            TargetArch::Armv7 | TargetArch::Arm => &["qemu-arm"],
            TargetArch::Riscv64 => &["qemu-riscv64"],
            TargetArch::Powerpc64le => &["qemu-ppc64le"],
            TargetArch::Powerpc64 => &["qemu-ppc64"],
            TargetArch::Powerpc => &["qemu-ppc"],
            TargetArch::S390x => &["qemu-s390x"],
            TargetArch::Sparc64 => &["qemu-sparc64"],
            TargetArch::Loongarch64 => &["qemu-loongarch64"],
            TargetArch::X86 => &["qemu-i386"],
            _ => return false,
        };
        names.iter().any(|n| dir.join(n).exists())
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// The target's root for QEMU's `-L`: the configured one, else where distributions' cross
/// toolchains put it (`/usr/aarch64-linux-gnu`; Arch adds `/libc`).
fn resolve_sysroot_prefix(target: &TargetInfo, custom: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = custom
        && p.exists()
    {
        // Published SDKs flatten cross-distribution /usr/<triple>/lib into usr/lib.
        // Older packages have an empty top-level lib; use usr as QEMU's prefix only
        // when it actually contains the interpreter this target will request.
        if let Some(loader) = target.dynamic_linker() {
            let loader = loader.trim_start_matches('/');
            if !p.join(loader).is_file() && p.join("usr").join(loader).is_file() {
                return Some(p.join("usr"));
            }
        }
        return Some(p.to_path_buf());
    }
    let mut candidates = Vec::new();
    if let Some(prefix) = target.tool_prefix() {
        candidates.push(PathBuf::from(format!("/usr/{prefix}/libc")));
        candidates.push(PathBuf::from(format!("/usr/{prefix}")));
    }
    candidates.push(PathBuf::from(format!("/usr/{}", target.triple)));
    candidates.into_iter().find(|p| p.join("lib").is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattened_sdk_uses_the_prefix_containing_its_loader() {
        let dir = tempfile::tempdir().unwrap();
        let sdk = dir.path();
        std::fs::create_dir_all(sdk.join("usr/lib")).unwrap();
        std::fs::create_dir(sdk.join("lib")).unwrap();
        std::fs::write(sdk.join("usr/lib/ld-linux.so.3"), b"loader").unwrap();
        let target = TargetInfo::parse("arm-unknown-linux-gnueabi");
        assert_eq!(
            resolve_sysroot_prefix(&target, Some(sdk)),
            Some(sdk.join("usr"))
        );
        std::fs::write(sdk.join("lib/ld-linux.so.3"), b"loader").unwrap();
        assert_eq!(
            resolve_sysroot_prefix(&target, Some(sdk)),
            Some(sdk.to_path_buf())
        );
    }
}
