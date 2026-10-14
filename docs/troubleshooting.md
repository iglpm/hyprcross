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

# Or use rootless Podman (no daemon needed)
sudo apt-get install -y podman
hyprcross config set build.container_engine podman
```

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

If a container enters an inconsistent state:

```sh
# Stop and prune all hyprcross daemon containers
hyprcross daemon stop --all
hyprcross daemon prune
```

---

## 6. Container Image 403 Forbidden or Not Found on Windows MSVC (`*-pc-windows-msvc`)

### Symptom
Running `hyprcross daemon start --target i686-pc-windows-msvc` or building for Windows MSVC in a container fails with:
```text
Error: unable to copy from source docker://ghcr.io/cross-rs/i686-pc-windows-msvc:edge
fetching manifest edge in ghcr.io/cross-rs/i686-pc-windows-msvc: Requesting bearer token: received unexpected HTTP status: 403 Forbidden
```

### Cause
Microsoft MSVC SDK headers and runtime DLLs cannot be legally distributed in public open-source container images. Therefore, `ghcr.io/cross-rs` does not provide pre-built container images for `*-pc-windows-msvc` or `*-apple-darwin`.

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

