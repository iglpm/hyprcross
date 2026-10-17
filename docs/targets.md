# Targets

Any target rustc knows can be built: hyprcross runs the builder with `--target`. What differs is
where the C toolchain comes from and what runs the tests. `hyprcross targets` lists the targets
with images and how each builds on your machine.

## With images

These have an image at `ghcr.io/runepm/hyprcross/<target>` with their C/C++ toolchain, and most
with a runner for `test` and `run`. The full list, with toolchain versions:
[docker/README.md](../docker/README.md#targets).

| Family | Targets | Tests run under |
| :--- | :--- | :--- |
| Linux, glibc 2.39 | x86-64, x86, AArch64, ARMv7, ARMv6, RISC-V 64, POWER (LE and BE), PowerPC, s390x, SPARC64, LoongArch64 | QEMU (x86-64 natively) |
| Linux, musl 1.2.5 | x86-64, x86, AArch64, ARMv7, ARMv6, RISC-V 64, POWER LE | QEMU (x86-64 natively) |
| Windows, MinGW-w64 | `x86_64-pc-windows-gnu`, `i686-pc-windows-gnu` | Wine |
| Android, NDK r27c | AArch64, ARMv7, x86-64, x86 | build only |
| FreeBSD 14.4 | `x86_64-unknown-freebsd` | build only |
| WASI | `wasm32-wasip1`, `wasm32-wasip2` | Wasmtime |
| Bare-metal Arm | `thumbv6m-none-eabi`, `thumbv7em-none-eabihf` | build only |

hyprlnk links the Linux glibc and musl targets on x86-64, x86, AArch64, ARM, RISC-V 64, POWER LE,
s390x and LoongArch64; the others link with their image's toolchain.

## Without images

| Targets | How |
| :--- | :--- |
| macOS (`*-apple-darwin`) | On a Mac, directly (hyprlnk links them). Apple's SDK can't be put in a public image. |
| Windows MSVC (`*-pc-windows-msvc`) | On Windows directly, or on Linux with an MSVC sysroot (e.g. from `xwin`) and `--engine direct`. Use `*-pc-windows-gnu` for a container. |
| `wasm32-unknown-unknown`, other bare metal | Directly: rust-lld links them. |
| Anything else cross-rs has an image for | In cross-rs' image (`ghcr.io/cross-rs/<target>:edge`). |

## glibc or musl

- **musl** (`*-linux-musl`): static binaries that run on any Linux of the architecture, from
  `FROM scratch` containers to old distributions.
- **glibc** (`*-linux-gnu`): dynamically linked against glibc 2.39 when built in the images, so
  they need a system with glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 and later). For
  older systems, build for musl, or build in an image of your own with an older glibc (`image`
  in `Hyprcross.toml`).
