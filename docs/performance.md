# Performance

hyprcross adds SDK and runner configuration around the selected builder. A warm build with no
work left is therefore a useful measurement of its fixed overhead.

## Fully cached builds

The workload is the hyprcross workspace, using release builds of hyprcross and Cargo. Each row
has one checked warm-up followed by 15 checked samples, run sequentially and offline.

| Command | Median | Fastest |
| :--- | ---: | ---: |
| `cargo build --target x86_64-unknown-linux-gnu` | 60.469 ms | 60.000 ms |
| `hyprcross build --builder cargo -t x86_64-unknown-linux-gnu` | 61.664 ms | 60.408 ms |
| `hyprcross build --builder cargo -t aarch64-unknown-linux-gnu` | 62.088 ms | 61.245 ms |
| `hyprcross --version` | 0.765 ms | 0.712 ms |

Environment: Intel Core i5-10400F, Linux, rustc 1.98.1. The AArch64 case uses the installed
direct SDK. The measured fixed overhead is about 1.2 ms for the host target and 1.6 ms for the
cross target. These numbers describe a warm no-op build; compile and link time dominates real
builds.

Reproduce the measurement after release builds with:

```sh
python3 benches/bench_runner.py \
  --hyprcross target/release/hyprcross \
  --igl ../target/release/igl --runs 15
```

Pass `--builder igl` to measure Igel when the workspace lockfile can be resolved by Igel. Raw
samples and version details are in [`benches/benchmark_results.json`](../benches/benchmark_results.json).
