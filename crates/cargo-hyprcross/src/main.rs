//! `cargo hyprcross …`: Cargo runs this as `cargo-hyprcross hyprcross …`.

fn main() {
    hyprcross::crash::init();
    let code = match hyprcross::run() {
        Ok(code) => code,
        Err(err) => {
            hyprcross::ui::fatal(&err);
            1
        }
    };
    std::process::exit(code);
}
