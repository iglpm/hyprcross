# hyprcross images

One image per target, with what building for it needs besides Rust: the target's C and C++
toolchain and system libraries, and, where possible, something to run its programs on (QEMU,
Wine or Wasmtime), so `hyprcross test` works. hyprcross pulls them when a target can't be built
on the host:

```text
ghcr.io/runepm/hyprcross/<target>:latest
```

Rust itself is not in the images: hyprcross mounts the host's `RUSTUP_HOME`, `CARGO_HOME` and
`RUNE_HOME`, at the paths they have on the host, so a container builds with the same toolchain
and shares the same caches.

## Targets

| Target | Toolchain | Tests run under |
| :--- | :--- | :--- |
| **Linux, glibc** | | |
| `aarch64-unknown-linux-gnu` | Linux AArch64 (glibc 2.39) | QEMU aarch64 |
| `arm-unknown-linux-gnueabi` | Linux ARMv6 soft-float (glibc 2.39) | QEMU arm |
| `armv7-unknown-linux-gnueabihf` | Linux ARMv7 hard-float (glibc 2.39) | QEMU arm |
| `i686-unknown-linux-gnu` | Linux x86 (glibc 2.39) | QEMU i386 |
| `loongarch64-unknown-linux-gnu` | Linux LoongArch64 (glibc 2.39) | QEMU loongarch64 |
| `powerpc-unknown-linux-gnu` | Linux PowerPC 32-bit (glibc 2.39) | QEMU ppc |
| `powerpc64-unknown-linux-gnu` | Linux POWER big-endian (glibc 2.39) | QEMU ppc64 |
| `powerpc64le-unknown-linux-gnu` | Linux POWER little-endian (glibc 2.39) | QEMU ppc64le |
| `riscv64gc-unknown-linux-gnu` | Linux RISC-V 64 (glibc 2.39) | QEMU riscv64 |
| `s390x-unknown-linux-gnu` | Linux IBM Z (glibc 2.39) | QEMU s390x |
| `sparc64-unknown-linux-gnu` | Linux SPARC64 (glibc 2.39) | QEMU sparc64 |
| `x86_64-unknown-linux-gnu` | Linux x86-64 (glibc 2.39) | natively |
| **Linux, musl** | | |
| `aarch64-unknown-linux-musl` | Linux AArch64 (musl 1.2.5, static) | QEMU aarch64 |
| `arm-unknown-linux-musleabihf` | Linux ARMv6 hard-float (musl 1.2.5, static) | QEMU arm |
| `armv7-unknown-linux-musleabihf` | Linux ARMv7 hard-float (musl 1.2.5, static) | QEMU arm |
| `i686-unknown-linux-musl` | Linux x86 (musl 1.2.5, static) | QEMU i386 |
| `powerpc64le-unknown-linux-musl` | Linux POWER little-endian (musl 1.2.5, static) | QEMU ppc64le |
| `riscv64gc-unknown-linux-musl` | Linux RISC-V 64 (musl 1.2.5, static) | QEMU riscv64 |
| `x86_64-unknown-linux-musl` | Linux x86-64 (musl 1.2.5, static) | natively |
| **Windows** | | |
| `i686-pc-windows-gnu` | Windows x86 (MinGW-w64, Wine to run tests) | Wine |
| `x86_64-pc-windows-gnu` | Windows x64 (MinGW-w64, Wine to run tests) | Wine |
| **Android** | | |
| `aarch64-linux-android` | Android AArch64 (NDK r27c, API 21+) | — |
| `armv7-linux-androideabi` | Android ARMv7 (NDK r27c, API 21+) | — |
| `i686-linux-android` | Android x86 (NDK r27c, API 21+) | — |
| `x86_64-linux-android` | Android x86-64 (NDK r27c, API 21+) | — |
| **FreeBSD** | | |
| `x86_64-unknown-freebsd` | FreeBSD x86-64 (14.4 sysroot, clang) | — |
| **WebAssembly** | | |
| `wasm32-wasip1` | WebAssembly WASI preview 1 (wasi-sdk, Wasmtime to run tests) | Wasmtime |
| `wasm32-wasip2` | WebAssembly WASI preview 2 (wasi-sdk, Wasmtime to run tests) | Wasmtime |
| **Bare metal** | | |
| `thumbv6m-none-eabi` | Bare-metal Cortex-M0/M0+/M1 (Arm GNU toolchain, newlib) | — |
| `thumbv7em-none-eabihf` | Bare-metal Cortex-M4F/M7F (Arm GNU toolchain, newlib) | — |

Targets without a runner build and link; their programs need the real system (or an emulator
of a whole machine) to run. Apple and MSVC targets have no image: their SDKs can't be
redistributed. hyprcross builds them on the host, or in an image named in `Hyprcross.toml`.

