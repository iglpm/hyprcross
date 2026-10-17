//! Output styling, shared with Rune: a violet `▸` for what hyprcross does, `✓` when it is done,
//! `!` for warnings, `✗` for errors and a dimmed `note:` for what to do about them. Everything
//! hyprcross says goes to stderr, so stdout stays the builder's and the program's.

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static QUIET: AtomicBool = AtomicBool::new(false);
static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(on: bool) {
    QUIET.store(on, Ordering::Relaxed);
}

pub fn quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// `--color`: `always` and `never` override what the terminal and `NO_COLOR` say.
pub fn set_color(choice: &str) {
    match choice {
        "always" => colored::control::set_override(true),
        "never" => colored::control::set_override(false),
        _ => {}
    }
    let on = colors();
    console::set_colors_enabled(on);
    console::set_colors_enabled_stderr(on);
}

pub fn colors() -> bool {
    colored::control::SHOULD_COLORIZE.should_colorize()
}

/// Without 24-bit colour, violet is sent as its 256-colour neighbour rather than rounded to blue.
fn truecolor() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("COLORTERM").is_ok_and(|v| v.contains("truecolor") || v.contains("24bit"))
    })
}

/// The brand colour: bold violet.
pub fn brand(s: impl Display) -> String {
    if !colors() {
        return s.to_string();
    }
    if truecolor() {
        format!("\x1b[1;38;2;167;139;250m{s}\x1b[0m")
    } else {
        format!("\x1b[1;38;5;141m{s}\x1b[0m")
    }
}

pub fn dim(s: impl Display) -> String {
    s.to_string().dimmed().to_string()
}

static OUTPUT: Mutex<Option<ProgressBar>> = Mutex::new(None);

/// One line on stderr, written above the spinner when one is showing.
pub fn err(line: impl Display) {
    let text = format!("{line}\n");
    let guard = OUTPUT.lock().unwrap_or_else(|e| e.into_inner());
    let write = || {
        let mut err = io::stderr().lock();
        let _ = err.write_all(text.as_bytes()).and_then(|_| err.flush());
    };
    match guard.as_ref() {
        Some(bar) => bar.suspend(write),
        None => write(),
    }
}

/// One line on stdout: what commands like `targets` print as their result.
pub fn out(line: impl Display) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
}

/// `▸ verb message`: something hyprcross did.
pub fn action(verb: &str, message: impl Display) {
    if quiet() {
        return;
    }
    err(format!("{} {} {message}", brand("▸"), brand(verb)));
}

/// `✓ message`: a step finished.
pub fn success(message: impl Display) {
    if quiet() {
        return;
    }
    err(format!("{} {message}", "✓".green().bold()));
}

pub fn warn(message: impl Display) {
    err(format!(
        "{} {}",
        "!".yellow().bold(),
        message.to_string().yellow()
    ));
}

pub fn error(message: impl Display) {
    let text = message.to_string();
    let mut lines = text.lines();
    err(format!(
        "{} {}",
        "✗".red().bold(),
        lines.next().unwrap_or_default()
    ));
    for line in lines {
        err(format!("  {line}"));
    }
}

/// An error and the errors that caused it, outermost first.
pub fn fatal(error: &anyhow::Error) {
    self::error(error);
    for cause in error.chain().skip(1) {
        if let Some(h) = cause.downcast_ref::<Hinted>() {
            hint(&h.hint);
        } else {
            err(format!("  {} {cause}", dim("caused by:")));
        }
    }
    if let Some(h) = error.downcast_ref::<Hinted>() {
        hint(&h.hint);
    }
}

/// An error with what to do about it, shown as `✗ message` and `note: hint`.
#[derive(Debug)]
pub struct Hinted {
    pub message: String,
    pub hint: String,
}

impl Hinted {
    pub fn error(message: impl Display, hint: impl Display) -> anyhow::Error {
        anyhow::Error::new(Hinted {
            message: message.to_string(),
            hint: hint.to_string(),
        })
    }
}

impl Display for Hinted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Hinted {}

/// `  note: …`, dimmed and wrapped to the terminal: what to do next.
pub fn hint(message: impl Display) {
    if quiet() {
        return;
    }
    let text = message.to_string();
    let lines = if io::stderr().is_terminal() {
        wrap(&text, term_width().saturating_sub(9).max(30))
    } else {
        text.lines().map(str::to_string).collect()
    };
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            err(format!("  {} {}", dim("note:"), dim(line)));
        } else {
            err(format!("        {}", dim(line)));
        }
    }
}

/// A command hyprcross runs, shown with `--verbose`.
pub fn trace(what: impl Display) {
    if verbose() && !quiet() {
        err(format!("  {} {}", dim("$"), dim(what)));
    }
}

pub fn heading(title: &str) {
    out(brand(title));
}

/// `key  value` rows with the keys dimmed and aligned; rows with empty values are left out.
pub fn kv_lines(rows: &[(&str, String)], indent: usize) -> Vec<String> {
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    rows.iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{:indent$}{}  {v}", "", dim(format!("{k:<width$}"))))
        .collect()
}

