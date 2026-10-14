use colored::Colorize;
use std::time::Duration;

pub fn header(title: &str) {
    println!("{}", title.bold().cyan());
}

pub fn badge(name: &str) -> String {
    format!("[{}]", name).bold().blue().to_string()
}

pub fn info(tag: &str, msg: impl std::fmt::Display) {
    println!("{} {}", format!("[{}]", tag).bold().cyan(), msg);
}

pub fn success(tag: &str, msg: impl std::fmt::Display) {
    println!("{} {}", format!("[{}]", tag).bold().green(), msg);
}

pub fn warn(msg: impl std::fmt::Display) {
    eprintln!("{} {}", "[warn]".bold().yellow(), msg);
}

pub fn error(msg: impl std::fmt::Display) {
    eprintln!("{} {}", "[error]".bold().red(), msg);
}

pub fn timing(phase: &str, duration: Duration) {
    let millis = duration.as_millis();
    let time_str = if millis < 1000 {
        format!("{}ms", millis)
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    };
    println!(
        "{} {} finished in {}",
        "[time]".bold().magenta(),
        phase,
        time_str.bold()
    );
}

pub fn status_line(action: &str, details: &str) {
    println!("{:>12} {}", action.bold().green(), details);
}
