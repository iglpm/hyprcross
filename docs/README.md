# hyprcross documentation

| Guide | |
| :--- | :--- |
| [Getting started](getting-started.md) | Installation and the first cross build |
| [Architecture](architecture.md) | Direct builds, warm containers and the agent |
| [Configuration](configuration.md) | `Hyprcross.toml`, `Cross.toml` and environment variables |
| [Targets](targets.md) | What builds where, and what runs the tests |
| [Images](../docker/README.md) | The per-target images and how they are maintained |
| [hyprlnk](hyprlnk-integration.md) | How builds are linked |
| [Testing with QEMU](qemu-testing.md) | Running tests of other architectures |
| [Performance](performance.md) | Measurements and method |
| [Troubleshooting](troubleshooting.md) | Common problems and their fixes |

In short: hyprcross builds a target on the host when the host can (its own target, targets that
need no C toolchain, targets whose C cross compiler is installed), and otherwise in that
target's image, in a container that stays warm between builds and takes commands from hyprcross
over a Unix socket. Rune or Cargo does the building; hyprlnk links.
