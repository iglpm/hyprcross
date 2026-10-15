# Architecture

hyprcross decides, per target, whether a build can run on the host or needs a container; runs
the builder (Rune or Cargo) there with the target's linker and test runner; and keeps
containers warm so the second build costs what it would on the host.

```text
hyprcross build -t <target>
        │
        ├─ host can build it ──────────► direct: rune/cargo on the host
        │  (own target, no C needed,        linker = the target's C compiler
        │   C cross compiler installed)     runner = QEMU / Wine / Wasmtime
        │
        └─ otherwise ──────────────────► container: rune/cargo in the target's image
                                            warm container, agent over a Unix socket
```

## Choosing where to build

With `engine = "auto"` (the default), a target builds on the host when

- it is the host's own target, or the same OS family (macOS on macOS, MSVC on Windows),
- it needs no C toolchain to link (bare metal, `wasm32-*`): Rust's own `rust-lld` links it, or
- its C cross compiler is on PATH (`aarch64-linux-gnu-gcc`, `x86_64-w64-mingw32-gcc`, or what
  `CC_<triple>` / `CARGO_TARGET_<TRIPLE>_LINKER` names),

and in a container otherwise, if there is an image for it and a container engine to run it.
`hyprcross targets` shows the decision for every target with an image; `--engine` or the
target's `engine` setting overrides it.

## Direct builds

The builder runs on the host with `--target`. For a cross target hyprcross names the C cross
compiler as `CARGO_TARGET_<TRIPLE>_LINKER` (unless it is set already): without it rustc would
link with the host's `cc`. For `test`, `run` and `bench` it sets `CARGO_TARGET_<TRIPLE>_RUNNER`
to QEMU (with the target's sysroot as `-L`), Wine or Wasmtime when they are installed.

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
