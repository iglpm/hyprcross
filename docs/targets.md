# Targets

Any target rustc knows can be built: hyprcross runs the builder with `--target`. What differs is
where the C toolchain comes from and what runs the tests. `hyprcross targets` lists the targets
and how each builds on your machine. You can filter targets by query:

```console
$ hyprcross targets "windows arm"
Targets · 2 available matching `windows arm`
    aarch64-pc-windows-msvc     direct     MSVC · hyprlnk
    aarch64-pc-windows-gnullvm  container  

$ hyprcross targets macos
Targets · 2 available matching `macos`
  ✓ x86_64-apple-darwin   direct     macOS SDK stubs · hyprlnk
  ✓ aarch64-apple-darwin  direct     macOS SDK stubs · hyprlnk
```

## Native direct builds with SDKs (Default, Zero Docker)

hyprcross compiles on the host and links with **hyprlnk** against minimal SDK sysroots:

| Targets | How | Tests run under |
| :--- | :--- | :--- |
| Linux glibc (`*-unknown-linux-gnu`) | Host compiler + target glibc SDK sysroot + hyprlnk | QEMU (or host natively) |
| Linux musl (`*-unknown-linux-musl`) | Host compiler + musl sysroot + hyprlnk | QEMU (or host natively) |
| macOS (`*-apple-darwin`) | Host compiler + MacOSX SDK + hyprlnk Mach-O | macOS host |
| Windows MSVC (`*-pc-windows-msvc`) | Host compiler + MSVC/Windows SDK + hyprlnk COFF | Wine (or Windows natively) |
| Windows MinGW (`*-pc-windows-gnu`) | Host compiler + MinGW SDK + hyprlnk COFF | Wine (or Windows natively) |
| WASI (`wasm32-wasip1`, `wasm32-wasip2`) | Directly: rust-lld links them | Wasmtime |
| Bare-metal Arm (`thumbv*`) | Directly: rust-lld links them | build only |

SDKs are downloaded automatically on demand to `~/.local/share/hyprcross/sdks/` or can be pre-fetched via `hyprcross sdk install <target>`.

## Optional container images

If `--engine container` is specified or custom Dockerfiles are configured in `Hyprcross.toml`, hyprcross uses the prebuilt images at `ghcr.io/runepm/hyprcross/<target>`. See [docker/README.md](../docker/README.md#targets).


## glibc or musl

- **musl** (`*-linux-musl`): static binaries that run on any Linux of the architecture, from
  `FROM scratch` containers to old distributions.
- **glibc** (`*-linux-gnu`): dynamically linked against glibc 2.39 when built in the images, so
  they need a system with glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 and later). For
  older systems, build for musl, or build in an image of your own with an older glibc (`image`
  in `Hyprcross.toml`).
