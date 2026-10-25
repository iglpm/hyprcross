# cargo-hyprcross

Cargo subcommand for [hyprcross](https://crates.io/crates/hyprcross): cross-compile and cross-test Rust with Igel or Cargo.

## Installation

```sh
cargo install cargo-hyprcross
```

## Usage

```sh
# Cross-compile for 64-bit Arm Linux
cargo hyprcross build --target aarch64-unknown-linux-gnu

# Cross-compile for macOS Apple Silicon
cargo hyprcross build --target aarch64-apple-darwin --release

# Cross-test under QEMU
cargo hyprcross test --target aarch64-unknown-linux-gnu
```
