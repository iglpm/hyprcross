//! hyprlnk, the linker built into hyprcross. Rune links with its own copy and needs nothing from
//! here; Cargo gets this executable as the target's linker, under the name `hyprlnk`.
//!
//! The executable links instead of running the CLI when it is started
//! - as `hyprlnk` (the name hyprcross gives it for Cargo),
//! - as `ld` by the C compiler hyprlnk runs for a `cc` command line (`HYPRLNK_DRIVER` is set),
//! - with `HYPRCROSS_HYPRLNK` set, or as `hyprcross __hyprlnk <args>`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Makes the hyprcross executable act as the linker.
pub const LINKER_ENV: &str = "HYPRCROSS_HYPRLNK";

/// Set by hyprlnk for the C compiler it runs, which then runs hyprlnk again as `ld`.
const DRIVER_ENV: &str = "HYPRLNK_DRIVER";

/// The C compiler hyprlnk links through on Linux (it knows where the C runtime is).
pub const CC_ENV: &str = "HYPRLNK_CC";

/// If this process was started as the linker: link, and return the exit code.
pub fn linker_invocation(args: &[OsString]) -> Option<i32> {
    if args.get(1).map(OsString::as_os_str) == Some(OsStr::new("__hyprlnk")) {
        let rest = std::iter::once(OsString::from("hyprlnk")).chain(args[2..].iter().cloned());
        return Some(run_linker(rest));
    }
    let name = args
        .first()
        .and_then(|a| Path::new(a).file_name())
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    let as_ld = std::env::var_os(DRIVER_ENV).is_some() && (name == "ld" || name.starts_with("ld."));
    if name == "hyprlnk" || as_ld || std::env::var_os(LINKER_ENV).is_some() {
        return Some(run_linker(args.iter().cloned()));
    }
    None
}

pub fn is_available() -> bool {
    cfg!(feature = "builtin-hyprlnk") || find_external_hyprlnk().is_some()
}

/// A standalone `hyprlnk` on PATH.
pub fn find_external_hyprlnk() -> Option<PathBuf> {
    crate::builder::find_executable("hyprlnk")
}

pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

pub fn supports_target(triple: &str) -> bool {
    #[cfg(feature = "builtin-hyprlnk")]
    {
        hyprlnk::supports_target(triple)
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        crate::targets::TargetInfo::parse(triple).hyprlnk_supported
    }
}

/// The `-C linker-flavor` rustc needs for hyprlnk on this target, if not its default.
pub fn rustc_linker_flavor(triple: &str) -> Option<&'static str> {
    #[cfg(feature = "builtin-hyprlnk")]
    {
        hyprlnk::rustc_linker_flavor(triple)
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        if triple.contains("-apple-") {
            Some("ld64.lld")
        } else if (cfg!(target_os = "macos") || cfg!(not(unix)))
            && (triple.contains("-linux-") || triple.contains("-freebsd"))
        {
            Some("ld.lld")
        } else {
            None
        }
    }
}

/// Link with the full command line (`argv[0]` first) and return the exit code.
pub fn run_linker<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    #[cfg(feature = "builtin-hyprlnk")]
    {
        hyprlnk::main_with_args(args)
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        let mut args = args.into_iter().map(Into::into);
        let _argv0 = args.next();
        let Some(exe) = find_external_hyprlnk() else {
            eprintln!("hyprcross: this build has no built-in hyprlnk, and none is on PATH");
            return 1;
        };
        match std::process::Command::new(exe).args(args).status() {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("hyprcross: cannot run hyprlnk: {e}");
                1
            }
        }
    }
}

/// What Cargo should run as the target's linker: a standalone hyprlnk, or this executable under
/// the name `hyprlnk` (a link in the cache directory, whose path stays the same across
/// hyprcross updates, so Cargo's fingerprints do too). `None` where links can't be made.
pub fn linker_for_cargo() -> Option<PathBuf> {
    if let Some(external) = find_external_hyprlnk() {
        return Some(external);
    }
    #[cfg(unix)]
    {
        link_named_hyprlnk(&crate::config::cache_dir()?.join("bin"), &current_exe()?)
    }
    #[cfg(windows)]
    {
        link_named_hyprlnk(&crate::config::cache_dir()?.join("bin"), &current_exe()?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

#[cfg(unix)]
fn link_named_hyprlnk(dir: &Path, exe: &Path) -> Option<PathBuf> {
    let link = dir.join("hyprlnk");
    if std::fs::read_link(&link).ok().as_deref() == Some(exe) {
        return Some(link);
    }
    std::fs::create_dir_all(dir).ok()?;
    // Replace atomically: another build may be linking through it right now.
    let tmp = dir.join(format!(".hyprlnk.{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(exe, &tmp).ok()?;
    std::fs::rename(&tmp, &link).ok()?;
    Some(link)
}

#[cfg(windows)]
fn link_named_hyprlnk(dir: &Path, exe: &Path) -> Option<PathBuf> {
    let link = dir.join("hyprlnk.exe");
    if link.is_file() {
        return Some(link);
    }
    std::fs::create_dir_all(dir).ok()?;
    let _ = std::fs::hard_link(exe, &link).or_else(|_| std::fs::copy(exe, &link).map(|_| ()));
    link.is_file().then_some(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cli_is_not_mistaken_for_the_linker() {
        assert_eq!(
            linker_invocation(&["hyprcross".into(), "build".into()]),
            None
        );
        assert_eq!(
            linker_invocation(&["/usr/bin/cargo-hyprcross".into(), "hyprcross".into()]),
            None
        );
        // `ld` without hyprlnk's driver variable is some other program's business.
        if std::env::var_os(DRIVER_ENV).is_none() {
            assert_eq!(linker_invocation(&["/tmp/x/ld".into(), "-o".into()]), None);
        }
    }

    #[test]
    fn apple_targets_need_the_ld64_flavor() {
        assert_eq!(
            rustc_linker_flavor("aarch64-apple-darwin"),
            Some("ld64.lld")
        );
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(rustc_linker_flavor("aarch64-unknown-linux-gnu"), None);
        #[cfg(any(not(unix), target_os = "macos"))]
        assert_eq!(
            rustc_linker_flavor("aarch64-unknown-linux-gnu"),
            Some("ld.lld")
        );
    }

    #[cfg(unix)]
    #[test]
    fn cargo_gets_a_stable_link_named_hyprlnk() {
        let dir = tempfile::tempdir().unwrap();
        let exe = current_exe().unwrap();
        let link = link_named_hyprlnk(dir.path(), &exe).unwrap();
        assert_eq!(link, dir.path().join("hyprlnk"));
        assert_eq!(std::fs::read_link(&link).unwrap(), exe);
        assert_eq!(
            link_named_hyprlnk(dir.path(), &exe).unwrap(),
            link,
            "reused"
        );

        let other = dir.path().join("hyprcross-2");
        std::fs::write(&other, b"").unwrap();
        link_named_hyprlnk(dir.path(), &other).unwrap();
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            other,
            "follows a new executable"
        );
    }
}
