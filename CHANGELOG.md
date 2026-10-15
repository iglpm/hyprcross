# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
