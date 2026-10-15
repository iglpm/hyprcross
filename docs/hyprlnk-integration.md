# hyprlnk

[hyprlnk](https://github.com/runepm/hyprlnk) is the linker built into Rune and hyprcross. It
links Linux ELF (x86-64, x86, AArch64, ARM from ARMv5TE, RISC-V 64, POWER little-endian, s390x
and LoongArch64; glibc and musl), Windows PE/COFF (x64, ARM64, x86, with PDBs) and macOS Mach-O
(arm64, arm64e, x86_64, with code signatures). Other targets keep their own linker.

## How fast

Linking Rune's own debug build on Linux x86-64 (Intel i5-10400F, outputs on tmpfs, median of 11):

| Linker | Time |
| :--- | ---: |
| hyprlnk | 258 ms |
| wild | 255 ms |
| mold | 427 ms |
| lld | 478 ms |

hyprlnk links in parallel, maps its inputs instead of reading them, and doesn't free memory
before exiting.

## How hyprcross uses it

### With Rune

Rune links with its built-in hyprlnk by itself (`build.linker = "hyprlnk"`, its default).
hyprcross adds nothing to the build, so Rune's cache is the same whether a build runs through
hyprcross or not. `--linker default` passes `--config build.linker="default"` to Rune.

On Linux hyprlnk links through a C compiler, which knows where the target's C runtime and
system libraries are. Rune takes that compiler from the target's configured linker:
`CARGO_TARGET_<TRIPLE>_LINKER`, which every image sets (`aarch64-linux-gnu-gcc`,
`aarch64-linux-musl-gcc`, …) and which hyprcross sets in direct mode when it finds the target's
C cross compiler.

### With Cargo

hyprcross is the linker. Cargo gets `CARGO_TARGET_<TRIPLE>_LINKER` pointing at hyprcross under
the name `hyprlnk`, and `HYPRLNK_CC` names the C compiler to link through:

- on the host, `~/.cache/hyprcross/bin/hyprlnk`, a link to the hyprcross executable (its path
  stays the same when hyprcross is updated, so Cargo doesn't rebuild);
- in containers, `/opt/hyprcross/bin/hyprlnk`, hyprcross mounted under that name.

Run as `hyprlnk`, as `ld` by the C compiler hyprlnk drives, with `HYPRCROSS_HYPRLNK` set, or as
`hyprcross __hyprlnk <args>`, the executable links instead of starting the CLI. A standalone
`hyprlnk` on PATH is used instead when there is one.

Apple targets get `-C linker-flavor=ld64.lld` (through `CARGO_TARGET_<TRIPLE>_RUSTFLAGS`, or
appended to `RUSTFLAGS` when that is set), so rustc passes ld64's command line.
