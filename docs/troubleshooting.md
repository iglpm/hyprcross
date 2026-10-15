# Troubleshooting Guide

Solutions and diagnostic steps for common cross-compilation hurdles.

---

## 1. Running Diagnostics

Whenever unexpected build errors occur, the first step is running `hyprcross doctor`:

```sh
hyprcross doctor
```

This verifies that:
- Host toolchains (`rustc`, `cargo`, `rune`) are functioning.
- `hyprlnk` is available and compatible with your target.
- Docker or Podman daemon is responding without permission errors.
- QEMU static binaries are in your `$PATH`.

---

## 2. Docker Permission Denied (`/var/run/docker.sock`)

### Symptom
```text
permission denied while trying to connect to the Docker daemon socket
```

### Solution
Add your current user to the `docker` group, or use Podman:

```sh
# Add user to docker group (requires logout/login)
sudo usermod -aG docker $USER

# Or use rootless Podman (no daemon needed); hyprcross prefers it when both are installed
sudo apt-get install -y podman
```

To pick one explicitly: `container_engine = "podman"` under `[build]` in `Hyprcross.toml`, or
`HYPRCROSS_CONTAINER_ENGINE=podman`.

---

## 3. Dynamic Linker Not Found (`/lib/ld-linux-*.so.1`) During `test`

### Symptom
Running `hyprcross test` fails with:
```text
/lib/ld-linux-aarch64.so.1: No such file or directory
```

### Solution
The foreign test binary is dynamically linked against glibc, but QEMU cannot find the target dynamic linker.

1. **Option A: Compile for musl (Static Linking)**:
   ```sh
   hyprcross test --target aarch64-unknown-linux-musl
   ```
   Musl binaries are fully static and run under QEMU without external library paths.

2. **Option B: Configure Target Sysroot**:
   Install the target cross-toolchain libc and set `sysroot` in `Hyprcross.toml`:
   ```toml
   [target.aarch64-unknown-linux-gnu]
   sysroot = "/usr/aarch64-linux-gnu"
   ```

---

## 4. Missing C Library Headers in Cross Build (e.g. `openssl-sys`)

### Symptom
A C/C++ dependency fails to compile inside the container with:
```text
Could not find directory of OpenSSL installation
```

### Solution
Add `pre_build` package installations or static vendoring in `Hyprcross.toml`:

```toml
[target.aarch64-unknown-linux-gnu]
pre_build = [
    "apt-get update && apt-get install -y libssl-dev:arm64",
]

[target.aarch64-unknown-linux-gnu.env]
OPENSSL_STATIC = "1"
PKG_CONFIG_ALLOW_CROSS = "1"
```

Or enable the `vendored` feature on the crate in your `Cargo.toml`:
```toml
openssl = { version = "0.10", features = ["vendored"] }
```

---

## 5. Cleaning Stale Daemon Containers

hyprcross replaces a warm container by itself when what it was made from changes (image,
mounts, a rebuilt `rune` or `hyprcross`, `pre_build`). If one still misbehaves:

```sh
hyprcross daemon status          # what runs, for which project
hyprcross daemon stop -t <triple> # this project's container for a target
hyprcross daemon stop --all      # every hyprcross container
hyprcross daemon prune           # the stopped ones
```

Containers from hyprcross 0.1 (named `hyprcross-<project>-<target>-<builder>`, without a
hash) are no longer used; `hyprcross daemon stop --all` removes them.

## 5a. Builds in a Container Are Slow Even With Nothing to Do

- Run `hyprcross doctor`: a debug build of `rune` on PATH makes every build several times
  slower.
- With `-v`, hyprcross prints what it runs. A warm build shows one `agent:` line and no
  `podman`/`docker` commands; if it shows `exec`, the agent couldn't start in the image (the
  warning when the container was created says why), or the platform can't reach it.

## 5b. `hyprcross build ... --engine direct` Passes `--engine` to the Builder

It doesn't any more: hyprcross' own options (`-t`, `--engine`, `--builder`, `--linker`,
`--daemon`, `--no-daemon`, `-q`, `-v`, `--color`) are picked out wherever they are, before `--`.
Everything after `--` goes to the builder as is.

---

## 6. Container Image 403 Forbidden or Not Found on Windows MSVC (`*-pc-windows-msvc`)

### Symptom
Running `hyprcross daemon start --target i686-pc-windows-msvc` or building for Windows MSVC in a container fails with:
```text
Error: unable to copy from source docker://ghcr.io/cross-rs/i686-pc-windows-msvc:edge
fetching manifest edge in ghcr.io/cross-rs/i686-pc-windows-msvc: Requesting bearer token: received unexpected HTTP status: 403 Forbidden
```

### Cause
Microsoft MSVC SDK headers and runtime DLLs cannot be legally distributed in public open-source container images. Therefore neither hyprcross nor cross-rs provides images for `*-pc-windows-msvc` or `*-apple-darwin`; hyprcross says so before trying to pull one.

### Solutions

1. **Option A: Use the Windows GNU ABI (Recommended for Containers)**:
   The MinGW-w64 GNU toolchain is open source and pre-built in public registries:
   ```sh
   hyprcross build --target x86_64-pc-windows-gnu
   # or for 32-bit:
   hyprcross build --target i686-pc-windows-gnu
   ```

2. **Option B: Use Direct Engine with Host MSVC Tools (`xwin`)**:
   Run directly on your host without containers using an MSVC sysroot or `xwin`:
   ```sh
   hyprcross build --target x86_64-pc-windows-msvc --engine direct
   ```

3. **Option C: Provide a Custom Container Image**:
   If your organization has built an internal container image containing the licensed MSVC SDK:
   ```toml
   # Hyprcross.toml
   [target.i686-pc-windows-msvc]
   image = "your-private-registry.com/msvc-builder:latest"
   ```

