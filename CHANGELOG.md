# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-14

### Added
- Initial release of **Hyprcross**, the accelerated cross-compilation and cross-testing tool.
- **hyprlnk Linker Acceleration**: Built-in 2x-4x faster linking on Linux ELF (x86_64, aarch64, armv7, riscv64, powerpc64le, s390x), Windows PE/COFF, and macOS Mach-O.
- **Zero-Container Direct Mode**: Direct compilation for musl, bare-metal, and native cross targets without starting any container.
- **Persistent Container Daemon**: Background warm container pool executing commands in ~20ms via `exec`, eliminating container startup latency.
- **Dual Builder Support**: Native integration with `rune` (content-addressable caching, pipelined compilation) with seamless fallback to `cargo`.
- **QEMU Emulation & Cross-Testing**: Transparent test runner configuration with automatic `QEMU_LD_PREFIX` sysroot dynamic linker setup and kernel `binfmt_misc` integration.
- **Configuration Compatibility**: Drop-in support for existing `Cross.toml` and `Cargo.toml [package.metadata.cross]`, extended with `Hyprcross.toml`.
- **Diagnostic Suite**: `hyprcross doctor` command verifying host toolchains, container runtimes, QEMU emulators, binfmt_misc status, and installed targets.
- **Cargo Plugin**: Invokable as standalone `hyprcross` or via `cargo hyprcross`.
- **Complete Documentation**: Exhaustive documentation in `docs/` synced automatically to the GitHub Wiki.
