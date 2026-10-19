//! Target SDK and Sysroot Manager: native cross-compilation SDK bootstrapping,
//! sysroot configuration, and linking via hyprlnk without Docker or virtualization.

use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::builder::find_executable;
use crate::targets::{TargetEnv, TargetInfo, TargetOs};
use crate::ui;

pub const DEFAULT_SDK_REPO: &str = "https://github.com/runepm/sdks/releases/download/v2027.0.0";
pub const FALLBACK_SDK_REPO: &str = "https://github.com/runepm/sdks/releases/download/v1.0.0";

pub const MACOS_SDK_PRIMARY_URL: &str =
    "https://github.com/runepm/sdks/releases/download/v2027.0.0/MacOSX.sdk.tar.xz";
pub const MACOS_SDK_FALLBACK_URL: &str =
    "https://github.com/joseluisq/macosx-sdks/releases/download/26.1/MacOSX26.1.sdk.tar.xz";

#[derive(Debug, Clone)]
pub struct TargetSdkDef {
    pub triple: &'static str,
    pub dir_name: &'static str,
    pub display_name: &'static str,
    pub approx_size: &'static str,
    pub primary_url: Option<&'static str>,
    pub fallback_url: Option<&'static str>,
}

/// Known SDK definitions for cross-compilation targets.
pub const KNOWN_SDKS: &[TargetSdkDef] = &[
    // Apple macOS targets (shared SDK)
    TargetSdkDef {
        triple: "aarch64-apple-darwin",
        dir_name: "MacOSX.sdk",
        display_name: "macOS SDK (Apple Silicon / Intel)",
        approx_size: "18.4 MB",
        primary_url: Some(MACOS_SDK_PRIMARY_URL),
        fallback_url: Some(MACOS_SDK_FALLBACK_URL),
    },
    TargetSdkDef {
        triple: "x86_64-apple-darwin",
        dir_name: "MacOSX.sdk",
        display_name: "macOS SDK (Apple Silicon / Intel)",
        approx_size: "18.4 MB",
        primary_url: Some(MACOS_SDK_PRIMARY_URL),
        fallback_url: Some(MACOS_SDK_FALLBACK_URL),
    },
    // Linux GNU (glibc) targets
    TargetSdkDef {
        triple: "aarch64-unknown-linux-gnu",
        dir_name: "aarch64-unknown-linux-gnu",
        display_name: "Linux GNU (AArch64 64-bit)",
        approx_size: "26.5 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "x86_64-unknown-linux-gnu",
        dir_name: "x86_64-unknown-linux-gnu",
        display_name: "Linux GNU (x86-64 64-bit)",
        approx_size: "28.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "i686-unknown-linux-gnu",
        dir_name: "i686-unknown-linux-gnu",
        display_name: "Linux GNU (i686 32-bit)",
        approx_size: "25.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "armv7-unknown-linux-gnueabihf",
        dir_name: "armv7-unknown-linux-gnueabihf",
        display_name: "Linux GNU (ARMv7 hard-float)",
        approx_size: "24.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "arm-unknown-linux-gnueabi",
        dir_name: "arm-unknown-linux-gnueabi",
        display_name: "Linux GNU (ARM soft-float)",
        approx_size: "24.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "riscv64gc-unknown-linux-gnu",
        dir_name: "riscv64gc-unknown-linux-gnu",
        display_name: "Linux GNU (RISC-V 64-bit)",
        approx_size: "25.5 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "powerpc64le-unknown-linux-gnu",
        dir_name: "powerpc64le-unknown-linux-gnu",
        display_name: "Linux GNU (PPC64 Little-Endian)",
        approx_size: "26.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "powerpc64-unknown-linux-gnu",
        dir_name: "powerpc64-unknown-linux-gnu",
        display_name: "Linux GNU (PPC64 Big-Endian)",
        approx_size: "26.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "powerpc-unknown-linux-gnu",
        dir_name: "powerpc-unknown-linux-gnu",
        display_name: "Linux GNU (PowerPC 32-bit)",
        approx_size: "24.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "s390x-unknown-linux-gnu",
        dir_name: "s390x-unknown-linux-gnu",
        display_name: "Linux GNU (IBM s390x)",
        approx_size: "25.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "sparc64-unknown-linux-gnu",
        dir_name: "sparc64-unknown-linux-gnu",
        display_name: "Linux GNU (SPARC64)",
        approx_size: "25.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "loongarch64-unknown-linux-gnu",
        dir_name: "loongarch64-unknown-linux-gnu",
        display_name: "Linux GNU (LoongArch 64-bit)",
        approx_size: "25.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // Windows MSVC targets
    TargetSdkDef {
        triple: "x86_64-pc-windows-msvc",
        dir_name: "x86_64-pc-windows-msvc",
        display_name: "Windows MSVC CRT & SDK (x86-64)",
        approx_size: "35.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "aarch64-pc-windows-msvc",
        dir_name: "aarch64-pc-windows-msvc",
        display_name: "Windows MSVC CRT & SDK (ARM64)",
        approx_size: "35.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "i686-pc-windows-msvc",
        dir_name: "i686-pc-windows-msvc",
        display_name: "Windows MSVC CRT & SDK (x86 32-bit)",
        approx_size: "32.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // Windows MinGW GNU targets
    TargetSdkDef {
        triple: "x86_64-pc-windows-gnu",
        dir_name: "x86_64-pc-windows-gnu",
        display_name: "Windows MinGW-w64 (x86-64)",
        approx_size: "22.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "i686-pc-windows-gnu",
        dir_name: "i686-pc-windows-gnu",
        display_name: "Windows MinGW-w64 (i686)",
        approx_size: "20.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // Android NDK targets
    TargetSdkDef {
        triple: "aarch64-linux-android",
        dir_name: "aarch64-linux-android",
        display_name: "Android NDK Sysroot (ARM64)",
        approx_size: "16.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "armv7-linux-androideabi",
        dir_name: "armv7-linux-androideabi",
        display_name: "Android NDK Sysroot (ARMv7)",
        approx_size: "15.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "x86_64-linux-android",
        dir_name: "x86_64-linux-android",
        display_name: "Android NDK Sysroot (x86-64)",
        approx_size: "16.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "i686-linux-android",
        dir_name: "i686-linux-android",
        display_name: "Android NDK Sysroot (x86)",
        approx_size: "15.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // Linux musl targets
    TargetSdkDef {
        triple: "x86_64-unknown-linux-musl",
        dir_name: "x86_64-unknown-linux-musl",
        display_name: "Linux musl (x86-64 64-bit)",
        approx_size: "12.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "i686-unknown-linux-musl",
        dir_name: "i686-unknown-linux-musl",
        display_name: "Linux musl (i686 32-bit)",
        approx_size: "11.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "aarch64-unknown-linux-musl",
        dir_name: "aarch64-unknown-linux-musl",
        display_name: "Linux musl (AArch64 64-bit)",
        approx_size: "12.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "armv7-unknown-linux-musleabihf",
        dir_name: "armv7-unknown-linux-musleabihf",
        display_name: "Linux musl (ARMv7 hard-float)",
        approx_size: "11.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "arm-unknown-linux-musleabihf",
        dir_name: "arm-unknown-linux-musleabihf",
        display_name: "Linux musl (ARM hard-float)",
        approx_size: "11.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "riscv64gc-unknown-linux-musl",
        dir_name: "riscv64gc-unknown-linux-musl",
        display_name: "Linux musl (RISC-V 64-bit)",
        approx_size: "12.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "powerpc64le-unknown-linux-musl",
        dir_name: "powerpc64le-unknown-linux-musl",
        display_name: "Linux musl (PPC64 Little-Endian)",
        approx_size: "12.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // FreeBSD
    TargetSdkDef {
        triple: "x86_64-unknown-freebsd",
        dir_name: "x86_64-unknown-freebsd",
        display_name: "FreeBSD Base Sysroot (x86-64)",
        approx_size: "28.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    // Windows LLVM MinGW targets
    TargetSdkDef {
        triple: "x86_64-pc-windows-gnullvm",
        dir_name: "x86_64-pc-windows-gnullvm",
        display_name: "Windows MinGW LLVM (x86-64)",
        approx_size: "22.0 MB",
        primary_url: None,
        fallback_url: None,
    },
    TargetSdkDef {
        triple: "aarch64-pc-windows-gnullvm",
        dir_name: "aarch64-pc-windows-gnullvm",
        display_name: "Windows MinGW LLVM (ARM64)",
        approx_size: "22.0 MB",
        primary_url: None,
        fallback_url: None,
    },
];

/// The default directory storing all cached SDKs.
pub fn default_sdks_dir() -> PathBuf {
    if let Ok(p) = std::env::var("HYPRCROSS_SDK_DIR")
        && !p.trim().is_empty()
    {
        return PathBuf::from(p.trim());
    }
    crate::config::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sdks")
}

/// Find SDK definition for a given target triple.
pub fn find_sdk_def(triple: &str) -> Option<&'static TargetSdkDef> {
    KNOWN_SDKS.iter().find(|s| s.triple == triple)
}

/// Default destination path for a given target's SDK.
pub fn sdk_path_for_target(triple: &str) -> PathBuf {
    let dir_name = find_sdk_def(triple).map_or(triple, |d| d.dir_name);
    default_sdks_dir().join(dir_name)
}

/// Checks whether `path` contains a valid SDK structure.
pub fn is_valid_sdk(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    // Apple Mach-O SDK markers
    path.join("usr").is_dir()
        || path.join("System").is_dir()
        || path.join("SDKSettings.json").is_file()
        || path.join("SDKSettings.plist").is_file()
        // Linux / Unix ELF sysroot markers
        || path.join("lib").is_dir()
        || path.join("usr/lib").is_dir()
        || path.join("usr/include").is_dir()
        // Windows MSVC / MinGW SDK markers
        || path.join("ucrt").is_dir()
        || path.join("um").is_dir()
        || path.join("lib/x64").is_dir()
        || path.join("lib/arm64").is_dir()
}

/// Checks whether the target requires an external SDK to link.
pub fn target_needs_sdk(target: &TargetInfo) -> bool {
    if target.is_host() || target.links_without_c_toolchain() {
        return false;
    }
    // Musl targets have self-contained libc.a and startup files in rustup toolchain
    if target.env == TargetEnv::Musl && target.arch_matches_host() {
        return false;
    }
    // Bare metal & wasm don't need external SDKs
    if target.os == TargetOs::None || target.os == TargetOs::Wasi {
        return false;
    }
    true
}

/// Look up user-configured SDK path from environment variables.
pub fn configured_sdk_path(triple: &str) -> Option<PathBuf> {
    let key = triple.replace(['-', '.'], "_").to_uppercase();
    if let Ok(p) = std::env::var(format!("HYPRCROSS_SDK_PATH_{key}"))
        && !p.trim().is_empty()
    {
        return Some(PathBuf::from(p.trim()));
    }
    if let Ok(p) = std::env::var("HYPRCROSS_SDK_PATH")
        && !p.trim().is_empty()
    {
        let path = PathBuf::from(p.trim());
        let target_path = path.join(triple);
        if is_valid_sdk(&target_path) {
            return Some(target_path);
        }
        if is_valid_sdk(&path) {
            return Some(path);
        }
    }
    if triple.contains("-apple-") {
        if let Ok(p) = std::env::var("HYPRCROSS_MACOS_SDK_PATH")
            && !p.trim().is_empty()
        {
            return Some(PathBuf::from(p.trim()));
        }
        if let Ok(p) = std::env::var("SDKROOT")
            && !p.trim().is_empty()
        {
            let path = PathBuf::from(p.trim());
            if is_valid_sdk(&path) {
                return Some(path);
            }
        }
    }
    None
}

/// Ensure a valid SDK is available for `target`, downloading it automatically if missing.
pub fn ensure_sdk(target: &TargetInfo) -> Result<Option<PathBuf>> {
    if !target_needs_sdk(target) {
        return Ok(None);
    }

    if let Some(explicit) = configured_sdk_path(&target.triple) {
        if is_valid_sdk(&explicit) {
            return Ok(Some(explicit));
        }
        bail!(
            "configured SDK at {} is missing or invalid; check HYPRCROSS_SDK_PATH",
            explicit.display()
        );
    }

    let sdk_path = sdk_path_for_target(&target.triple);
    if is_valid_sdk(&sdk_path) {
        return Ok(Some(sdk_path));
    }

    // Auto-bootstrap missing SDK
    let sdk_def = find_sdk_def(&target.triple);
    bootstrap_target_sdk(&target.triple, sdk_def, &sdk_path)?;
    Ok(Some(sdk_path))
}

/// Download URLs for a target's SDK.
pub fn sdk_urls(triple: &str, def: Option<&TargetSdkDef>) -> (String, String) {
    if let Ok(url) = std::env::var(format!(
        "HYPRCROSS_SDK_URL_{}",
        triple.replace(['-', '.'], "_").to_uppercase()
    )) && !url.trim().is_empty()
    {
        return (url.trim().to_string(), url.trim().to_string());
    }

    let repo = std::env::var("HYPRCROSS_SDK_REPO").unwrap_or_else(|_| DEFAULT_SDK_REPO.to_string());
    let repo_clean = repo.trim_end_matches('/');

    let fallback_repo = FALLBACK_SDK_REPO.trim_end_matches('/');

    if let Some(d) = def
        && let Some(primary) = d.primary_url
    {
        let fallback = d.fallback_url.unwrap_or(primary);
        return (primary.to_string(), fallback.to_string());
    }

    let primary = format!("{repo_clean}/{triple}.tar.xz");
    let fallback = format!("{fallback_repo}/{triple}.tar.xz");
    (primary, fallback)
}

fn bootstrap_target_sdk(triple: &str, def: Option<&TargetSdkDef>, dest: &Path) -> Result<()> {
    let name = def.map_or(triple, |d| d.display_name);
    let size_desc = def.map_or("", |d| d.approx_size);
    let (primary_url, fallback_url) = sdk_urls(triple, def);

    ui::action(
        "fetching",
        format!(
            "SDK for {name}... {}",
            if size_desc.is_empty() {
                String::new()
            } else {
                format!("[{size_desc}]")
            }
        ),
    );

    let temp_dir =
        tempfile::tempdir().context("cannot create temporary directory for SDK download")?;
    let tarball_path = temp_dir.path().join("sdk.tar.xz");

    let downloaded = download_file(&primary_url, &tarball_path).or_else(|e| {
        ui::trace(format!(
            "primary SDK download URL failed ({e}), trying fallback..."
        ));
        download_file(&fallback_url, &tarball_path)
    });

    if let Err(e) = downloaded {
        ui::trace(format!("SDK download error: {e:#}"));
        bail!(
            "Failed to fetch SDK for {triple} from {primary_url}. \
             Check network connection or set HYPRCROSS_SDK_PATH."
        );
    }

    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir).context("cannot create SDK extraction directory")?;

    extract_tarball(&tarball_path, &extract_dir)?;

    let source_sdk = find_extracted_sdk(&extract_dir)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).context("cannot create SDK destination parent directory")?;
    }
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }

    if let Err(e) = fs::rename(&source_sdk, dest) {
        ui::trace(format!(
            "cannot rename {source_sdk:?} to {dest:?} ({e}); copying instead"
        ));
        copy_dir_recursive(&source_sdk, dest)?;
    }

    if !is_valid_sdk(dest) {
        bail!(
            "downloaded SDK at {} is incomplete or corrupted",
            dest.display()
        );
    }

    ui::action("installed", format!("SDK to {}", ui::tilde(dest)));
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<()> {
    if find_executable("curl").is_some() {
        let status = Command::new("curl")
            .args(["-fsSL", "--retry", "3", "--retry-connrefused", "-o"])
            .arg(dest)
            .arg(url)
            .status()
            .context("failed to invoke curl")?;
        if status.success() {
            return Ok(());
        }
        bail!("curl exited with code {:?}", status.code());
    }

    if find_executable("wget").is_some() {
        let status = Command::new("wget")
            .args(["-q", "--tries=3", "-O"])
            .arg(dest)
            .arg(url)
            .status()
            .context("failed to invoke wget")?;
        if status.success() {
            return Ok(());
        }
        bail!("wget exited with code {:?}", status.code());
    }

    #[cfg(windows)]
    {
        if find_executable("powershell").is_some() {
            let script = format!(
                "Invoke-WebRequest -Uri '{url}' -OutFile '{}'",
                dest.display()
            );
            let status = Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .status()
                .context("failed to invoke powershell")?;
            if status.success() {
                return Ok(());
            }
        }
    }

    bail!("no download tool (curl, wget) found on PATH to fetch SDK")
}

fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    if find_executable("tar").is_some() {
        let status = Command::new("tar")
            .args(["-xf"])
            .arg(tarball)
            .args(["-C"])
            .arg(dest)
            .status()
            .context("failed to invoke tar")?;
        if status.success() {
            return Ok(());
        }
        bail!("tar exited with code {:?}", status.code());
    }
    bail!("`tar` executable not found on PATH to extract SDK archive")
}

fn find_extracted_sdk(extract_dir: &Path) -> Result<PathBuf> {
    if is_valid_sdk(extract_dir) {
        return Ok(extract_dir.to_path_buf());
    }

    let entries = fs::read_dir(extract_dir).context("cannot read extracted SDK directory")?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && is_valid_sdk(&path) {
            return Ok(path);
        }
    }

    // Even if markers are loose, return the directory if non-empty
    for entry in fs::read_dir(extract_dir)?.flatten() {
        if entry.path().is_dir() {
            return Ok(entry.path());
        }
    }

    bail!("no valid SDK directory structure found inside downloaded archive")
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Install SDK manually for one or more targets.
pub fn install_sdks(targets: &[String], all: bool) -> Result<()> {
    let to_install: Vec<&str> = if all {
        KNOWN_SDKS.iter().map(|d| d.triple).collect()
    } else if targets.is_empty() {
        bail!("specify target(s) to install SDK for, or pass `--all`");
    } else {
        targets.iter().map(String::as_str).collect()
    };

    ui::out(format!(
        "{} {}",
        ui::brand("Installing SDKs"),
        ui::dim(format!("· {} targets", to_install.len()))
    ));

    for t in to_install {
        let target = TargetInfo::parse(t);
        let sdk_path = sdk_path_for_target(&target.triple);
        if is_valid_sdk(&sdk_path) {
            ui::out(format!(
                "  {} {} is already installed at {}",
                "✓".green().bold(),
                t.bold(),
                ui::dim(ui::tilde(&sdk_path))
            ));
            continue;
        }
        let def = find_sdk_def(&target.triple);
        bootstrap_target_sdk(&target.triple, def, &sdk_path)?;
    }
    ui::success("SDK installation completed");
    Ok(())
}

/// Remove SDK for one or more targets.
pub fn remove_sdks(targets: &[String], all: bool) -> Result<()> {
    let sdks_dir = default_sdks_dir();
    if !sdks_dir.exists() {
        ui::out("no SDKs installed");
        return Ok(());
    }

    if all {
        let count = fs::read_dir(&sdks_dir)?
            .flatten()
            .filter(|e| e.path().is_dir())
            .count();
        fs::remove_dir_all(&sdks_dir)?;
        ui::success(format!("removed all {count} installed SDKs"));
        return Ok(());
    }

    if targets.is_empty() {
        bail!("specify target(s) to remove, or pass `--all`");
    }

    for t in targets {
        let path = sdk_path_for_target(t);
        if path.exists() {
            fs::remove_dir_all(&path)?;
            ui::success(format!("removed SDK for {t}"));
        } else {
            ui::warn(format!("no SDK installed for {t}"));
        }
    }
    Ok(())
}

/// List all available and installed SDKs.
pub fn list_sdks() -> Result<()> {
    let sdks_dir = default_sdks_dir();
    ui::out(format!(
        "{} {}",
        ui::brand("Target SDKs"),
        ui::dim(format!("· directory: {}", ui::tilde(&sdks_dir)))
    ));

    let width = KNOWN_SDKS.iter().map(|s| s.triple.len()).max().unwrap_or(0) + 2;

    for sdk in KNOWN_SDKS {
        let path = sdk_path_for_target(sdk.triple);
        let installed = is_valid_sdk(&path);
        let status = if installed {
            "✓ installed".green().bold().to_string()
        } else {
            ui::dim(format!("available ({})", sdk.approx_size))
        };
        ui::out(format!(
            "  {:<width$} {:<22} {}",
            sdk.triple.bold(),
            status,
            ui::dim(sdk.display_name)
        ));
    }
    ui::out("");
    ui::out(ui::dim(
        "Missing SDKs download automatically during builds, or via `hyprcross sdk install <target>`.",
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sdks_contain_primary_architectures() {
        assert!(find_sdk_def("aarch64-unknown-linux-gnu").is_some());
        assert!(find_sdk_def("x86_64-apple-darwin").is_some());
        assert!(find_sdk_def("x86_64-pc-windows-msvc").is_some());
        assert!(find_sdk_def("aarch64-linux-android").is_some());
    }

    #[test]
    fn validation_detects_mock_sysroot() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_valid_sdk(tmp.path()));
        fs::create_dir_all(tmp.path().join("usr/lib")).unwrap();
        assert!(is_valid_sdk(tmp.path()));
    }
}