pub fn width_of(s: &str) -> usize {
    console::measure_text_width(s)
}

pub fn term_width() -> usize {
    console::Term::stderr()
        .size_checked()
        .map(|(_, w)| w as usize)
        .unwrap_or(80)
        .max(20)
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            if !line.is_empty() && width_of(&line) + 1 + width_of(word) > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

pub fn secs(d: Duration) -> String {
    if d < Duration::from_secs(1) {
        format!("{}ms", d.as_millis())
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

pub fn plural(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else if let Some(stem) = what.strip_suffix('y') {
        format!("{n} {stem}ies")
    } else if what.ends_with('s') || what.ends_with("sh") || what.ends_with("ch") {
        format!("{n} {what}es")
    } else {
        format!("{n} {what}s")
    }
}

/// `~/x` for paths in the home directory: shorter, and what people type.
pub fn tilde(path: &std::path::Path) -> String {
    if let Some(home) = crate::config::dirs_home()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
        .template("{spinner:.141} {prefix:.141.bold} {wide_msg:.dim}")
        .expect("valid template")
}

/// A spinner for work that takes a moment (starting a container, pulling an image); on a
/// terminal only. `finish` replaces it with a `▸` line saying how long it took.
pub struct Activity {
    bar: Option<ProgressBar>,
    started: Instant,
}

impl Activity {
    pub fn start(verb: &str, message: impl Display) -> Self {
        let bar = (!quiet() && io::stderr().is_terminal()).then(|| {
            let bar = ProgressBar::new_spinner();
            bar.set_style(spinner_style());
            bar.set_prefix(verb.to_string());
            bar.set_message(message.to_string());
            bar.enable_steady_tick(Duration::from_millis(80));
            bar
        });
        if let Some(b) = &bar {
            *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(b.clone());
        }
        Self {
            bar,
            started: Instant::now(),
        }
    }

    pub fn set_message(&self, message: impl Display) {
        if let Some(b) = &self.bar {
            b.set_message(message.to_string());
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn finish(mut self, verb: &str, message: impl Display) {
        let took = self.elapsed();
        self.stop();
        action(
            verb,
            format!("{message} {}", dim(format!("({})", secs(took)))),
        );
    }

    pub fn clear(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if let Some(b) = self.bar.take() {
            b.finish_and_clear();
            *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A custom progress bar for image downloads (pulls), showing progress bars, layer status,
/// and timing without raw engine logs.
pub struct PullProgress {
    bar: Option<ProgressBar>,
    started: Instant,
    image: String,
}

impl PullProgress {
    pub fn start(image: &str) -> Self {
        let is_term = !quiet() && io::stderr().is_terminal();
        action("pulling", image);
        let bar = is_term.then(|| {
            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{prefix:.141.bold} [{bar:24.141/dim}] {wide_msg:.dim}")
                    .unwrap_or_else(|_| ProgressStyle::default_bar())
                    .progress_chars("━╸─"),
            );
            pb.set_prefix(format!("{} {}", brand("▸"), brand("pulling")));
            pb.set_message("connecting...");
            pb.enable_steady_tick(Duration::from_millis(80));
            pb
        });
        if let Some(b) = &bar {
            *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(b.clone());
        }
        Self {
            bar,
            started: Instant::now(),
            image: image.to_string(),
        }
    }

    pub fn update(&self, percent: Option<u64>, message: impl Display) {
        if let Some(b) = &self.bar {
            if let Some(p) = percent {
                b.set_position(p.min(100));
            }
            b.set_message(message.to_string());
        }
    }

    pub fn finish(mut self) {
        let took = self.started.elapsed();
        self.stop();
        action(
            "pulled",
            format!("{} {}", self.image, dim(format!("({})", secs(took)))),
        );
    }

    pub fn clear(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if let Some(b) = self.bar.take() {
            b.finish_and_clear();
            *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
}

impl Drop for PullProgress {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals() {
        assert_eq!(plural(1, "target"), "1 target");
        assert_eq!(plural(3, "target"), "3 targets");
        assert_eq!(plural(2, "dependency"), "2 dependencies");
        assert_eq!(plural(2, "process"), "2 processes");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(secs(Duration::from_millis(42)), "42ms");
        assert_eq!(secs(Duration::from_millis(1500)), "1.50s");
    }

    #[test]
    fn wrapping_keeps_words_whole() {
        let lines = wrap("one two three four five six", 10);
        assert!(lines.iter().all(|l| width_of(l) <= 10), "{lines:?}");
        assert_eq!(lines.join(" "), "one two three four five six");
    }

    #[test]
    fn rows_align_and_skip_empty_values() {
        colored::control::set_override(false);
        let rows = kv_lines(
            &[
                ("a", "1".into()),
                ("long", "2".into()),
                ("x", String::new()),
            ],
            2,
        );
        assert_eq!(rows, vec!["  a     1", "  long  2"]);
    }
}
