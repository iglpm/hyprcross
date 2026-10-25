# hyprcross documentation

| Guide | |
| :--- | :--- |
| [Getting started](getting-started.md) | Installation and the first cross build |
| [Architecture](architecture.md) | Direct SDK builds and runners |
| [Configuration](configuration.md) | `Hyprcross.toml`, `Cross.toml` and environment variables |
| [Targets](targets.md) | What builds where, and what runs the tests |
| [hyprlnk](hyprlnk-integration.md) | How builds are linked |
| [Testing with QEMU](qemu-testing.md) | Running tests of other architectures |
| [Performance](performance.md) | Measurements and method |
| [Troubleshooting](troubleshooting.md) | Common problems and their fixes |

In short: hyprcross builds a target on the host when the host can (its own target, targets that
need no C toolchain, targets whose C cross compiler is installed), and otherwise in that
target's SDK, directly on the host, with QEMU, Wine, or Wasmtime when execution needs a runner
over a Unix socket. Igel or Cargo does the building; hyprlnk links.
