# Contributing to Hyprcross

Thank you for your interest in making cross-compilation faster, lighter, and more enjoyable for the Rust community.

We welcome bug reports, pull requests, performance benchmarks on real-world projects, sysroot additions, and documentation enhancements.

Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before participating.

---

## Developing Hyprcross

Hyprcross is written in modern Rust (2024 edition). It can be developed standalone or inside the [Rune](https://github.com/runepm/rune) ecosystem workspace.

### Prerequisites

- A current Rust toolchain (`rustup`)
- Either `rune` or `cargo`
- (Optional, for container testing) `docker` or `podman`
- (Optional, for emulation testing) `qemu-user-static`

### Building and Testing Locally

```sh
# Clone the repository
git clone git@github.com:runepm/hyprcross.git
cd hyprcross

# Compile in debug mode
cargo build

# Run test suite
cargo test

# Check system health with doctor
cargo run -- doctor
```

---

## Architecture Tour

When hacking on Hyprcross, here is how the codebase is organized:

- **`src/cli.rs`**: Command-line arguments parsed via `clap`.
- **`src/config.rs`**: Config discovery for `Hyprcross.toml`, `Cross.toml`, and `Cargo.toml [package.metadata.hyprcross]`.
- **`src/targets.rs`**: Target architecture mapping, default container images, QEMU runner mapping, and musl/glibc detection.
- **`src/hyprlnk_driver.rs`**: Direct integration with the `hyprlnk` high-performance linker (COFF, ELF, Mach-O).
- **`src/direct.rs`**: Zero-container direct host compilation engine for musl, bare-metal, and native cross builds.
- **`src/container.rs`**: Container engine runner with user permission preservation and volume mounts.
- **`src/daemon.rs`**: Persistent warm container daemon executing commands via `exec` in ~20ms.
- **`src/qemu.rs`**: Cross-runner resolution and `QEMU_LD_PREFIX` dynamic library prefixing.
- **`src/doctor.rs`**: Diagnostic health check examining container runtimes, QEMU, binfmt_misc, and toolchains.
- **`src/builder.rs`**: Build orchestrator abstraction bridging `rune` and `cargo`.
- **`docs/`**: Documentation synced automatically to the GitHub Wiki on push.

---

## Reporting Issues

If a build fails under Hyprcross or behaves differently than native builds:

1. Run `hyprcross doctor` to verify your environment.
2. Run your build with `-v` or `--verbose` to inspect the exact container or host commands.
3. Open an issue on GitHub with:
   - Host OS and architecture (`hyprcross doctor` output)
   - Target triple (e.g., `aarch64-unknown-linux-gnu`)
   - Exact command invoked
   - Reproducible `Cargo.toml` or minimal example repo

---

## Coding Guidelines

- Keep code clean, idiomatic, and documented.
- Maintain fast build and link times: avoid heavy unnecessary dependencies.
- Ensure all tests pass (`cargo test`) before submitting a pull request.
- Ensure `cargo clippy` and `cargo fmt` pass cleanly.
