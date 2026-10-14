# Hyprcross Documentation

Welcome to the Hyprcross documentation. Hyprcross is a next-generation, high-performance cross-compilation and cross-testing tool for Rust and Rune.

## Documentation Index

| Guide | Description |
| :--- | :--- |
| [Getting Started](getting-started.md) | Installation, initial setup, and compiling your first cross target |
| [Architecture](architecture.md) | Dual-engine design, direct host execution, and persistent container daemons |
| [hyprlnk Integration](hyprlnk-integration.md) | How the built-in hyprlnk linker achieves 2x–4x faster linking on cross binaries |
| [Configuration Reference](configuration.md) | Complete guide to `Hyprcross.toml`, `Cross.toml` compatibility, and options |
| [Supported Targets](targets.md) | Architecture matrix, musl/glibc tiers, container images, and runners |
| [Performance & Benchmarks](performance.md) | Latency and throughput benchmarks vs standard `cross` |
| [Testing with QEMU](qemu-testing.md) | Cross-testing, QEMU user emulation, binfmt_misc, and `QEMU_LD_PREFIX` |
| [Troubleshooting](troubleshooting.md) | Resolving Docker socket permissions, missing C headers, and runner errors |

## Key Concepts

- **Zero-Container Direct Execution**: For targets like `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, bare-metal, and native cross builds, Hyprcross bypasses container runtimes entirely, delivering 0ms startup latency at native NVMe I/O speeds.
- **Persistent Container Daemons**: When container isolation is required for target sysroots and C/C++ libraries, Hyprcross maintains warm background containers that execute commands in under 20ms via `exec`.
- **hyprlnk Acceleration**: Standard cross builds are often bottlenecked by slow containerized GNU `ld` or `lld`. Hyprcross transparently injects `hyprlnk`, slashing link times by 50% to 75%.
- **Rune & Cargo Dual Engine**: Leverages [Rune](https://runepm.com)'s content-addressable storage (CAS) and pipelined compilation when available, while maintaining 100% standard `cargo` compatibility.
