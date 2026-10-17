# Targets

Any target rustc knows can be built: hyprcross runs the builder with `--target`. What differs is
where the C toolchain comes from and what runs the tests. `hyprcross targets` lists the targets
and how each builds on your machine. You can filter targets by query:

```console
$ hyprcross targets "windows arm"
Targets · 2 available matching `windows arm`
    aarch64-pc-windows-msvc     direct     MSVC · hyprlnk
    aarch64-pc-windows-gnullvm  container  

$ hyprcross targets macos
Targets · 2 available matching `macos`
  ✓ x86_64-apple-darwin   direct     macOS SDK stubs · hyprlnk
  ✓ aarch64-apple-darwin  direct     macOS SDK stubs · hyprlnk
```

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

## Native direct builds (without Docker)

| Targets | How |
| :--- | :--- |
| macOS (`*-apple-darwin`) | Cross-compiled directly on Linux/Windows host without containers. hyprcross automatically bootstraps minimal SDK stubs (cached in `~/.local/share/hyprcross/sdks/MacOSX.sdk`) and links with hyprlnk. |
| Windows MSVC (`*-pc-windows-msvc`) | On Windows directly, or on Linux with hyprlnk / MSVC sysroot. |
| `wasm32-unknown-unknown`, bare metal | Directly: rust-lld links them. |
| Anything else cross-rs has an image for | In cross-rs' image (`ghcr.io/cross-rs/<target>:edge`). |


## glibc or musl

- **musl** (`*-linux-musl`): static binaries that run on any Linux of the architecture, from
  `FROM scratch` containers to old distributions.
- **glibc** (`*-linux-gnu`): dynamically linked against glibc 2.39 when built in the images, so
  they need a system with glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 and later). For
  older systems, build for musl, or build in an image of your own with an older glibc (`image`
  in `Hyprcross.toml`).
