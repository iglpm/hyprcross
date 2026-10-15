# Configuration

Everything is optional: without configuration hyprcross picks the engine, builder, linker and
image per target.

## Where hyprcross looks

From the current directory upwards, the first of these in a directory wins:

1. `Hyprcross.toml`
2. `Cross.toml` (cross-rs' file, read as is)
3. `Cargo.toml`: `[package.metadata.hyprcross]`, `[workspace.metadata.hyprcross]`, or the same
   under `.cross`

Then `~/.config/hyprcross/config.toml`. Keys can be spelled `snake_case` or, as in
`Cross.toml`, `kebab-case`.

`hyprcross init [-t <target>]` writes a commented `Hyprcross.toml`.

## `[build]`

```toml
[build]
# Where builds run: "auto" (on the host when it can build the target, else in a container),
# "direct" or "container".
engine = "auto"

# "hyprlnk" or "default" (the target's own linker).
linker = "hyprlnk"

# "auto" (Rune when installed, else Cargo), "rune" or "cargo".
builder = "auto"

# Keep a warm container per project and target.
daemon = true

# Minutes a warm container waits for the next build before it stops.
idle_timeout_mins = 15

# "auto" (Podman, Docker, Finch, nerdctl: the first installed), or one of them.
container_engine = "auto"

# The target when --target is left out.
default_target = "aarch64-unknown-linux-gnu"

# Images: <image_repository>/<target>:<image_tag>.
image_repository = "ghcr.io/runepm/hyprcross"
image_tag = "latest"

# Variables for every target's builds, and host variables passed into containers.
[build.env]
PKG_CONFIG_ALLOW_CROSS = "1"
passthrough = ["MY_TOKEN"]
volumes = ["DATA_DIR"]   # variables holding host paths: mounted, and passed on
```

## `[target.<triple>]`

```toml
[target.aarch64-unknown-linux-gnu]
# An image of your own instead of the target's:
image = "registry.example.com/aarch64-builder:1"

# Or one built from a Dockerfile (its directory is the context). It gets the target's image as
# the HYPRCROSS_BASE_IMAGE (and CROSS_BASE_IMAGE) build argument:
#   ARG HYPRCROSS_BASE_IMAGE
#   FROM $HYPRCROSS_BASE_IMAGE
#   RUN apt-get update && apt-get install -y libssl-dev:arm64
dockerfile = "docker/Dockerfile.aarch64"

# Shell commands run as root when the warm container is created.
pre_build = ["apt-get update && apt-get install -y libssl-dev:arm64"]

# Variables for this target's builds, and Cross.toml's lists.
env = { OPENSSL_STATIC = "1" }

# Extra mounts, host:container[:options].
volumes = ["/opt/sdk:/opt/sdk:ro"]

# Per-target overrides of [build]:
engine = "container"
linker = "default"

# Direct mode: what runs the target's programs instead of QEMU, and the sysroot QEMU loads
# shared libraries from.
runner = "qemu-aarch64"
sysroot = "/usr/aarch64-linux-gnu"
```

The images enable Ubuntu's multiarch for their architecture, so `apt-get install
libfoo-dev:<arch>` in `pre_build` installs the target's libraries.

## Environment variables

| Variable | Overrides |
| :--- | :--- |
| `HYPRCROSS_TARGET` | `--target` / `default_target` |
| `HYPRCROSS_ENGINE`, `HYPRCROSS_LINKER`, `HYPRCROSS_BUILDER` | `--engine`, `--linker`, `--builder` |
| `HYPRCROSS_CONTAINER_ENGINE` | `container_engine` |
| `HYPRCROSS_IMAGE_REPOSITORY`, `HYPRCROSS_IMAGE_TAG` | `image_repository`, `image_tag` (CI uses these to test images it just built) |
| `NO_COLOR`, `CLICOLOR_FORCE` | colours, like `--color` |

Passed into containers as they are on the host: `RUST_BACKTRACE`, `RUST_LOG`, `RUST_MIN_STACK`,
`RUSTFLAGS`, `RUSTDOCFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_INCREMENTAL`, `CARGO_BUILD_JOBS`,
`CARGO_PROFILE_*_DEBUG`, `CARGO_TERM_VERBOSE`, `TERM`, `COLORTERM`, `NO_COLOR`, every `RUNE_*`
setting except `RUNE_HOME`, and what `passthrough` lists.

## Coming from cross

A `Cross.toml` works unchanged, `[target.<triple>.env] passthrough`/`volumes` and `pre-build`
included. Differences:

- Targets with a hyprcross image use it instead of cross-rs' (set `image` to keep cross-rs').
- Containers mount the project at its host path, not `/project`.
- `pre-build` runs once per warm container, not before every build.
