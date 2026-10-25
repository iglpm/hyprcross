# Architecture

hyprcross runs Igel or Cargo directly on the host. It installs a small target SDK when a target
needs system headers or libraries, configures the linker and sysroot, and then invokes the
builder with `--target`.

```text
hyprcross CLI
  ├─ target and SDK selection
  ├─ environment and linker configuration
  ├─ igl or cargo on the host
  └─ QEMU, Wine or Wasmtime for test/run
```

SDKs live under the platform data directory, normally
`~/.local/share/hyprcross/sdks/<target>`. `HYPRCROSS_SDK_DIR` changes the SDK store;
`HYPRCROSS_SDK_PATH` or `HYPRCROSS_SDK_PATH_<TARGET>` selects an existing SDK.

Linux targets use an SDK sysroot and `ld.lld` where supported. PowerPC64 big-endian and
SPARC64 use the corresponding GNU linker because LLVM lld does not implement those ELF
emulations. macOS uses SDK stubs, Windows uses MSVC, GNU, or LLVM-compatible SDK content, and
WASI and bare-metal targets use Rust's bundled linker.

For `test`, `run`, and `bench`, hyprcross injects a runner when the output cannot execute on the
host: QEMU user mode for Linux, Wine for Windows GNU, and Wasmtime for WASI. A missing runner
does not prevent `build` or `check`.
