//! Native macOS cross-compilation pipeline: automatic SDK bootstrap, sysroot configuration,
//! and Mach-O linking with hyprlnk.

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::builder::find_executable;
use crate::ui;

pub const MACOS_SDK_PRIMARY_URL: &str =
    "https://github.com/runepm/sdks/releases/download/v2027.0.1/MacOSX.sdk.tar.xz";
pub const MACOS_SDK_FALLBACK_URL: &str =
    "https://github.com/joseluisq/macosx-sdks/releases/download/26.1/MacOSX26.1.sdk.tar.xz";

/// The default filesystem location for the cached macOS SDK.
pub fn default_sdk_path() -> PathBuf {
    crate::config::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sdks")
        .join("MacOSX.sdk")
}

/// Checks whether `path` contains a valid macOS SDK structure.
pub fn is_valid_sdk(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    path.join("usr").is_dir()
        || path.join("System").is_dir()
        || path.join("SDKSettings.json").is_file()
        || path.join("SDKSettings.plist").is_file()
}

/// Determine the SDK path to use: explicit environment variable overrides take precedence.
pub fn configured_sdk_path() -> Option<PathBuf> {
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
    None
}

/// Ensure a valid macOS SDK is available, downloading and extracting it if missing.
pub fn ensure_macos_sdk() -> Result<PathBuf> {
    if let Some(explicit) = configured_sdk_path() {
        if is_valid_sdk(&explicit) {
            return Ok(explicit);
        }
        bail!(
            "configured macOS SDK at {} does not exist or is invalid; check HYPRCROSS_MACOS_SDK_PATH or SDKROOT",
            explicit.display()
        );
    }

    let sdk_path = default_sdk_path();
    if is_valid_sdk(&sdk_path) {
        return Ok(sdk_path);
    }

    bootstrap_sdk(&sdk_path)?;
    Ok(sdk_path)
}

fn bootstrap_sdk(dest: &Path) -> Result<()> {
    ui::action(
        "fetching",
        "minimal macOS SDK for cross-compilation... [18.4 MB]",
    );

    let temp_dir =
        tempfile::tempdir().context("cannot create temporary directory for SDK download")?;
    let tarball_path = temp_dir.path().join("MacOSX.sdk.tar.xz");

    let downloaded = download_file(MACOS_SDK_PRIMARY_URL, &tarball_path).or_else(|e| {
        ui::trace(format!("primary SDK URL failed ({e}), trying fallback..."));
        download_file(MACOS_SDK_FALLBACK_URL, &tarball_path)
    });

    if let Err(e) = downloaded {
        ui::trace(format!("SDK download error: {e:#}"));
        bail!(
            "Failed to fetch macOS SDK stubs. Check internet connection or set HYPRCROSS_MACOS_SDK_PATH."
        );
    }

    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir).context("cannot create SDK extraction directory")?;

    extract_tarball(&tarball_path, &extract_dir)?;

    let source_sdk = find_extracted_sdk(&extract_dir)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).context("cannot create SDK parent directory")?;
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
            "downloaded macOS SDK at {} is incomplete or corrupted",
            dest.display()
        );
    }

    ui::action("extracted", format!("to {}", ui::tilde(dest)));
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

    bail!("no download tool (curl, wget) found on PATH")
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

    bail!("no valid macOS SDK root found inside downloaded archive")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_ends_with_macosx_sdk() {
        let p = default_sdk_path();
        assert!(p.ends_with("sdks/MacOSX.sdk") || p.ends_with(r"sdks\MacOSX.sdk"));
    }

    #[test]
    fn validation_detects_mock_sdk() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_valid_sdk(tmp.path()));
        fs::create_dir(tmp.path().join("usr")).unwrap();
        assert!(is_valid_sdk(tmp.path()));
    }
}
