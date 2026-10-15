# Testing with QEMU User Emulation

Hyprcross provides seamless execution for `hyprcross test` and `hyprcross run` on foreign architectures using QEMU user-mode emulation.

---

## How Cross-Testing Works

When compiling for a foreign target (such as `aarch64-unknown-linux-gnu` from an `x86_64` host), the resulting binary contains machine instructions the host CPU cannot natively execute.

Hyprcross bridges this automatically:

1. **Emulator Discovery**: Hyprcross locates the appropriate static QEMU binary (e.g. `qemu-aarch64-static`, `qemu-arm-static`, `qemu-riscv64-static`).
2. **Dynamic Linker Configuration (`QEMU_LD_PREFIX`)**: Foreign dynamically linked binaries require target glibc shared libraries (`ld-linux-aarch64.so.1`, `libc.so.6`). Hyprcross configures the sysroot library path so the emulator can resolve dynamic symbols without `file not found` errors.
3. **Runner Injection**: Hyprcross sets `CARGO_TARGET_<TRIPLE>_RUNNER` to route test harness execution through QEMU transparently.

In containers none of this is needed on the host: every hyprcross image whose target can run
under emulation has QEMU (or Wine, or Wasmtime) and sets `CARGO_TARGET_<TRIPLE>_RUNNER` to
`hyprcross-runner`, which picks the right one. The sections below are about tests run directly
on the host (`--engine direct`, or targets whose C cross compiler is installed).

On the host, QEMU's `-L` sysroot is the configured `sysroot`, else where distributions install
the cross toolchain's C library: `/usr/<prefix>/libc` (Arch) or `/usr/<prefix>` (Debian, Fedora),
with `<prefix>` like `aarch64-linux-gnu`.

---

## Running Unit and Integration Tests

```sh
# Run all tests under ARM64 emulation
hyprcross test --target aarch64-unknown-linux-gnu

# Run a specific test with backtrace enabled
RUST_BACKTRACE=1 hyprcross test --target aarch64-unknown-linux-gnu -- test_name
```

---

## Kernel `binfmt_misc` Acceleration

On Linux hosts, you can enable kernel-level transparent emulation via `binfmt_misc`:

```sh
# Debian / Ubuntu
sudo apt-get install -y qemu-user-static binfmt-support

# Arch Linux
sudo pacman -S qemu-user-static qemu-user-static-binfmt

# Verify status
hyprcross doctor
```

When `binfmt_misc` is active, the Linux kernel detects foreign ELF headers and invokes QEMU automatically, eliminating user-space wrapper indirection.

---

## Running Executables Directly

To compile and immediately execute a binary target:

```sh
hyprcross run --target aarch64-unknown-linux-gnu -- --flag argument
```
