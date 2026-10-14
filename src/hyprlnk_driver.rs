use std::ffi::OsString;
use std::path::PathBuf;

/// Check if hyprlnk is available (either built into this binary or present in PATH)
pub fn is_available() -> bool {
    #[cfg(feature = "builtin-hyprlnk")]
    {
        true
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        find_external_hyprlnk().is_some()
    }
}

/// Find external hyprlnk executable in PATH
pub fn find_external_hyprlnk() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("hyprlnk");
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join("hyprlnk.exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// Return path to current hyprcross binary
pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// Check if a target triple is supported by hyprlnk
pub fn supports_target(triple: &str) -> bool {
    #[cfg(feature = "builtin-hyprlnk")]
    {
        hyprlnk::supports_target(triple)
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        // General support matrix for hyprlnk
        let is_linux = triple.contains("-linux-");
        let is_apple = triple.contains("-apple-darwin");
        let is_windows_msvc = triple.contains("-windows-msvc");
        is_linux || is_apple || is_windows_msvc
    }
}

/// Get the rustc linker flavor string if required (e.g. for Apple targets)
pub fn rustc_linker_flavor(triple: &str) -> Option<&'static str> {
    #[cfg(feature = "builtin-hyprlnk")]
    {
        hyprlnk::rustc_linker_flavor(triple)
    }
    #[cfg(not(feature = "builtin-hyprlnk"))]
    {
        if triple.contains("-apple-") {
            Some("ld64.lld")
        } else {
            None
        }
    }
}

/// Run hyprlnk with the provided arguments and return exit code
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
        if let Some(exe) = find_external_hyprlnk() {
            let status = std::process::Command::new(exe)
                .args(args)
                .status();
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("Failed to execute external hyprlnk: {e}");
                    1
                }
            }
        } else {
            eprintln!("hyprlnk was requested but neither built-in hyprlnk nor external binary was found");
            1
        }
    }
}

/// Formulate rustflags to inject hyprlnk into rustc invocations
pub fn get_rustflags(triple: &str, self_as_linker: bool) -> Vec<String> {
    let mut flags = Vec::new();

    if self_as_linker {
        if let Some(exe) = current_exe() {
            let exe_str = exe.to_string_lossy().to_string();
            flags.push(format!("-C linker={exe_str}"));
            flags.push("-C link-arg=__hyprlnk".to_string());
        } else if let Some(ext) = find_external_hyprlnk() {
            flags.push(format!("-C linker={}", ext.to_string_lossy()));
        } else {
            flags.push("-C linker=hyprlnk".to_string());
        }
    } else if let Some(ext) = find_external_hyprlnk() {
        flags.push(format!("-C linker={}", ext.to_string_lossy()));
    } else {
        flags.push("-C linker=hyprlnk".to_string());
    }

    if let Some(flavor) = rustc_linker_flavor(triple) {
        flags.push(format!("-C linker-flavor={flavor}"));
    }

    flags
}
