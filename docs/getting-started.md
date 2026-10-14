# Getting Started with Hyprcross

Hyprcross is designed as a drop-in, faster replacement for `cross`. You can invoke it as a standalone binary (`hyprcross`) or as a Cargo subcommand (`cargo hyprcross`).

---

## 1. Installation

### From Source (Release Binary)

```sh
# Clone and build
git clone https://github.com/runepm/hyprcross.git
cd hyprcross
cargo build --release

# Place into your PATH
sudo cp target/release/hyprcross /usr/local/bin/
sudo ln -sf /usr/local/bin/hyprcross /usr/local/bin/cargo-hyprcross
```

### Pre-built Binaries

Pre-compiled release packages with SHA-256 checksums are available on our [GitHub Releases](https://github.com/runepm/hyprcross/releases) page for:
- Linux (x86_64, aarch64, musl)
- macOS (Apple Silicon arm64, Intel x86_64)
- Windows (x86_64 MSVC)

---

## 2. Verifying Your Environment

Run `hyprcross doctor` to inspect installed container runtimes, QEMU emulators, kernel `binfmt_misc` support, and Rust toolchains:

```sh
hyprcross doctor
```

Sample output:
```text
hyprcross doctor: cross-compilation health check

  ▸ Host triple: x86_64-unknown-linux-gnu
  ▸ Rust compiler: rustc 1.98.1 (48a229cea 2026-09-01)
  ▸ Rune build engine: rune 2027.0.0 (CAS acceleration available)
  ▸ Cargo package manager: cargo 1.98.1
  ▸ hyprlnk linker: available (embedded v2027.0.0, ELF/COFF/Mach-O accelerated)

Container Engines:
  ✓ docker at /usr/bin/docker (daemon responsive)
  ✓ podman at /usr/bin/podman (daemon responsive)

QEMU User Emulation:
  ✓ QEMU for aarch64: /usr/bin/qemu-aarch64-static
  ✓ QEMU for arm: /usr/bin/qemu-arm-static
  ✓ QEMU for riscv64: /usr/bin/qemu-riscv64-static
  ▸ Linux kernel binfmt_misc: enabled (transparent binary emulation ready)

Doctor summary: System is ready for hyprcross builds.
```

---

## 3. Your First Cross-Compilation

Inside any Rust or Rune project, simply specify your target architecture:

```sh
# Cross-compile for Linux ARM64
hyprcross build --target aarch64-unknown-linux-gnu

# Or via cargo plugin syntax
cargo hyprcross build --target aarch64-unknown-linux-gnu
```

### Type-Checking Without Codegen

Hyprcross supports `check` for lightning-fast feedback:

```sh
hyprcross check --target aarch64-unknown-linux-gnu
```

---

## 4. Running Cross-Compiled Tests

To compile and run your test suite under target architecture emulation:

```sh
hyprcross test --target aarch64-unknown-linux-gnu
```

Hyprcross resolves the target QEMU emulator (`qemu-aarch64-static`) and configures `QEMU_LD_PREFIX` so your test binary executes seamlessly.

---

## 5. Next Steps

- Learn how [Architecture](architecture.md) delivers zero-delay container execution.
- See how [hyprlnk Integration](hyprlnk-integration.md) accelerates linking by 2x–4x.
- Customize your build settings in [Configuration](configuration.md).
