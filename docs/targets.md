# Targets

`hyprcross targets` prints the supported matrix and filters by optional search words. Every
advertised target is built directly on the host.

| Family | Advertised targets | SDK and execution |
| :--- | :--- | :--- |
| Linux glibc | x86_64, i686, AArch64, ARM, RISC-V, PowerPC, s390x, LoongArch, SPARC64 | glibc SDK; native or QEMU |
| Linux musl | x86_64, AArch64, ARM, RISC-V | musl SDK; native or QEMU |
| Windows | x86_64/i686 GNU and MSVC; AArch64 MSVC and GNU/LLVM | Windows SDK; native Windows or Wine where compatible |
| macOS | x86_64 and AArch64 Darwin | macOS SDK; native macOS execution |
| Android | x86_64, i686, AArch64, ARMv7 | Android NDK SDK; build on the host |
| FreeBSD | x86_64 | FreeBSD SDK; build on the host |
| WASI | wasm32-wasip1 and wasm32-wasip2 | Rust sysroot; Wasmtime |
| Bare metal | thumbv6m, thumbv7m, thumbv7em, thumbv7em-eabihf | Rust sysroot; build only |

The exact 39-target list is the output of `hyprcross targets`. Install SDKs in advance with
`hyprcross sdk install <target>`. Targets that need no external SDK use Rust's installed target
standard library directly.

musl is useful for self-contained Linux programs. glibc builds are dynamically linked and need
a runtime compatible with the SDK used for the build.
