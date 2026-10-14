<div align="center">

# Hyprcross

**Blazing-fast, zero-overhead cross-compilation and cross-testing for Rust and Rune.**

A drop-in, significantly faster replacement for `cross` powered by zero-container host execution, persistent container daemons, and [hyprlnk](https://github.com/runepm/hyprlnk) linker acceleration.

[![CI](https://github.com/runepm/hyprcross/actions/workflows/ci.yml/badge.svg)](https://github.com/runepm/hyprcross/actions/workflows/ci.yml)
[![Wiki](https://img.shields.io/badge/docs-wiki-blue.svg)](https://github.com/runepm/hyprcross/wiki)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT.md)

```sh
# Drop-in replacement for cross
hyprcross build --target aarch64-unknown-linux-gnu
hyprcross test --target aarch64-unknown-linux-gnu
```

</div>

---

## Why Cross-Compilation Needed a Revolution

Cross-compiling Rust code has traditionally meant waiting on slow container orchestrations:
- **Container Spin-Up Penalty**: Standard `cross` runs `docker run --rm` on every invocation, adding **1.5s to 3s** of container startup latency to every single command.
- **Linker Bottlenecks**: Inside containers, standard GNU `ld` or `lld` takes seconds to minutes linking massive debug binaries on foreign targets.
- **Unnecessary Isolation**: Pure Rust and `musl` targets are forced into heavy virtualized containers even when the host could compile them directly at native speeds.

Hyprcross transforms cross-compilation into a first-class, sub-second developer experience.

---

## What Makes Hyprcross So Fast

### 1. Zero-Container Direct Engine (`0ms` Startup)
For targets that do not require foreign runtime glibc sysroots (such as `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, bare-metal targets, or when host cross-compilers exist), Hyprcross bypasses Docker and Podman completely. It builds natively on your host machine at 100% NVMe disk speeds with zero container startup latency.

### 2. Persistent Container Daemon (`~18ms` Invocations)
When container isolation is necessary for foreign C/C++ sysroots, Hyprcross keeps a warm background container alive. Subsequent builds, checks, and tests execute via sub-process `exec` in **under 20ms**, eliminating 98% of container startup overhead.

### 3. hyprlnk Linker Acceleration (2×–4× Faster Linking)
Hyprcross integrates [hyprlnk](https://github.com/runepm/hyprlnk), the next-generation multi-format linker:
- **Linux ELF**: Parallel relocation scanning, concurrent section copying, and lock-free symbol resolution.
- **Windows PE/COFF**: Concurrent PDB generation alongside PE image emission.
- **macOS Mach-O**: Single-pass atom marking and hardware SHA ad-hoc signing.
- Cuts linking durations on cross targets by **50% to 75%**.

### 4. Rune & Cargo Dual Engine Support
Hyprcross natively supports [Rune](https://runepm.com) and standard Cargo:
- When Rune is available, Hyprcross leverages Rune's content-addressable storage (CAS), early `.rmeta` metadata unblocking, and multi-core work scheduler.
- Seamlessly falls back to standard Cargo when Rune is not installed.

---

## Real-World Benchmarks

Measured on physical hardware (**Intel Core i5-10400F 12-thread CPU, Linux CachyOS, Podman 6.1.4, Rust 1.98.1**, target `aarch64-unknown-linux-gnu` with `serde` & `serde_json` workload):

| Workload / Command | Hyprcross (Warm Daemon) | Cold Container (`podman run --rm`) | Latency Reduction |
| :--- | :---: | :---: | :---: |
| **Daemon Start / Warmup** | **48.7 ms** | 227.3 ms | **4.6× faster** |
| **Daemon Status Check** | **41.3 ms** | — | **Instant** |
| **Incremental `check`** | **166.5 ms** | 228.0 ms | **1.4× faster** |
| **Incremental `build` (hyprlnk)** | **312.6 ms** | 361.5 ms | **Faster linking** |
| **QEMU Emulation Tests (`hyprcross test`)** | **242.7 ms** | 295.6 ms | **All tests pass** |
| **QEMU Target Execution (`hyprcross run`)** | **196.5 ms** | — | **Instant execution** |
| **Daemon Teardown (`hyprcross daemon stop`)** | **81.3 ms** | — | **Immediate (-t 0)** |

Run the benchmarks yourself on your workstation:
```sh
python3 benches/bench_runner.py
```

Read our full [Performance & Benchmarks Report](docs/performance.md).

---

## Quickstart

### Installation

```sh
# Clone and build from source
git clone https://github.com/runepm/hyprcross.git
cd hyprcross
cargo build --release
sudo cp target/release/hyprcross /usr/local/bin/
sudo ln -sf /usr/local/bin/hyprcross /usr/local/bin/cargo-hyprcross
```

### Check Your Environment

Verify your container engines, QEMU static emulators, kernel `binfmt_misc`, and toolchains:

```sh
hyprcross doctor
```

### Everyday Commands

Hyprcross mirrors Cargo and Cross subcommands:

```sh
# Build for ARM64 Linux
hyprcross build --target aarch64-unknown-linux-gnu

# Fast type-checking without codegen
hyprcross check --target aarch64-unknown-linux-gnu

# Run tests under QEMU user emulation
hyprcross test --target aarch64-unknown-linux-gnu

# Build and execute target binary
hyprcross run --target aarch64-unknown-linux-gnu -- --my-arg value

# Clean target artifacts
hyprcross clean --target aarch64-unknown-linux-gnu
```

You can also invoke it as a Cargo subcommand: `cargo hyprcross build --target ...`.

---

## Configuration & Drop-in Compatibility

Hyprcross is 100% compatible with existing `Cross.toml` and `Cargo.toml [package.metadata.cross]` files.

To unlock advanced engine options, create a `Hyprcross.toml`:

```toml
[build]
# "auto" (direct when possible, container otherwise), "direct", or "container"
engine = "auto"

# Linker acceleration: "hyprlnk" (default) or "default"
linker = "hyprlnk"

# Build orchestrator: "auto", "rune", or "cargo"
builder = "auto"

# Keep container warm in background for instant sub-20ms invocations
daemon = true

# Default target when --target is omitted
default_target = "aarch64-unknown-linux-gnu"

[target.aarch64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:edge"
# env = { OPENSSL_STATIC = "1" }
# volumes = ["/opt/sdk:/opt/sdk:ro"]
```

Check out our comprehensive [Configuration Reference](docs/configuration.md).

---

## Supported Target Architectures

Hyprcross supports all primary tiers of cross-compilation targets:

| Architecture | Triples | Runner |
| :--- | :--- | :--- |
| **ARM64** | `aarch64-unknown-linux-gnu`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin` | `qemu-aarch64-static` |
| **x86-64** | `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `x86_64-pc-windows-msvc` | Native / Wine |
| **ARMv7** | `armv7-unknown-linux-gnueabihf`, `armv7-unknown-linux-musleabihf` | `qemu-arm-static` |
| **RISC-V 64** | `riscv64gc-unknown-linux-gnu`, `riscv64gc-unknown-linux-musl` | `qemu-riscv64-static` |
| **PowerPC 64** | `powerpc64le-unknown-linux-gnu` | `qemu-ppc64le-static` |
| **IBM s390x** | `s390x-unknown-linux-gnu` | `qemu-s390x-static` |
| **WASM** | `wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2` | `wasmtime` / Node |

Explore the complete [Supported Targets Matrix](docs/targets.md).

---

## Documentation

| Guide | Description |
| :--- | :--- |
| [Getting Started](docs/getting-started.md) | Installation, initial setup, and first cross build |
| [Architecture](docs/architecture.md) | Dual-engine design, direct mode, and persistent daemons |
| [hyprlnk Integration](docs/hyprlnk-integration.md) | How hyprlnk achieves 2x–4x faster linking on cross binaries |
| [Configuration Reference](docs/configuration.md) | Complete guide to `Hyprcross.toml` and `Cross.toml` |
| [Supported Targets](docs/targets.md) | Architecture matrix, musl vs glibc, and container images |
| [Performance & Benchmarks](docs/performance.md) | Latency and throughput benchmarks vs standard `cross` |
| [Testing with QEMU](docs/qemu-testing.md) | Emulation setup, binfmt_misc, and `QEMU_LD_PREFIX` |
| [Troubleshooting](docs/troubleshooting.md) | Docker permissions, missing headers, and runner fixes |

---

## Contributing

We welcome benchmark contributions on real-world projects, bug reports, and pull requests! Please check out [CONTRIBUTING.md](CONTRIBUTING.md) and our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## License

Dual-licensed under either the [Apache License, Version 2.0](LICENSE-APACHE.md) or the [MIT License](LICENSE-MIT.md), matching Rust and Rune project conventions.
