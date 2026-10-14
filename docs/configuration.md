# Configuration Reference

Hyprcross supports extensive configuration via `Hyprcross.toml`, existing `Cross.toml` files, and `Cargo.toml` metadata tables.

---

## Configuration File Resolution

Hyprcross checks for configuration in the following order of priority:

1. `Hyprcross.toml` in the current project or workspace directory.
2. `Cross.toml` (100% backward compatibility with `cross-rs/cross`).
3. `Cargo.toml` (`[package.metadata.hyprcross]` or `[package.metadata.cross]`).
4. Global configuration at `~/.config/hyprcross/config.toml`.

---

## Global Build Settings (`[build]`)

```toml
[build]
# Execution engine mode:
# - "auto" (default): Runs direct on host if target is musl/native, otherwise uses container
# - "direct": Forces host execution (0ms container latency)
# - "container": Forces containerized execution
engine = "auto"

# Linker acceleration:
# - "hyprlnk" (default): Uses hyprlnk for 2x–4x faster linking
# - "default": Uses target's standard linker (lld or GNU ld)
linker = "hyprlnk"

# Build orchestrator:
# - "auto" (default): Uses Rune if installed on host, otherwise Cargo
# - "rune": Forces Rune build engine
# - "cargo": Forces Cargo
builder = "auto"

# Persistent warm container daemon:
# When true, maintains background warm container for sub-20ms invocations
daemon = true

# Container daemon idle timeout in minutes before auto-stopping (default: 15)
idle_timeout_mins = 15

# Container engine runtime:
# "auto", "docker", "podman", "finch", or "nerdctl"
container_engine = "auto"

# Default target triple when --target is omitted
default_target = "aarch64-unknown-linux-gnu"
```

---

## Target-Specific Settings (`[target.<triple>]`)

You can customize individual target architectures:

```toml
[target.aarch64-unknown-linux-gnu]
# Custom container image
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:edge"

# Path to custom Dockerfile / Containerfile
# dockerfile = "docker/Dockerfile.aarch64"

# Custom sysroot path for direct mode or QEMU library path
sysroot = "/usr/aarch64-linux-gnu"

# Target runner executable for `hyprcross test` and `hyprcross run`
runner = "qemu-aarch64-static"

# Injected environment variables
[target.aarch64-unknown-linux-gnu.env]
OPENSSL_STATIC = "1"
PKG_CONFIG_ALLOW_CROSS = "1"

# Additional volume mounts (format: host:container[:opts])
volumes = [
    "/opt/custom-sdk:/opt/custom-sdk:ro",
]

# Commands to run inside container before the build
pre_build = [
    "apt-get update && apt-get install -y libssl-dev",
]
```

---

## Environment Variables

Hyprcross respects the following environment variables:

| Variable | Description | Default |
| :--- | :--- | :--- |
| `HYPRCROSS_TARGET` | Default target triple | None |
| `HYPRCROSS_ENGINE` | Engine override (`auto`, `direct`, `container`) | `auto` |
| `HYPRCROSS_LINKER` | Linker override (`hyprlnk`, `default`) | `hyprlnk` |
| `HYPRCROSS_BUILDER` | Builder override (`auto`, `rune`, `cargo`) | `auto` |
| `HYPRCROSS_HYPRLNK` | When set, executable acts directly as the linker | Unset |
| `RUST_BACKTRACE` | Forwarded directly into build and test processes | Host value |

---

## Migration from `Cross.toml`

Existing `Cross.toml` files work out of the box with zero modifications:

```toml
# Existing Cross.toml
[target.aarch64-unknown-linux-gnu]
image = "my-custom-image:latest"
volumes = ["/tmp:/tmp"]

[target.aarch64-unknown-linux-gnu.env]
passthrough = ["MY_API_KEY"]
```

Hyprcross reads these settings and automatically applies `hyprlnk` linking acceleration and persistent daemon caching.
