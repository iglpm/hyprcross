# Performance

What a cross build costs beyond the build itself: starting the right toolchain in the right
place. For a build with nothing to do, that is most of the time.

## Fully cached builds

hyprcross building itself with nothing to compile (`build` right after a `build`), with Rune as
the builder:

| | Median | Fastest |
| :--- | ---: | ---: |
| `rune build --target x86_64-unknown-linux-gnu` (no hyprcross) | 39.1 ms | 33.3 ms |
| `hyprcross build -t x86_64-unknown-linux-gnu` (direct) | 39.7 ms | 35.7 ms |
| `hyprcross build -t aarch64-unknown-linux-gnu` (warm container, agent) | **40.5 ms** | 37.0 ms |
| `podman exec <container> rune build --target aarch64-unknown-linux-gnu` | 149.4 ms | 138.4 ms |
| `podman exec <container> true` | 142.3 ms | 137.1 ms |
| `hyprcross --version` | 0.7 ms | 0.6 ms |

Intel Core i5-10400F (6 cores, 12 threads), CachyOS (Linux 7.2), ext4, rootless Podman 6.1.4,
rustc 1.98.1, release builds of Rune 2027.0.0 and hyprcross, image
`hyprcross/aarch64-unknown-linux-gnu` built from [docker/](../docker); median of 15 runs after
one warm-up.

hyprcross adds about a millisecond on the host and under two in a container. The fourth row is
what a build in a warm container cost before the agent: `podman exec` alone takes 140 ms.

Reproduce with `python3 benches/bench_runner.py` (it writes `benches/benchmark_results.json`).

## What was slow before

hyprcross 0.1 took over a second for a cached build in a container: 1.1 s for
`aarch64-unknown-linux-gnu` before failing to link, and 1.6 s (debug builds) once linking was
fixed but before the first fix below. Three things added up:

| Cause | Cost | Fix |
| :--- | ---: | :--- |
| Rune restored every artifact from its cache on each build, and in a container it copied them: hard links can't cross the container's separate mounts of the project and `~/.rune`. Each copy was hashed again. | ≈ 1.5 s | Rune dates its copies like the cached original and recognises them on the next build (Rune `rune-cas`, `rune-common`). |
| `podman exec` for every build, plus `podman ps` to see whether the container was running | ≈ 180 ms | The agent: a socket connection instead of the engine, and the container's state in a file instead of `ps`. |
| `rustup target list` to check the target's standard library | ≈ 20 ms | Read from the toolchain's directory. |

hyprcross 0.1 also passed its linker through `RUSTFLAGS`, which made Rune treat every crate as
changed whenever builds alternated between `rune` and `hyprcross`, and put `-C
link-arg=__hyprlnk` on link lines, which broke them. Rune now links with its own hyprlnk and
gets no flags from hyprcross.

## Your own setup

- **A debug build of `rune` on PATH** is several times slower than a release build, cached
  builds included (116 ms against 39 ms here). `hyprcross doctor` warns about it.
- **The first build in a container** starts it (about 0.2 to 0.3 s with a local image) and, the
  first time, pulls the image.
- **Docker Desktop** (macOS, Windows) can't reach the agent's socket: builds use `exec` there.
