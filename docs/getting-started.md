# Getting started

Install the standalone CLI and Cargo subcommand:

```sh
cargo install --locked hyprcross cargo-hyprcross
hyprcross doctor
hyprcross targets
```

Build for a target. The first command downloads its SDK when needed; later projects reuse it.

```sh
hyprcross build -t aarch64-unknown-linux-gnu
hyprcross test -t aarch64-unknown-linux-gnu
```

Tests for foreign Linux architectures use QEMU when installed. Windows GNU tests use Wine and
WASI tests use Wasmtime. You can still build when a compatible runner is unavailable.

Use Cargo explicitly with `--builder cargo`, or Igel with `--builder igl`. Select the target's
normal linker with `--linker default`; otherwise hyprcross uses hyprlnk where supported.

SDK commands make downloads explicit:

```sh
hyprcross sdk list
hyprcross sdk install aarch64-unknown-linux-gnu
hyprcross sdk path aarch64-unknown-linux-gnu
hyprcross sdk remove aarch64-unknown-linux-gnu
```
