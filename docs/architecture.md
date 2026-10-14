# Hyprcross Architecture

Hyprcross is designed to eliminate the performance bottlenecks that plague conventional containerized cross-compilation tools.

---

## The Cross-Compilation Bottlenecks

Traditional tools like `cross-rs/cross` use a straightforward but slow execution model:

```text
[User Command] → [Start Docker Container] → [Mount Volumes] → [Run Cargo in Container] → [Slow GNU ld/lld Link] → [Tear Down Container]
     0s                   +1.8s                 +0.4s                   +8.2s                 +6.4s                   +0.3s
Total Time: ~17.1 seconds
```

This model suffers from four structural performance flaws:
1. **Container Spin-up Overhead**: Spawning a fresh container for each command incurs a 1.5s to 3s penalty on Linux and up to 5s on macOS/Windows.
2. **Filesystem Virtualization Penalties**: Host-to-container volume bind mounts suffer from virtualization overhead and permission translation.
3. **Linker Stalls**: Linking complex binaries inside the container relies on default GNU `ld` or `lld`, which lack modern parallel symbol hashing and concurrent relocation passes.
4. **Unnecessary Containerization**: Many cross targets (such as `musl`, bare metal, or systems with local target sysroots) do not require container isolation, yet are forced into slow containers.

---

## How Hyprcross Solves This

Hyprcross introduces a multi-tier execution architecture:

```text
                                 ┌───────────────────────────────┐
                                 │      hyprcross CLI / API     │
                                 └───────────────┬───────────────┘
                                                 │
                                 ┌───────────────▼───────────────┐
                                 │     Engine Selection Matrix   │
                                 └───────┬───────────────┬───────┘
                                         │               │
                 Target supports Direct  │               │ Requires Target Sysroot/Env
                                         ▼               ▼
                       ┌───────────────────┐   ┌───────────────────────────┐
                       │   Direct Engine   │   │     Container Engine      │
                       │   (Host Native)   │   │  (Persistent Daemon Pool) │
                       └─────────┬─────────┘   └─────────────┬─────────────┘
                                 │                           │
                                 │  0ms startup delay        │  ~20ms docker/podman exec
                                 │                           │
                                 └───────────────┬───────────┘
                                                 │
                                 ┌───────────────▼───────────────┐
                                 │     hyprlnk Linker Driver     │
                                 │  (ELF / Mach-O / PE/COFF)     │
                                 └───────────────┬───────────────┘
                                                 │
                                 ┌───────────────▼───────────────┐
                                 │    Rune CAS or Cargo Engine   │
                                 └───────────────────────────────┘
```

---

## Tier 1: Direct Host Engine (`engine = "direct"`)

When targeting platforms that don't depend on target-specific dynamic glibc runtime environments (such as `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `wasm32-unknown-unknown`, bare-metal Cortex-M, or when the host has the required target toolchains installed), Hyprcross completely bypasses Docker and Podman:

- **0 ms container startup latency**.
- Direct NVMe host disk I/O.
- Full access to host compiler caches and Rune CAS storage.
- Automated `hyprlnk` injection as the target cross-linker.

---

## Tier 2: Persistent Container Daemon (`daemon = true`)

When container isolation is necessary (for instance, cross-compiling complex C/C++ dependencies like OpenSSL or SQLite that link against a target glibc sysroot), Hyprcross maintains a persistent warm container session in the background:

1. **Lazy Initialization**: The target container is spun up once in detached mode (`sleep infinity`).
2. **Instant Sub-20ms Execution**: Subsequent commands (`build`, `check`, `test`, `clippy`) execute via `docker exec -i` or `podman exec -i`.
3. **Shared Toolchain & Cache Volumes**: The Cargo cache and Rune CAS directories remain hot in memory, eliminating cold dependency downloads.
4. **Idle Teardown**: After 15 minutes of inactivity (configurable via `idle_timeout_mins`), the daemon automatically shuts down to release system memory.

---

## Tier 3: Accelerated Linking via `hyprlnk`

Linking is often the single longest serialized step of a multi-crate build. Hyprcross mounts or injects `hyprlnk` into the build process:
- Linux ELF: Concurrent section copy, single-pass relocation processing, parallel symbol hashing.
- Windows PE/COFF: Concurrent PDB generation alongside PE image writing.
- macOS Mach-O: Parallel atom marking, chained fixups, and pipelined ad-hoc code signing.

---

## Builder Orchestration: Rune & Cargo

Hyprcross automatically probes for [Rune](https://runepm.com):
- **If Rune is detected**: Hyprcross prioritizes `rune` as the build engine. In direct mode, it invokes `rune` on the host. In container mode (both one-off and persistent daemon), Hyprcross mounts the host's `rune` binary to `/usr/local/bin/rune` and bind-mounts `$HOME/.rune` to `/rune` (`RUNE_HOME=/rune`). This brings Rune's content-addressable store (CAS), pipelined `.rmeta` metadata unblocking, and multi-core scheduling into the cross-compilation environment with zero container rebuilds required.
- **If only Cargo is detected**: Hyprcross falls back to standard `cargo`, configuring target flags and runners transparently.
