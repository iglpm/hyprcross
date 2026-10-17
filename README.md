<div align="center">

# hyprcross

**Cross-compile and cross-test Rust, with Rune or Cargo.**

On the host when it can, in a warm container when it can't, linked with hyprlnk.
A drop-in for `cross`: same `Cross.toml`, same commands.

[![CI](https://github.com/runepm/hyprcross/actions/workflows/ci.yml/badge.svg)](https://github.com/runepm/hyprcross/actions/workflows/ci.yml)
[![Images](https://github.com/runepm/hyprcross/actions/workflows/images.yml/badge.svg)](https://github.com/runepm/hyprcross/actions/workflows/images.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT.md)

</div>

```console
$ hyprcross test -t aarch64-unknown-linux-gnu
▸ testing aarch64-unknown-linux-gnu · container · rune · hyprlnk
✓ 1 passed · 1 test binary · 1.12s

$ hyprcross build -t aarch64-unknown-linux-gnu
▸ building aarch64-unknown-linux-gnu · container · rune · hyprlnk
✓ dev · 0 compiled · 94 cached · 0.03s
```

## What it does

- **Builds on the host when it can.** The host's own target, macOS targets (`*-apple-darwin`
  via zero-Docker minimal SDK bootstrapping and hyprlnk), targets that need no C toolchain
  (bare metal, WebAssembly), and targets whose C cross compiler is installed build directly:
  hyprcross names the compiler as the linker and QEMU, Wine or Wasmtime as the test runner.

- **Builds in a container when it can't.** Each target has an image with its toolchain and an
  emulator for its tests ([30 of them](docker/README.md)). The project, `CARGO_HOME`,
  `RUSTUP_HOME` and `RUNE_HOME` are mounted at their host paths, so the container builds with
  the host's toolchain, shares its caches, and reports errors with host paths.
- **Keeps the container warm.** The first build starts it; hyprcross' agent inside it then runs
  every build over a Unix socket, without asking Docker or Podman anything. A fully cached
  build in a container takes as long as one on the host.
- **Links with hyprlnk.** Rune links with its built-in hyprlnk; with Cargo, hyprcross is the
  linker. Linux (glibc and musl), Windows MSVC and macOS targets.
- **Runs Rune or Cargo.** Rune when it is installed, with its shared, content-addressed cache;
  Cargo otherwise.

## How fast

A fully cached build of hyprcross itself (`build` with nothing to do), Rune 2027.0 and
hyprcross release builds, Podman 6.1, Intel i5-10400F, median of 15 runs:

| | Time |
| :--- | ---: |
| `rune build`, host target, without hyprcross | 39 ms |
| `hyprcross build`, host target | 40 ms |
| `hyprcross build -t aarch64-unknown-linux-gnu`, warm container | **41 ms** |
| the same through `podman exec`, as hyprcross 0.1 did | 149 ms |

Before this release a cached build in a container took over 1.5 s: Rune copied and re-hashed
every artifact on each build there, because its cache can't hard-link across the container's
mounts.

How it's measured, and what changed: [docs/performance.md](docs/performance.md).

## Install

**With Rune, there is nothing to install:** hyprcross is built into it, as `rune cross` and
`rune build --cross <target>` (see Rune's
[cross-compilation guide](https://github.com/runepm/rune/blob/master/docs/cross-compilation.md)).
On its own, for Cargo:

```sh
git clone --recursive https://github.com/runepm/hyprcross   # with hyprlnk, a submodule
cd hyprcross
cargo install --path .
```

This installs `hyprcross` and `cargo-hyprcross` (for `cargo hyprcross …`). Then:

```sh
hyprcross doctor     # what this machine has for cross-compiling
hyprcross targets    # the targets with images, and how each builds here
```

## Use

```sh
hyprcross build -t aarch64-unknown-linux-gnu             # build
hyprcross build -t x86_64-pc-windows-gnu --release       # builder options go anywhere
hyprcross test  -t riscv64gc-unknown-linux-gnu           # tests under QEMU
hyprcross test  -t x86_64-pc-windows-gnu                 # tests under Wine
hyprcross run   -t aarch64-unknown-linux-musl -- --flag  # run the binary
hyprcross check -t wasm32-wasip1                         # check, clippy, doc, bench, clean too

hyprcross daemon status                                  # the warm containers
hyprcross images pull -t aarch64-unknown-linux-gnu       # a newer image
```

`--engine direct|container` forces where a build runs, `--builder rune|cargo` what runs it,
`--linker default` keeps the target's own linker, `--no-daemon` uses a fresh container per build.

## Configure

Optional. `Hyprcross.toml` (or `Cross.toml`, or `[package.metadata.cross]`), in the project or
any directory above it; `hyprcross init` writes one.

```toml
[build]
default_target = "aarch64-unknown-linux-gnu"

[target.aarch64-unknown-linux-gnu]
# Libraries for the target, installed once when the warm container is created:
pre_build = ["apt-get update && apt-get install -y libssl-dev:arm64"]
env = { OPENSSL_STATIC = "1" }
```

All settings: [docs/configuration.md](docs/configuration.md).

## Images

`ghcr.io/runepm/hyprcross/<target>`: Linux on glibc and musl for 12 architectures, Windows
(MinGW-w64, with Wine), Android, FreeBSD, WASI (with Wasmtime) and bare-metal Arm. Rebuilt every
week with the base system's updates and tested with hyprcross before they are pushed. See
[docker/README.md](docker/README.md). Targets without an image fall back to cross-rs' images.

## Documentation

| Guide | |
| :--- | :--- |
| [Getting started](docs/getting-started.md) | Installation and the first cross build |
| [Architecture](docs/architecture.md) | Direct builds, warm containers and the agent |
| [Configuration](docs/configuration.md) | `Hyprcross.toml`, `Cross.toml` and environment variables |
| [Targets](docs/targets.md) | What builds where, and what runs the tests |
| [Images](docker/README.md) | The per-target images and how they are maintained |
| [hyprlnk](docs/hyprlnk-integration.md) | How builds are linked |
| [Testing with QEMU](docs/qemu-testing.md) | Running tests of other architectures |
| [Performance](docs/performance.md) | Measurements and method |
| [Troubleshooting](docs/troubleshooting.md) | Common problems and their fixes |

## Contributing

Bug reports, benchmarks on real projects and pull requests are welcome: see
[CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

MIT or Apache-2.0, at your option: [LICENSE-MIT.md](LICENSE-MIT.md),
[LICENSE-APACHE.md](LICENSE-APACHE.md).
