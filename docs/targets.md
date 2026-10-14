# Supported Target Architectures

Hyprcross supports all primary Rust target architectures across Linux, Windows, macOS, bare-metal, and WebAssembly.

---

## Target Matrix

| Target Triple | Operating System | ABI / Runtime | Direct Mode | hyprlnk Accelerated | Default QEMU Runner |
| :--- | :--- | :--- | :---: | :---: | :--- |
| `aarch64-unknown-linux-gnu` | Linux | glibc | Via sysroot | :white_check_mark: | `qemu-aarch64-static` |
| `aarch64-unknown-linux-musl` | Linux | musl (static) | :white_check_mark: (Native) | :white_check_mark: | `qemu-aarch64-static` |
| `x86_64-unknown-linux-gnu` | Linux | glibc | Host native | :white_check_mark: | Native |
| `x86_64-unknown-linux-musl` | Linux | musl (static) | :white_check_mark: (Native) | :white_check_mark: | Native |
| `armv7-unknown-linux-gnueabihf`| Linux | glibc | Via sysroot | :white_check_mark: | `qemu-arm-static` |
| `armv7-unknown-linux-musleabihf`| Linux | musl (static) | :white_check_mark: (Native) | :white_check_mark: | `qemu-arm-static` |
| `riscv64gc-unknown-linux-gnu` | Linux | glibc | Via sysroot | :white_check_mark: | `qemu-riscv64-static` |
| `riscv64gc-unknown-linux-musl`| Linux | musl (static) | :white_check_mark: (Native) | :white_check_mark: | `qemu-riscv64-static` |
| `powerpc64le-unknown-linux-gnu`| Linux | glibc | Via sysroot | :white_check_mark: | `qemu-ppc64le-static`|
| `s390x-unknown-linux-gnu` | Linux | glibc | Via sysroot | :white_check_mark: | `qemu-s390x-static` |
| `i686-unknown-linux-gnu` | Linux | glibc | Via multiarch | :white_check_mark: | `qemu-i386-static` |
| `i686-unknown-linux-musl` | Linux | musl (static) | :white_check_mark: (Native) | :white_check_mark: | `qemu-i386-static` |
| `x86_64-pc-windows-gnu` | Windows | MinGW-w64 (GNU) | Container / Direct | :white_check_mark: | Wine / Native |
| `i686-pc-windows-gnu` | Windows | MinGW-w64 (GNU) | Container / Direct | :white_check_mark: | Wine / Native |
| `x86_64-pc-windows-msvc` | Windows | MSVC | Direct mode / Custom image | :white_check_mark: | Wine / Native |
| `i686-pc-windows-msvc` | Windows | MSVC | Direct mode / Custom image | :white_check_mark: | Wine / Native |
| `aarch64-pc-windows-msvc` | Windows | MSVC | Direct mode / Custom image | :white_check_mark: | Windows ARM64 |
| `aarch64-apple-darwin` | macOS | Mach-O / dyld | Direct mode / macOS host | :white_check_mark: | Rosetta / Native |
| `x86_64-apple-darwin` | macOS | Mach-O / dyld | Direct mode / macOS host | :white_check_mark: | Native |
| `wasm32-unknown-unknown` | WebAssembly | None | :white_check_mark: (Native) | — | Wasmtime / Node |
| `wasm32-wasip1` / `wasip2` | WebAssembly | WASI | :white_check_mark: (Native) | — | Wasmtime |

---

## musl vs. glibc Targets

### When to Choose musl (`*-unknown-linux-musl`)

For purely static Linux binaries that run anywhere without external runtime dependencies (including scratch containers like `FROM scratch` or Alpine Linux):
- Hyprcross enables **Direct Mode** automatically for musl targets.
- No Docker daemon or container image is required.
- Compiled with `hyprlnk` in milliseconds on your host machine.

### When to Choose glibc (`*-unknown-linux-gnu`)

When your crate depends on dynamically linked system packages, vendor SDKs, or hardware drivers:
- Hyprcross uses the persistent container daemon with official multiarch sysroots.
- Mounts `hyprlnk` inside the container for accelerated foreign linking.
