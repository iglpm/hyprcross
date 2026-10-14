# hyprlnk Integration

[hyprlnk](https://github.com/runepm/hyprlnk) is the high-performance linker built for Rune and modern systems programming. Hyprcross integrates `hyprlnk` directly to eliminate the linking bottleneck in cross-compilation.

---

## Why Linking Is a Bottleneck in Cross Builds

In a standard cross-compilation workflow, compilation units (`.o` and `.rlib` files) are emitted relatively quickly. However, linking all objects into the final executable or dynamic library must process hundreds of megabytes of inputs sequentially.

Inside containerized cross environments:
- Standard GNU `ld` is single-threaded and lacks zero-copy memory mapping.
- LLVM's `lld` or `rust-lld`, while faster than GNU `ld`, still incurs heavy memory deallocation teardown overhead and sequential relocation passes on foreign target architectures.
- On large Rust projects with debug symbols, linking alone often accounts for **40% to 60%** of total clean and incremental build times.

---

## How hyprlnk Accelerates Cross Builds

`hyprlnk` is built with a fundamentally different internal architecture:

1. **Zero-Copy Memory-Mapped I/O**:
   All input object files, static archives, and text stubs are memory-mapped in-place. Symbol tables are hashed in parallel without copying string buffers into temporary heaps.

2. **Parallel Relocation Scanning**:
   Relocations are scanned in concurrent waves across available CPU cores. Symbols, PLT stubs, and GOT entries are resolved in a lock-free work-stealing phase.

3. **Instant Process Teardown**:
   Traditional linkers spend hundreds of milliseconds freeing individual nodes, strings, and hash table buckets. `hyprlnk` skips manual memory deallocation at process completion, letting the operating system kernel reclaim the process address space instantly.

4. **Multi-Format Target Coverage**:
   - **Linux (ELF)**: x86-64, AArch64, ARMv7, ARMv5TE+, 64-bit RISC-V, POWER (ELFv2), s390x, and LoongArch64 (glibc and musl).
   - **Windows (PE/COFF)**: x64, ARM64, and x86 with full PDB debug generation and Natvis visualizers.
   - **macOS (Mach-O)**: arm64, arm64e, and x86_64 with chained fixups and hardware SHA ad-hoc signing.

---

## Benchmark: Linking Under Cross Compilation

Linking a representative 250-package Rust workspace on Linux x86_64 targeting `aarch64-unknown-linux-gnu` (dev build, 420 MB input data, 85 MB binary):

| Linker | Linking Time | Relative Performance |
| :--- | :---: | :---: |
| **hyprlnk (via Hyprcross)** | **310 ms** | **Baseline (Fastest)** |
| `mold` 3.0 | 480 ms | 1.5× slower |
| `lld` 23 | 820 ms | 2.6× slower |
| GNU `ld` (cross-gcc) | 2,450 ms | 7.9× slower |

---

## How Hyprcross Injects hyprlnk

### Direct Host Mode

Hyprcross passes rustflags configuring itself as the linker:

```sh
-C linker=/path/to/hyprcross -C link-arg=__hyprlnk
```

When rustc or gcc invokes the linker, `hyprcross` dispatches directly to `hyprlnk::main_with_args`.

### Containerized Mode

Hyprcross bind-mounts the `hyprlnk` binary into the container at `/usr/local/bin/hyprlnk:ro` and exports:

```sh
CARGO_TARGET_<TRIPLE>_LINKER=/usr/local/bin/hyprlnk
```

The container's build process automatically picks up `hyprlnk`, giving containerized builds host-native linking speeds.
