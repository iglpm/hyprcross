//! `hyprcross help`: a grouped overview in Igel's layout, and clap's help in Igel's colours.

use crate::ui;
use colored::Colorize;

const SECTIONS: &[(&str, &[&str])] = &[
    (
        "Build",
        &[
            "build", "check", "test", "run", "bench", "clippy", "doc", "clean",
        ],
    ),
    ("Targets and SDKs", &["targets", "sdk", "doctor", "init"]),
];

/// Handle `hyprcross`, `-h`, `--help`, `help` and `help <command>`; whether it did.
pub fn intercept(args: &[std::ffi::OsString]) -> bool {
    let words: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let mut rest: &[String] = &words;
    while let Some(first) = rest.first() {
        if (first == "--color") && rest.len() >= 2 {
            rest = &rest[2..];
        } else if first.starts_with("--color=") || first == "-q" || first == "--quiet" {
            rest = &rest[1..];
        } else {
            break;
        }
    }
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match rest.as_slice() {
        [] | ["-h"] | ["--help"] | ["help"] => {
            // `hyprcross` alone builds when a default target is configured.
            if rest.is_empty() && has_default_target() {
                return false;
            }
            overview();
        }
        ["help", command @ ..] => command_help(command),
        _ => return false,
    }
    true
}

fn has_default_target() -> bool {
    std::env::var_os("HYPRCROSS_TARGET").is_some()
        || std::env::current_dir()
            .ok()
            .and_then(|d| crate::config::Config::load_from_dir(&d).ok())
            .is_some_and(|c| c.build.default_target.is_some())
}

fn command_help(path: &[&str]) {
    let mut argv: Vec<std::ffi::OsString> = vec![crate::host::command().into()];
    argv.extend(path.iter().map(Into::into));
    argv.push("--help".into());
    match crate::cli::try_parse(&argv) {
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
            let _ = e.print();
        }
        _ => {
            ui::error(format!("no such command: `{}`", path.join(" ")));
            ui::hint(format!("{} lists them", crate::host::invocation("help")));
            std::process::exit(1);
        }
    }
}

fn overview() {
    let cmd = crate::cli::command();
    let host = crate::host::command();
    let label = |name: &str| -> (String, usize) {
        let aliases: Vec<&str> = cmd
            .find_subcommand(name)
            .map(|c| c.get_visible_aliases().collect())
            .unwrap_or_default();
        let plain = std::iter::once(name)
            .chain(aliases.iter().copied())
            .collect::<Vec<_>>()
            .join(", ");
        let styled = std::iter::once(ui::brand(name))
            .chain(aliases.iter().map(|a| a.dimmed().to_string()))
            .collect::<Vec<_>>()
            .join(&", ".dimmed().to_string());
        (styled, plain.chars().count())
    };
    let width = SECTIONS
        .iter()
        .flat_map(|(_, names)| names.iter())
        .map(|n| label(n).1)
        .max()
        .unwrap_or(10)
        + 3;

    let mut out = Vec::new();
    let (version, about) = if host == "hyprcross" {
        (
            crate::VERSION.to_string(),
            "cross-compile and cross-test Rust, with Igel or Cargo",
        )
    } else {
        (
            format!("(hyprcross {})", crate::VERSION),
            "build, test and run for other targets",
        )
    };
    out.push(format!(
        "{} {}  {}",
        ui::brand(host),
        version.dimmed(),
        about.dimmed()
    ));
    out.push(String::new());
    out.push(format!(
        "{}  {} {}",
        "Usage".bold(),
        host.bold(),
        "<command> -t <target> [options] [-- builder arguments]".dimmed()
    ));
    for (title, names) in SECTIONS {
        out.push(String::new());
        out.push(title.bold().to_string());
        for name in *names {
            let Some(sub) = cmd.find_subcommand(name) else {
                continue;
            };
            let (styled, w) = label(name);
            let about = sub.get_about().map(|a| a.to_string()).unwrap_or_default();
            out.push(format!(
                "  {styled}{:pad$}{about}",
                "",
                pad = width.saturating_sub(w)
            ));
        }
    }
    out.push(String::new());
    out.push("Options".bold().to_string());
    for (flag, what) in [
        (
            "-t, --target <triple>",
            "Target to build for (else the default target)",
        ),
        ("--builder <tool>", "auto, igl or cargo"),
        ("--linker <linker>", "hyprlnk or default"),
        ("-q, --quiet", "Only print errors"),
        ("-v, --verbose", "Print the commands it runs"),
        ("--color <when>", "auto, always or never"),
        ("-h, --help", "Help, or help for a command"),
        ("-V, --version", "Print the version"),
    ] {
        out.push(format!(
            "  {flag}{:pad$}{what}",
            "",
            pad = 25usize.saturating_sub(flag.len())
        ));
    }
    out.push(String::new());
    out.push(format!(
        "{}  {}",
        format!("{host} help <command>").bold(),
        "options and examples for one command".dimmed()
    ));
    println!("{}", out.join("\n"));
}

pub fn styles() -> clap::builder::Styles {
    use clap::builder::styling::{Ansi256Color, AnsiColor, Color, Style};
    let quill = Some(Color::Ansi256(Ansi256Color(179)));
    clap::builder::Styles::styled()
        .header(Style::new().bold().fg_color(quill))
        .usage(Style::new().bold().fg_color(quill))
        .literal(Style::new().bold())
        .placeholder(Style::new().dimmed())
        .valid(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
        .invalid(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow))))
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
}

/// An "Examples:" section for a command's help; `hyprcross` in them becomes the host's command.
pub fn examples(rows: &[(&str, &str)]) -> String {
    use clap::builder::styling::{Ansi256Color, Color, Style};
    let header = Style::new()
        .bold()
        .fg_color(Some(Color::Ansi256(Ansi256Color(179))));
    let literal = Style::new().bold();
    let rows: Vec<(String, &str)> = rows
        .iter()
        .map(|(cmd, what)| {
            let cmd = match cmd.strip_prefix("hyprcross ") {
                Some(rest) => format!("{} {rest}", crate::host::command()),
                None => cmd.to_string(),
            };
            (cmd, *what)
        })
        .collect();
    let width = rows.iter().map(|(c, _)| c.len()).max().unwrap_or(0) + 3;
    let mut out = format!("{}Examples:{}\n", header.render(), header.render_reset());
    for (cmd, what) in &rows {
        out.push_str(&format!(
            "  {}{cmd}{}{:pad$}{what}\n",
            literal.render(),
            literal.render_reset(),
            "",
            pad = width - cmd.len()
        ));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::CommandFactory;

    #[test]
    fn every_visible_command_is_in_the_overview() {
        let cmd = Cli::command();
        let listed: Vec<&str> = SECTIONS
            .iter()
            .flat_map(|(_, n)| n.iter().copied())
            .collect();
        for c in cmd.get_subcommands().filter(|c| !c.is_hide_set()) {
            assert!(
                listed.contains(&c.get_name()),
                "{} is missing",
                c.get_name()
            );
        }
        for name in listed {
            assert!(cmd.find_subcommand(name).is_some(), "{name} doesn't exist");
        }
    }

    #[test]
    fn the_cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
