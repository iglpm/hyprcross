fn main() {
    match hyprcross::run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            hyprcross::ui::error(err);
            std::process::exit(1);
        }
    }
}
