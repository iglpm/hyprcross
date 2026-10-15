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

hyprcross links with [hyprlnk](https://github.com/runepm/hyprlnk), a submodule at `hyprlnk/`
pinned to one commit:

```sh
git clone --recursive git@github.com:runepm/hyprcross.git
cd hyprcross

cargo build
cargo test
cargo run -- doctor
```

To build against another hyprlnk commit, check it out in `hyprlnk/` and commit the submodule
change (`git add hyprlnk`); Dependabot proposes newer ones weekly. In the Rune repository,
hyprcross is a project of its own (excluded from Rune's workspace) with its own copy of hyprlnk:
`git submodule update --init --recursive` there fetches it, and hyprlnk changes made in Rune's
`hyprlnk/` reach hyprcross once they are committed, pushed and checked out in
`hyprcross/hyprlnk/`.

CI runs `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked` and
`cargo test --all-targets --all-features --locked` with warnings as errors, on Linux, macOS and
Windows. Changes to `docker/` are checked with shellcheck, and the images they reach are built
and smoke-tested; see [docker/README.md](docker/README.md) for doing that locally.

---

## Architecture Tour

- **`src/lib.rs`**: Entry point: reads the command line, loads configuration, picks the target, and hands the build to the engine.
- **`src/cli.rs`**, **`src/help.rs`**, **`src/ui.rs`**: Commands and options (`clap`), the grouped help, and output in Rune's style.
- **`src/config.rs`**: `Hyprcross.toml`, `Cross.toml` and `[package.metadata.*]`, found from the current directory upwards.
- **`src/targets.rs`**: Target triples: their toolchain, image, runner, and whether the host can build them.
- **`src/engine.rs`**: Where a build runs (host or container) and running it there.
- **`src/direct.rs`**, **`src/qemu.rs`**: Builds on the host, with the target's C compiler as linker and QEMU, Wine or Wasmtime as runner.
- **`src/container.rs`**, **`src/workspace.rs`**: Container engines, images, and the mounts a build needs (workspace root, path dependencies, toolchain and caches at their host paths).
- **`src/daemon.rs`**, **`src/agent.rs`**: Warm containers, and the agent inside them that runs builds sent over a Unix socket.
- **`src/hyprlnk_driver.rs`**: hyprlnk built in, as the linker for Cargo builds.
- **`src/builder.rs`**: Rune or Cargo, and the options each gets.
- **`src/doctor.rs`**: `hyprcross doctor`.
- **`docker/`**: The per-target images, their scripts and the smoke test CI runs in each.
- **`docs/`**: Documentation, synced to the GitHub Wiki on push (public repository only).

Maintainers: [MAINTAINING.md](MAINTAINING.md) covers the repository's GitHub setup, secrets and releases.

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
