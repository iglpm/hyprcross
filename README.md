<div align="center">

# hyprcross

**Cross-compile and cross-test Rust, natively with Rune or Cargo.**

Directly on the host with target SDKs and sysroots, linked with hyprlnk.
Zero containers, zero VM overhead, zero Docker required.

[![CI](https://github.com/runepm/hyprcross/actions/workflows/ci.yml/badge.svg)](https://github.com/runepm/hyprcross/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT.md)

</div>

```console
$ hyprcross test -t aarch64-unknown-linux-gnu
▸ testing aarch64-unknown-linux-gnu · direct · rune · hyprlnk
✓ 1 passed · 1 test binary · 0.28s

$ hyprcross build -t aarch64-unknown-linux-gnu
▸ building aarch64-unknown-linux-gnu · direct · rune · hyprlnk
✓ dev · 0 compiled · 94 cached · 0.03s
```

## What it does

- **Builds on the host natively without Docker.** hyprcross cross-compiles directly on
  the host using minimal SDK / sysroot packages and links natively with **hyprlnk**. You don't need
  Docker, Podman, or a container daemon installed. Missing SDKs are automatically downloaded on demand
  and cached locally in `~/.local/share/hyprcross/sdks/`.
- **Links with hyprlnk.** Built-in high-performance native linking for Linux (ELF glibc & musl for
  x86_64, AArch64, ARM, RISC-V, PowerPC, s390x, LoongArch), macOS (Mach-O Darwin), and Windows
  (PE/COFF MSVC & MinGW).
- **Fast, minimal SDKs.** Instead of downloading 500 MB – 1 GB container images, hyprcross only fetches
  the minimal linking headers and stub libraries (~15–30 MB), unpacks them once, and reuses them across
  all projects.
- **Manages SDKs directly.** Check status with `hyprcross sdk list`, pre-download toolchains with
  `hyprcross sdk install <target>`, or inspect paths with `hyprcross sdk path <target>`.
- **Runs Rune or Cargo.** Rune when installed, leveraging its content-addressed caching; Cargo otherwise.

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

### From Crates.io (Cargo)

Install both the standalone `hyprcross` CLI and the `cargo hyprcross` subcommand plugin:

```sh
# Stable release:
cargo install --locked hyprcross cargo-hyprcross

# Or a specific pre-release version:
cargo install --locked hyprcross cargo-hyprcross --version 2027.0.0-rc.1
```

### Pre-compiled Binaries

Download pre-built standalone binaries directly from the [GitHub Releases page](https://github.com/runepm/hyprcross/releases) for Linux, macOS, and Windows.

### From Source

```sh
git clone --recursive https://github.com/runepm/hyprcross   # with hyprlnk, a submodule
cd hyprcross
cargo install --locked --path . -p hyprcross -p cargo-hyprcross
```

This provides both `hyprcross` and `cargo-hyprcross` (for `cargo hyprcross …`). Then:

```sh
hyprcross doctor     # what this machine has for cross-compiling
hyprcross targets    # the targets with images, and how each builds here
```

## Use

```sh
hyprcross build -t aarch64-unknown-linux-gnu             # build (downloads SDK automatically if missing)
hyprcross build -t x86_64-pc-windows-gnu --release       # builder options go anywhere
hyprcross test  -t riscv64gc-unknown-linux-gnu           # tests under QEMU
hyprcross test  -t x86_64-pc-windows-gnu                 # tests under Wine
hyprcross run   -t aarch64-unknown-linux-musl -- --flag  # run the binary
hyprcross check -t wasm32-wasip1                         # check, clippy, doc, bench, clean too

# SDK management (Zero Docker!)
hyprcross sdk list                                       # list installed and available SDKs
hyprcross sdk install aarch64-unknown-linux-gnu          # pre-install / pull target SDK
hyprcross sdk path aarch64-unknown-linux-gnu             # show SDK directory
hyprcross sdk remove aarch64-unknown-linux-gnu           # clean up cached SDK
```

`--builder rune|cargo` controls what runs the build, `--linker default` keeps the target's own linker.

## Configure

Optional. `Hyprcross.toml` (or `Cross.toml`, or `[package.metadata.cross]`), in the project or
any directory above it; `hyprcross init` writes one.

```toml
[build]
default_target = "aarch64-unknown-linux-gnu"
linker = "hyprlnk"
builder = "auto"

[target.aarch64-unknown-linux-gnu]
env = { OPENSSL_STATIC = "1" }
```


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