All images are based on Ubuntu 24.04, so glibc targets link against glibc 2.39: programs built
in them need glibc 2.39 or later. musl targets link statically and run anywhere.

## Tags

| Tag | What |
| :--- | :--- |
| `latest` | The newest build from `main`. hyprcross uses it by default (`image_tag`). |
| `<YYYYMMDD>` | The build of that day, kept for pinning. |
| `sha-<commit>` | The build of that commit of `main`. |
| `<version>`, `<major>.<minor>` | The build of a hyprcross release. |

## How the images stay current

- **Every Monday** the [Images workflow](../.github/workflows/images.yml) rebuilds every image
  with that week's Ubuntu package updates (the `HYPRCROSS_REFRESH` build argument changes
  weekly), tests it and pushes it as `latest` and `<date>`.
- **Every push to `main`** that changes `docker/` rebuilds the images it reaches: a Dockerfile
  its own image, a script every image whose Dockerfile copies it (`lib.sh` and `common.sh`:
  all), the smoke test or the workflow all of them.
- **The pinned tools** (wasi-sdk, Wasmtime, musl-cross-make) are moved to their newest releases
  by the [Image updates workflow](../.github/workflows/images-update.yml), as a pull request
  that merges itself once its image builds and smoke tests pass. The Android NDK (an LTS line),
  the FreeBSD sysroot (the oldest supported release) and the Ubuntu base (whose glibc programs
  built in the images need) are moved by hand.
- **Pull requests** build and test the images their changes reach, without pushing them.

Before an image is pushed, CI builds and tests [test/smoke](test/smoke) in it with hyprcross
itself: a C file compiled by the image's toolchain, linked into a Rust library, and its test run
under the image's runner (targets without one only build).

The musl toolchains are built from source (musl-cross-make) in a stage of their own, so the
weekly refresh reuses them until `scripts/musl.sh` or the base image changes. Layers are cached
in the registry, as `<image>:buildcache`.

The repository and package settings this needs (secrets, public packages) are in
[MAINTAINING.md](../MAINTAINING.md).

## Building and testing one locally

```sh
cd docker
docker build -f Dockerfile.aarch64-unknown-linux-gnu \
    -t localhost/hyprcross/aarch64-unknown-linux-gnu:latest .

# Build and test the smoke crate in it with your hyprcross:
cd test/smoke
HYPRCROSS_IMAGE_REPOSITORY=localhost/hyprcross \
    hyprcross test -t aarch64-unknown-linux-gnu --engine container
```

`podman build` works too. (Rootless Podman leaves a `/run/user/<uid>` owned by root in the
image; hyprcross mounts an empty `/run/user` over it, so Wine and others don't trip over it.)

## Layout

| Path | What |
| :--- | :--- |
| `Dockerfile.<target>` | One image. `# hyprcross:platforms` says what CI builds it for; `# hyprcross:smoke` whether CI runs its tests (`test`) or only builds (`build`). |
| `scripts/common.sh` | What every image has: the native toolchain (build scripts run on the build machine), CMake, Perl, pkg-config, autotools. |
| `scripts/qemu.sh` | QEMU user-mode emulation for one architecture. |
| `scripts/multiarch.sh` | Lets `apt-get install libfoo-dev:<arch>` (in a `pre_build` step) fetch the target's libraries. |
| `scripts/musl.sh` | A musl cross toolchain, from source. |
| `scripts/wine.sh`, `android-ndk.sh`, `freebsd.sh`, `wasi.sh` | Wine, the Android NDK, a FreeBSD sysroot, wasi-sdk and Wasmtime. |
| `scripts/hyprcross-runner` | The images' `CARGO_TARGET_<TRIPLE>_RUNNER`: runs a target program under QEMU, Wine or Wasmtime. |
| `matrix.sh` | The CI matrix, from the Dockerfiles. |
| `update-pins.sh` | Moves the pinned upstream tools to their newest releases. |
| `test/smoke` | What CI builds and tests in every image. |

## Adding a target

1. Write `Dockerfile.<target>` (start from one for a similar target), with the
   `# hyprcross:platforms` and `# hyprcross:smoke` lines and the `dev.hyprcross.target` label.
2. Add the target to `IMAGE_TARGETS` in [src/targets.rs](../src/targets.rs); a test checks that
   the two lists agree.
3. Build and smoke-test it locally as above. CI picks the new Dockerfile up by itself.

Every image sets `CARGO_TARGET_<TRIPLE>_LINKER` and `CC_<triple>`, `CXX_<triple>` and
`AR_<triple>` to its toolchain, and `CARGO_TARGET_<TRIPLE>_RUNNER` when it can run the target's
programs. Cargo and Rune read the linker and runner, cc-rs the rest. Where hyprlnk links the
target, Rune makes the image's linker the C driver of its built-in hyprlnk.
