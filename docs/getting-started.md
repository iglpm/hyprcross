# Getting started

## Install

### Option 1: Via Cargo (Crates.io)

Install both the standalone `hyprcross` CLI and the `cargo hyprcross` subcommand:

```sh
# Stable release:
cargo install --locked hyprcross cargo-hyprcross

# Or a specific pre-release version:
cargo install --locked hyprcross cargo-hyprcross --version 2027.0.0-rc.1
```

### Option 2: Pre-compiled Binaries

Standalone release archives for Linux, macOS and Windows (with SHA-256 checksums) are on the
[GitHub Releases page](https://github.com/runepm/hyprcross/releases).

### Option 3: From Source

hyprcross builds with Rust 1.89 or newer. It links with hyprlnk, a submodule, so clone it with
`--recursive` (or run `git submodule update --init` in a clone made without it):

```sh
git clone --recursive https://github.com/runepm/hyprcross
cd hyprcross
cargo install --locked --path . -p hyprcross -p cargo-hyprcross
```

That provides `hyprcross` and `cargo-hyprcross`, so both `hyprcross build` and
`cargo hyprcross build` work.

For targets your machine can't build itself, install Podman or Docker. For running tests of other
architectures on the host, QEMU user-mode emulation (`qemu-user-static`); the images have their
own.

## Check the machine

```console
$ hyprcross doctor
hyprcross doctor · host x86_64-unknown-linux-gnu
  ✓ rustc 1.98.1 (48a229cea 2026-09-01), with 26 standard libraries
  ✓ rune 2027.0.0 builds (content-addressed cache shared with containers)
  ✓ hyprlnk 2027.0.0 links ELF, Mach-O and PE/COFF (built in)
  ✓ podman 6.1.4 runs build containers (warm containers answer over a socket)
  ✓ QEMU runs aarch64, arm, riscv64 programs (binfmt_misc on)
  · no cross C compilers here: Linux, Windows and Android targets build in containers

✓ ready to cross-compile
```

`hyprcross targets` lists the targets with images and where each would build.

## Build

In a Rust project:

```sh
hyprcross build -t aarch64-unknown-linux-gnu
```

```console
▸ building aarch64-unknown-linux-gnu · container · rune · hyprlnk
▸ pulled ghcr.io/runepm/hyprcross/aarch64-unknown-linux-gnu:latest (21.4s)
▸ started container hyprcross-app-aarch64-unknown-linux-gnu-rune-3f2a1c (0.31s)
✓ dev · 12 compiled · 0 cached · 4.12s
```

The first build for a target pulls its image and starts a container; later builds go straight
to the warm container. Options for the builder go anywhere: `hyprcross build --release -t
aarch64-unknown-linux-gnu -p app`. Arguments after `--` go to the builder unread.

## Test and run

```sh
hyprcross test -t aarch64-unknown-linux-gnu        # under QEMU
hyprcross test -t x86_64-pc-windows-gnu            # under Wine
hyprcross run  -t aarch64-unknown-linux-gnu -- --help
```

## Next

- Set a `default_target` and per-target settings: `hyprcross init`, then
  [configuration](configuration.md).
- How builds are placed and run: [architecture](architecture.md).
- What the images contain: [docker/README.md](../docker/README.md).
