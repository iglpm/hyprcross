# Troubleshooting

Start with `hyprcross doctor` and repeat the failing command with `-v`.

## Missing Rust target

Install it with `rustup target add <triple>`. hyprcross normally offers to do this when the
target standard library is absent.

## Missing or invalid SDK

```sh
hyprcross sdk remove <triple>
hyprcross sdk install <triple>
hyprcross sdk path <triple>
```

Use `HYPRCROSS_SDK_PATH` for a local sysroot or
`HYPRCROSS_SDK_PATH_<TARGET>` for a per-target override.

## C or C++ dependency cannot find headers

Confirm that the advertised SDK is installed and that the crate honors the standard `CC`,
`CXX`, `AR`, and target-specific Cargo environment variables. Run with `-v` to inspect the
compiler and sysroot. An SDK produced from symlinked distribution headers must include the
symlink targets inside the archive.

## Cross-linked program does not run

Install QEMU user mode for foreign Linux architectures, Wine for compatible Windows GNU
binaries, or Wasmtime for WASI. Building does not require a runner. For Linux, reinstall the
SDK if its dynamic loader is absent.

## Linker error for PowerPC64 big-endian or SPARC64

LLVM lld does not provide these ELF emulations. Install GNU binutils for the target so
`powerpc64-linux-gnu-ld` or `sparc64-linux-gnu-ld` is available on `PATH`.

## macOS and Windows SDKs

On the native platform, point hyprcross at a complete installed SDK when a crate needs headers
or libraries beyond the downloadable stubs. Use `HYPRCROSS_MACOS_SDK_PATH` on macOS and the
general SDK path overrides for Windows.
