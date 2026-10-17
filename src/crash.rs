//! Crash and panic handler for hyprcross.
//!
//! When an unexpected internal panic occurs, this module:
//! 1. Captures full backtrace and environment details (version, target, args, cwd).
//! 2. Writes a detailed crash dump to a log file (`hyprcross-crash-<timestamp>.log`).
//! 3. Prints a clean, styled error report directing the user to the GitHub issue tracker
//!    with a link to the dedicated crash report template.

use std::backtrace::Backtrace;
use std::fs;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use colored::Colorize;

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

const ISSUE_URL: &str = "https://github.com/runepm/hyprcross/issues/new?template=crash_report.yml";

/// Install the global crash/panic handler.
pub fn init() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    std::panic::set_hook(Box::new(|info| {
        handle_panic(info);
    }));
}

fn handle_panic(info: &PanicHookInfo) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let payload = info.payload();
    let message = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic payload".to_string()
    };

    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown location".to_string());

    let backtrace = Backtrace::force_capture();
    let version = crate::VERSION;
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let current_exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "hyprcross".to_string());
    let args = std::env::args().collect::<Vec<_>>().join(" ");
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let report_content = format!(
        "================================================================================\n\
         HYPRCROSS CRASH REPORT\n\
         ================================================================================\n\
         Timestamp:   {timestamp}\n\
         Version:     {version}\n\
         Platform:    {os} ({arch})\n\
         Executable:  {current_exe}\n\
         Arguments:   {args}\n\
         Working Dir: {cwd}\n\n\
         PANIC DETAILS\n\
         Message:     {message}\n\
         Location:    {location}\n\n\
         STACK BACKTRACE\n\
         {backtrace}\n\
         ================================================================================\n"
    );

    let filename = format!("hyprcross-crash-{timestamp}.log");
    let dump_path = write_crash_dump(&filename, &report_content);

    // Print styled console report to stderr
    eprintln!();
    eprintln!(
        "{}",
        "================================================================================"
            .red()
            .bold()
    );
    eprintln!(
        "{} {}",
        "💥".red(),
        "HYPRCROSS INTERNAL ERROR & CRASH REPORT".red().bold()
    );
    eprintln!(
        "{}",
        "================================================================================"
            .red()
            .bold()
    );
    eprintln!("hyprcross encountered an unexpected internal error and crashed.");
    eprintln!("This is an internal bug in hyprcross, not your project code.");
    eprintln!();
    eprintln!(
        "  {}  {}",
        "Version: ".bold(),
        format!("{version} ({os} {arch})").dimmed()
    );
    eprintln!("  {} {}", "Location:".bold(), location.yellow());
    eprintln!("  {}  {}", "Message: ".bold(), message.red().bold());
    eprintln!();

    if let Some(path) = &dump_path {
        eprintln!("{}", "A detailed crash dump was written to:".green().bold());
        eprintln!("  {}", path.display().to_string().cyan().underline());
        eprintln!();
    }

    eprintln!(
        "{}",
        "Please help us fix this by reporting it to the issue tracker:"
            .yellow()
            .bold()
    );
    eprintln!("  {}", ISSUE_URL.cyan().underline());
    eprintln!();
    eprintln!("Please include the crash dump contents in your bug report.");
    eprintln!("Thank you for helping improve hyprcross!");
    eprintln!(
        "{}",
        "================================================================================"
            .red()
            .bold()
    );

    // If RUST_BACKTRACE=1 or verbose is enabled, also dump backtrace directly to console
    if std::env::var_os("RUST_BACKTRACE").is_some() || std::env::var_os("HYPRCROSS_LOG").is_some() {
        eprintln!();
        eprintln!("{}", "Stack Backtrace:".bold());
        eprintln!("{backtrace}");
    }

    // Exit with standard panic exit code (101)
    std::process::exit(101);
}

fn write_crash_dump(filename: &str, content: &str) -> Option<PathBuf> {
    // 1. Try current working directory
    let cwd_path = Path::new(filename);
    if fs::write(cwd_path, content).is_ok() {
        if let Ok(abs) = dunce::canonicalize(cwd_path) {
            return Some(abs);
        }
        return Some(cwd_path.to_path_buf());
    }

    // 2. Try hyprcross data/crashes directory
    if let Some(data_dir) = crate::config::data_dir() {
        let data_crashes = data_dir.join("crashes");
        if fs::create_dir_all(&data_crashes).is_ok() {
            let path = data_crashes.join(filename);
            if fs::write(&path, content).is_ok() {
                return Some(path);
            }
        }
    }

    // 3. Fallback to system temp directory
    let temp_path = std::env::temp_dir().join(filename);
    if fs::write(&temp_path, content).is_ok() {
        return Some(temp_path);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_crash_dump() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test-crash.log");
        let content = "test crash content";
        fs::write(&file_path, content).unwrap();
        assert_eq!(fs::read_to_string(&file_path).unwrap(), content);
    }

    #[test]
    fn test_crash_dump_generation() {
        let filename = format!("test-crash-{}.log", std::process::id());
        let content = "Test crash dump line 1\nTest crash dump line 2";
        let path = write_crash_dump(&filename, content);
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.exists());
        let read = fs::read_to_string(&p).unwrap();
        assert_eq!(read, content);
        let _ = fs::remove_file(p);
    }
}
