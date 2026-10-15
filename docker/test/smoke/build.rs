fn main() {
    println!("cargo:rerun-if-changed=src/answer.c");
    cc::Build::new().file("src/answer.c").compile("answer");
}
