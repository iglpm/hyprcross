# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v2027.0.0-rc.2] - 2026-10-20

### Removed
- **Container Execution Subsystem**: Fully ditched Docker, Podman, Finch, and container daemon workflows. Removed `src/container.rs`, `src/daemon.rs`, `src/agent.rs`, and container-specific CLI subcommands (`hyprcross daemon`, `hyprcross images`, `--daemon`, `--no-daemon`).
- **Container Build Overhead**: Removed container image downloads, Unix socket agent communication, and guest mounts in favor of 100% native host execution with target SDKs and sysroots.

### Added
- **Native Host SDK Cross-Compilation Architecture**: Replaced container builds with lightweight target SDKs (~15–30 MB) automatically fetched from `runepm/sdks` releases and cached in `~/.local/share/hyprcross/sdks/`.
- **Universal 33-Target Native Support**: Full native compilation and linking across 33 targets:
  - Linux GNU (x86_64, i686, aarch64, armv7, arm, riscv64gc, powerpc64le, powerpc, s390x, loongarch64)
  - Linux musl (x86_64, i686, aarch64, armv7, arm, riscv64gc, powerpc64le)
  - Windows MSVC (x86_64, aarch64, i686)
  - Windows GNU (x86_64, i686) & Windows GnuLLVM (x86_64, aarch64)
  - Android (aarch64, armv7, x86_64, i686)
  - FreeBSD (x86_64)
  - Apple macOS (aarch64, x86_64)
- **Rune-Aligned Progress Bar UI**: Replaced generic download/spinner outputs with styled progress bars and spinners matching `rune_scheduler` (`━╾─` characters, braille spinners, violet brand accents).
- **Universal Clang/LLD Linker Driver Fallback**: For all non-host targets when no host-native cross GCC compiler is detected (e.g. `aarch64-linux-gnu-gcc`), `hyprcross` automatically configures `clang` as the linker driver and sets `-fuse-ld=lld` along with normalized target triples (e.g. `riscv64-unknown-linux-*`, `x86_64-w64-mingw32`, `x86_64-w64-windows-gnu`).
- **Comprehensive Target Sysroot Resolution**: Added automated library search path injection (`-L<sdk>/usr/lib`, `-L<sdk>/lib`, and `-L<sdk>/usr/lib/21` for Android) across Linux GNU, Linux musl, Android, FreeBSD, and Windows targets.

### Changed
- **Engine Simplification**: Simplified `Engine` and `ExecutionContext` to direct execution only, removing container branching.
- **Direct Mode Driver Orchestration**: Set `HYPRLNK_CC` to the detected Clang driver when no target-specific GCC binary is present, enabling `hyprlnk` to invoke the correct compiler driver during direct cross-linking.
- **MSVC Environment Resolution**: Enhanced `DirectEngine` to dynamically search both `xwin` splat directories (`crt/lib/<arch>`, `sdk/lib/um/<arch>`, `sdk/lib/ucrt/<arch>`) and flat layouts, populating both `/LIBPATH` arguments and the `LIB` environment variable.
- **Doctor Diagnostics**: Reworked `hyprcross doctor` to report installed and available target SDKs, QEMU/Wine/Wasmtime emulators, and host C/C++ cross-compilers instead of container engine daemons.

### Fixed
- **Symlink Preservation During SDK Extraction**: Fixed `copy_dir_recursive` in `sdk.rs` crashing on broken relative symlinks by explicitly creating symbolic links (`std::os::unix::fs::symlink`) instead of attempting direct file copy operations.
- **GNU ld Script Absolute Host Paths**: Sanitized Debian/Ubuntu glibc `libc.so` and `libm.so` linker scripts by stripping hardcoded host `/usr/<arch>/lib/` and `/lib/<arch>/` paths in SDK sysroots.
- **Android NDK Startfiles & Runtime Libs**: Configured relative symlinks for Android API 21+ startfiles (`crtbegin_dynamic.o`, `crtend_android.o`) and packaged `libunwind.a` and compiler builtins directly in the sysroot root.
- **Windows gnullvm Runtime Linking**: Packaged `llvm-mingw` for `x86_64-pc-windows-gnullvm` and added `libunwind.a` compatibility symlinks for `-lgcc` / `-lgcc_eh`.
- **Linux musl GCC Runtime Objects**: Extracted `libgcc.a` and `crtbegin*.o` / `crtend*.o` into musl sysroots to support direct non-container cross-linking.


## [0.2.0] - 2026-10-15


