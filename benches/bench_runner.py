#!/usr/bin/env python3
"""The measurements in docs/performance.md: fully cached builds of hyprcross itself, on the host
and in a warm container, next to the builder alone and to `<engine> exec`.

    python3 benches/bench_runner.py [--runs 15] [--target aarch64-unknown-linux-gnu]

Needs `hyprcross` and `rune` on PATH (release builds, for numbers worth comparing) and a
container engine. The first build pulls the target's image and starts its container; only the
builds after it are measured. Results go to benches/benchmark_results.json.
"""

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def version(cmd):
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout
        return out.strip().splitlines()[0]
    except (OSError, subprocess.CalledProcessError, IndexError):
        return None


def cpu_model():
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or None


def measure(argv, runs):
    """Median and fastest wall time of `argv` in milliseconds, after one warm-up run."""
    subprocess.run(argv, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    times = []
    for _ in range(runs):
        start = time.perf_counter()
        done = subprocess.run(argv, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        times.append((time.perf_counter() - start) * 1000)
        if done.returncode != 0:
            sys.exit(f"failed: {' '.join(argv)}\n{done.stderr.decode(errors='replace')}")
    return {"median_ms": round(statistics.median(times), 1), "min_ms": round(min(times), 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--runs", type=int, default=15)
    parser.add_argument("--target", default="aarch64-unknown-linux-gnu")
    parser.add_argument("--host", default="x86_64-unknown-linux-gnu")
    args = parser.parse_args()

    hyprcross = os.environ.get("HYPRCROSS") or shutil.which("hyprcross")
    rune = shutil.which("rune")
    engine = shutil.which("podman") or shutil.which("docker")
    if not (hyprcross and rune and engine):
        sys.exit("needs hyprcross, rune and podman or docker on PATH")

    print(f"warming up: building for {args.host} and {args.target} once …", file=sys.stderr)
    for target in (args.host, args.target):
        subprocess.run([hyprcross, "build", "-q", "-t", target], cwd=ROOT, check=True)

    names = subprocess.run(
        [engine, "ps", "--filter", f"label=dev.hyprcross.target={args.target}", "--format", "{{.Names}}"],
        capture_output=True, text=True, check=True,
    ).stdout.split()
    container = next((n for n in names if "-rune-" in n), None)
    if not container:
        sys.exit(f"no warm rune container for {args.target}")

    cases = [
        ("rune build, host target (no hyprcross)", [rune, "build", "--target", args.host]),
        ("hyprcross build, host target (direct)", [hyprcross, "build", "-t", args.host]),
        (f"hyprcross build, {args.target} (warm container, agent)", [hyprcross, "build", "-t", args.target]),
        (f"{Path(engine).name} exec + rune build (the exec path)",
         [engine, "exec", "-w", str(ROOT), container, "rune", "build", "--target", args.target]),
        (f"{Path(engine).name} exec true (exec floor)", [engine, "exec", container, "true"]),
        ("hyprcross --version", [hyprcross, "--version"]),
    ]
    results = []
    for name, argv in cases:
        r = measure(argv, args.runs)
        print(f"{name:<58} median {r['median_ms']:7.1f} ms   fastest {r['min_ms']:7.1f} ms")
        results.append({"case": name, **r})

    report = {
        "runs": args.runs,
        "target": args.target,
        "environment": {
            "cpu": cpu_model(),
            "kernel": platform.release(),
            "engine": version([engine, "--version"]),
            "rustc": version(["rustc", "--version"]),
            "rune": version([rune, "--version"]),
            "hyprcross": version([hyprcross, "--version"]),
        },
        "results": results,
    }
    out = ROOT / "benches" / "benchmark_results.json"
    out.write_text(json.dumps(report, indent=2) + "\n")
    print(f"written to {out.relative_to(ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
