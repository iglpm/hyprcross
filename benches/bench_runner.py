#!/usr/bin/env python3
"""
hyprcross Reproducible Benchmark Runner
Benchmarks real cross-compilation latency, linking throughput, and QEMU emulation overhead.
"""

import os
import sys
import time
import shutil
import tempfile
import subprocess
import json
from pathlib import Path

TARGET = "aarch64-unknown-linux-gnu"
WARMUP_RUNS = 1
BENCH_RUNS = 3

def run_cmd(cmd, cwd=None):
    t0 = time.perf_counter()
    res = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    t1 = time.perf_counter()
    if res.returncode != 0:
        print(f"FAILED: {' '.join(cmd)}\nSTDERR:\n{res.stderr}", file=sys.stderr)
        sys.exit(1)
    return (t1 - t0) * 1000.0  # ms

def bench(name, fn, runs=BENCH_RUNS, warmup=WARMUP_RUNS):
    for _ in range(warmup):
        fn()
    durations = []
    for _ in range(runs):
        durations.append(fn())
    avg_ms = sum(durations) / len(durations)
    min_ms = min(durations)
    print(f"  {name:<45} : avg = {avg_ms:6.1f} ms | min = {min_ms:6.1f} ms")
    return {"avg_ms": round(avg_ms, 1), "min_ms": round(min_ms, 1), "samples": [round(d, 1) for d in durations]}

def setup_test_project(dest_dir):
    cargo_toml = dest_dir / "Cargo.toml"
    cargo_toml.write_text("""[package]
name = "bench_crate"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
""")
    src_dir = dest_dir / "src"
    src_dir.mkdir(parents=True, exist_ok=True)
    main_rs = src_dir / "main.rs"
    main_rs.write_text("""use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Message {
    id: u64,
    content: String,
}

fn main() {
    let msg = Message { id: 42, content: "hyprcross benchmark payload".to_string() };
    let json = serde_json::to_string(&msg).unwrap();
    println!("Serialized: {}", json);
}

#[test]
fn test_serde_roundtrip() {
    let msg = Message { id: 100, content: "test".to_string() };
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
}
""")