### Added
- **Images of its own**, one per target, in `docker/`: Linux on glibc 2.39 for 12
  architectures and on musl 1.2.5 for 7, Windows (MinGW-w64, with Wine to run tests),
  Android (NDK r27c), FreeBSD 14.3, WASI (with Wasmtime) and bare-metal Arm; 30 in all.
  Published to `ghcr.io/runepm/hyprcross/<target>` by the Images workflow, which rebuilds them
  every week with the base system's updates, and builds and tests a smoke crate in each with
  hyprcross before pushing. The Image updates workflow moves the pinned upstream tools to their
  newest releases as pull requests. Targets without an image fall back to cross-rs'.
- **An agent in warm containers**: the container's main process is hyprcross, which runs builds
  sent over a Unix socket, on a pseudo-terminal when the output is a terminal. A warm build
  starts no `podman` or `docker` process: 41 ms for a fully cached build of hyprcross in a
  container, against 149 ms through `podman exec`.
- Built into Rune as `rune cross` and `--cross`: `run_hosted` runs hyprcross inside another
  program, which names the command in messages and help, brings itself as the builder, and
  applies its settings over hyprcross' files (`host::Host`). Without the `builtin-hyprlnk`
  feature it brings no hyprlnk of its own, for hosts that link with theirs.
- `hyprcross targets`: the targets with images, how each builds here, what runs its tests.
- `bench` and `doc`; the aliases `b`, `c`, `t`, `r`.
- `image_repository` and `image_tag`; `[build.env]` with `passthrough` and `volumes`;
  `dockerfile` (built locally, with the target's image as `HYPRCROSS_BASE_IMAGE`);
  `HYPRCROSS_CONTAINER_ENGINE`, `HYPRCROSS_IMAGE_REPOSITORY` and `HYPRCROSS_IMAGE_TAG`.
- `--color`, `--quiet` and `--verbose` (which prints every command hyprcross runs).

### Changed
- hyprlnk is a submodule (`hyprlnk/`, pinned to one commit) instead of a repository expected
  next to hyprcross: `git clone --recursive` is all a build needs. Dependabot proposes newer
  hyprlnk commits, and their pull requests smoke-test every image.
- Output in Rune's style: `▸` for what happens, `✓`, `!` and `✗`, `note:` hints, spinners for
  pulls and container starts, a grouped `help`, and a `doctor` that lists what to fix (a debug
  build of `rune` on PATH, for one).
- Containers mount the project (its workspace root), its path dependencies, `CARGO_HOME`,
  `RUSTUP_HOME` and `RUNE_HOME` at their host paths: errors show host paths and Rune's cache is
  shared with host builds. They run with the image's own `PATH`, extended.
- Warm containers are replaced when what they were made from changes (image, mounts, a rebuilt
  `rune` or `hyprcross`, `pre_build`), and stop after `idle_timeout_mins` without a build. Their
  names include a hash of the project's path, so projects in directories of the same name no
  longer share one.
- `engine = "auto"` builds on the host when the target's C cross compiler is installed, and in
  a container otherwise; musl targets no longer always build on the host.
- With Rune, hyprcross passes no linker flags: Rune links with its own hyprlnk, through the
  image's C compiler. With Cargo, hyprcross is the linker under the name `hyprlnk`.
- Direct builds name the target's C cross compiler as its linker, and find QEMU's sysroot where
  distributions install it (`/usr/aarch64-linux-gnu`).
- hyprcross' own options can come anywhere before `--`, mixed with the builder's.
- Configuration is looked for in parent directories too; `Cross.toml`'s kebab-case keys and
  its `[target.*.env]` lists are understood.
- The target's standard library is checked for in the toolchain's directory instead of with
  `rustup target list`.

### Fixed
- Builds linked through hyprlnk failed: `-C link-arg=__hyprlnk` ended up last on the link line
  ("cannot open __hyprlnk").
- Container builds with Rune failed to link host build scripts ("unrecognized command-line
  option '-m64'"): the target's `HYPRLNK_CC` was set for the whole container.
- Running a build through hyprcross and then through `rune` rebuilt everything: hyprcross'
  `RUSTFLAGS` changed Rune's cache keys.
- `pre_build` was never run.
- Wine failed in images built with rootless Podman (`mkdir /run/user/1000/wine: Permission
  denied`).
- CI: hyprlnk is checked out next to hyprcross (`path = "../hyprlnk"`); the wiki sync's
  condition was a syntax error; the release workflow used a retired macOS runner and lacked a
  musl C compiler.

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
