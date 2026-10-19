# Architecture

hyprcross cross-compiles directly on the host using lightweight native SDK / sysroot packages and
links natively with **hyprlnk**. Containers are no longer required for ordinary cross-compilation,
eliminating the need for Docker or Podman on Windows, Linux, and macOS. When containerized builds are
explicitly requested (or when custom Dockerfiles are configured), hyprcross uses warm containers with
a Unix socket agent.

```text
hyprcross build -t <target>
        │
        ├─ native direct (default) ────► direct: rune/cargo on host
        │  (uses hyprlnk, auto-downloads   passes --sysroot, SDKROOT,
        │   lightweight SDK if missing)     runner = QEMU / Wine / Wasmtime
        │
        └─ container requested ────────► container: rune/cargo in image
           (--engine container, or          warm container, agent over Unix socket
            custom image / dockerfile)
```

## Choosing where to build

With `engine = "auto"` (the default), builds run **directly on the host**:

- All supported targets (Linux glibc/musl on x86_64, AArch64, ARM, RISC-V, etc., macOS Darwin, Windows MSVC/MinGW)
  are compiled using rustc and linked directly via **hyprlnk**.
- Target SDKs / sysroots are checked at `~/.local/share/hyprcross/sdks/<target>`. If missing, hyprcross
  downloads and unpacks the minimal SDK package on demand automatically.
- Tests, runs, and benchmarks invoke host-native runners: QEMU (`-L <sdk_sysroot>`) for Linux, Wine
  for Windows, or Wasmtime for WASI.

Containers are only chosen if:
- `--engine container` is explicitly passed on the command line, or
- `image` or `dockerfile` is configured in `Hyprcross.toml`.

## Direct builds & native SDKs

The builder runs on the host with `--target`. hyprcross configures:
- `CARGO_TARGET_<TRIPLE>_LINKER = "hyprlnk"` (or hyprcross' built-in link driver)
- `RUSTFLAGS`: sets `-Clinker-flavor` (`ld.lld` for Linux ELF, `ld64.lld` for macOS Mach-O, `lld-link` for Windows PE/COFF) and `--sysroot=<sdk_path>`
- `SDKROOT` / `HYPRCROSS_SDK_PATH`: points to the target SDK directory
- `CARGO_TARGET_<TRIPLE>_RUNNER`: sets QEMU (pointing to the SDK sysroot via `-L`), Wine, or Wasmtime if installed on the host.

A missing standard library is installed with `rustup target add` first. Whether it is missing
is read from the toolchain's directory, so a build that needs nothing installed starts no
`rustup` process.

## Containers

### What a container gets

| Mounted at the same path | Why |
| :--- | :--- |
| the workspace root (the nearest `[workspace]` above the current directory) | sources and the target directory |
| path dependencies outside it (`path = "../lib"`) | so they resolve as on the host |
| `CARGO_HOME`, `RUSTUP_HOME` | the host's toolchain and registry cache |
| `RUNE_HOME` (with Rune) | Rune's shared build cache |

Because everything sits where it does on the host, compiler errors carry host paths, and
Rune's cache keys (which include paths) match between host and container builds.

hyprcross' own executable is mounted read-only at `/opt/hyprcross/bin/hyprcross` (the agent)
and `/opt/hyprcross/bin/hyprlnk` (the linker for Cargo); Rune's at `/opt/hyprcross/bin/rune`.
`/opt/hyprcross/bin` and `$CARGO_HOME/bin` come first on the image's own `PATH`. Containers run
as the invoking user (`--userns=keep-id` with Podman, `--user` with Docker), so files they write
belong to you.

### Warm containers and the agent

With `daemon = true` (the default) each project, target and builder gets one long-lived
container, `hyprcross-<project>-<target>-<builder>-<hash>`. Its main process is hyprcross
itself, as an agent listening on a Unix socket in a mounted directory
(`$XDG_RUNTIME_DIR/hyprcross/<id>/agent.sock`). A build:

1. reads the state file next to the socket: the spec the running container was made from,
2. compares it with the spec of this build (image, mounts, executables, `pre_build`), and if
   they match,
3. connects to the socket and sends the command, its environment and working directory.

The agent starts the command (on a pseudo-terminal when hyprcross' output is a terminal, so
progress bars and colours work), streams its output back and sends its exit code. If hyprcross
goes away (Ctrl+C), the agent kills the command's process group. A warm build runs no
`podman`/`docker` command at all; `podman exec` alone costs about 140 ms.

When the spec differs (a new image tag, a rebuilt `rune`, a changed `pre_build`) the container
is replaced. When it has stopped, it is started again. Containers stop by themselves after
`idle_timeout_mins` (15) without a build. Where Unix sockets can't reach into the container
(Docker Desktop on macOS and Windows), or the agent can't run in an image, builds go through
`exec` instead.

`pre_build` commands run once, as root, when the container is created.

### One-off containers

With `--no-daemon` (or `daemon = false`) every build runs in a fresh `run --rm` container.
`pre_build` needs a warm container and is skipped.

## Linking

- **Rune** links with its built-in hyprlnk wherever hyprlnk supports the target, and uses the
  target's configured linker (`CARGO_TARGET_<TRIPLE>_LINKER`, set by every image) as the C
  driver hyprlnk links through. hyprcross passes nothing: Rune's cache stays the same whether
  a build runs through hyprcross or not. `--linker default` passes
  `--config build.linker="default"`.
- **Cargo** gets hyprcross itself as the linker, under the name `hyprlnk` (a link in
  `~/.cache/hyprcross/bin`, whose path doesn't change across updates, so Cargo's fingerprints
  don't either), with `HYPRLNK_CC` naming the C driver.

See [hyprlnk integration](hyprlnk-integration.md).

## Images

Per-target images live in [docker/](../docker/README.md) and are published to
`ghcr.io/runepm/hyprcross/<target>`. When hyprcross' image for a target can't be pulled, it
falls back to cross-rs' (`ghcr.io/cross-rs/<target>:edge`), which works the same way: both set
the standard `CARGO_TARGET_*`/`CC_*` variables hyprcross and the builders read.