def main():
    print("=" * 70)
    print(" hyprcross Cross-Compilation Benchmark Suite")
    print(" Target: " + TARGET)
    print("=" * 70)

    # Detect specs
    specs = {
        "cpu": subprocess.check_output("LC_ALL=C lscpu | grep 'Model name' | head -1", shell=True).decode().split(":")[1].strip(),
        "cores": subprocess.check_output("nproc", shell=True).decode().strip(),
        "os": subprocess.check_output("cat /etc/os-release | grep PRETTY_NAME", shell=True).decode().split("=")[1].strip('"\n'),
        "kernel": subprocess.check_output("uname -r", shell=True).decode().strip(),
        "rustc": subprocess.check_output("rustc --version", shell=True).decode().strip(),
        "podman": subprocess.check_output("podman --version", shell=True).decode().strip(),
    }
    print(f"System: {specs['cpu']} ({specs['cores']} threads)")
    print(f"OS: {specs['os']} (Kernel {specs['kernel']})")
    print(f"Toolchains: {specs['rustc']} | {specs['podman']}")
    print("-" * 70)

    work_dir = Path("/tmp/hyprcross_bench_project")
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True)
    setup_test_project(work_dir)

    results = {"specs": specs, "target": TARGET, "benchmarks": {}}

    # 1. Warm daemon start/stop lifecycle
    print("\n[1] Container Lifecycle")
    # ensure stopped
    subprocess.run(["hyprcross", "daemon", "stop", "--target", TARGET], cwd=work_dir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    
    results["benchmarks"]["daemon_start"] = bench("Daemon Warm Container Start", lambda: run_cmd(["hyprcross", "daemon", "start", "--target", TARGET], cwd=work_dir), runs=2, warmup=0)
    results["benchmarks"]["daemon_status"] = bench("Daemon Status Query", lambda: run_cmd(["hyprcross", "daemon", "status"], cwd=work_dir))

    # Pre-build dependencies once so incremental/null benchmarks measure toolchain overhead
    print("\n[2] Warming Cargo Dependencies in Container...")
    run_cmd(["hyprcross", "build", "--target", TARGET, "--daemon"], cwd=work_dir)

    print("\n[3] Null Check Latency (cargo check with zero changes)")
    results["benchmarks"]["null_check_daemon"] = bench("hyprcross check (Warm Daemon)", lambda: run_cmd(["hyprcross", "check", "--target", TARGET, "--daemon"], cwd=work_dir))
    results["benchmarks"]["null_check_cold"] = bench("hyprcross check (Cold Container --no-daemon)", lambda: run_cmd(["hyprcross", "check", "--target", TARGET, "--no-daemon"], cwd=work_dir))

    print("\n[4] Null Build Latency (cargo build with zero changes)")
    results["benchmarks"]["null_build_daemon"] = bench("hyprcross build (Warm Daemon)", lambda: run_cmd(["hyprcross", "build", "--target", TARGET, "--daemon"], cwd=work_dir))
    results["benchmarks"]["null_build_cold"] = bench("hyprcross build (Cold Container --no-daemon)", lambda: run_cmd(["hyprcross", "build", "--target", TARGET, "--no-daemon"], cwd=work_dir))

    print("\n[5] Incremental Build & Link Latency (touch src/main.rs)")
    def touch_main():
        main_rs = work_dir / "src" / "main.rs"
        main_rs.touch()

    def run_inc_hyprlnk_daemon():
        touch_main()
        return run_cmd(["hyprcross", "build", "--target", TARGET, "--linker", "hyprlnk", "--daemon"], cwd=work_dir)

    def run_inc_default_daemon():
        touch_main()
        return run_cmd(["hyprcross", "build", "--target", TARGET, "--linker", "default", "--daemon"], cwd=work_dir)

    def run_inc_hyprlnk_cold():
        touch_main()
        return run_cmd(["hyprcross", "build", "--target", TARGET, "--linker", "hyprlnk", "--no-daemon"], cwd=work_dir)

    def run_inc_default_cold():
        touch_main()
        return run_cmd(["hyprcross", "build", "--target", TARGET, "--linker", "default", "--no-daemon"], cwd=work_dir)

    results["benchmarks"]["incremental_hyprlnk_daemon"] = bench("Incremental Build + hyprlnk (Warm Daemon)", run_inc_hyprlnk_daemon)
    results["benchmarks"]["incremental_default_daemon"] = bench("Incremental Build + GNU ld (Warm Daemon)", run_inc_default_daemon)
    results["benchmarks"]["incremental_hyprlnk_cold"] = bench("Incremental Build + hyprlnk (Cold Container)", run_inc_hyprlnk_cold)
    results["benchmarks"]["incremental_default_cold"] = bench("Incremental Build + GNU ld (Cold Container)", run_inc_default_cold)

    print("\n[6] Target Test Execution (QEMU Emulation)")
    results["benchmarks"]["test_qemu_daemon"] = bench("hyprcross test (QEMU, Warm Daemon)", lambda: run_cmd(["hyprcross", "test", "--target", TARGET, "--daemon"], cwd=work_dir))
    results["benchmarks"]["test_qemu_cold"] = bench("hyprcross test (QEMU, Cold Container)", lambda: run_cmd(["hyprcross", "test", "--target", TARGET, "--no-daemon"], cwd=work_dir))

    print("\n[7] Target Binary Execution (hyprcross run)")
    results["benchmarks"]["run_qemu_daemon"] = bench("hyprcross run (QEMU, Warm Daemon)", lambda: run_cmd(["hyprcross", "run", "--target", TARGET, "--daemon"], cwd=work_dir))

    print("\n[8] Daemon Teardown")
    results["benchmarks"]["daemon_stop"] = bench("Daemon Stop & Remove (-t 0)", lambda: run_cmd(["hyprcross", "daemon", "stop", "--target", TARGET], cwd=work_dir), runs=2, warmup=0)

    # Save output
    out_json = Path(__file__).parent / "benchmark_results.json"
    with open(out_json, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nSaved raw results to: {out_json}")

    # Cleanup
    shutil.rmtree(work_dir, ignore_errors=True)
    print("=" * 70)

if __name__ == "__main__":
    main()
