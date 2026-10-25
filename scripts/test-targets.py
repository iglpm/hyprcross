#!/usr/bin/env python3
"""Build every advertised target and exercise installed SDKs and available runners.

Run after `cargo build`: python3 scripts/test-targets.py --hyprcross target/debug/hyprcross
Logs and machine-readable results are kept under --out. SDKs and Rust targets must
already be installed, or hyprcross will offer its usual automatic installation.
"""

import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent

STD_PROGRAM = r'''
use std::cell::Cell;
thread_local! { static VALUE: Cell<u32> = const { Cell::new(7) }; }
fn main() {
    let mut values = vec![3, 1, 2];
    values.sort();
    assert_eq!(values, [1, 2, 3]);
    assert_eq!(format!("{}", values.iter().sum::<u32>()), "6");
    #[cfg(not(target_family = "wasm"))]
    {
        assert_eq!(VALUE.with(Cell::get), 7);
        assert_eq!(std::thread::spawn(|| {
            VALUE.with(|v| { v.set(42); v.get() })
        }).join().unwrap(), 42);
        assert_eq!(VALUE.with(Cell::get), 7);
        assert!(std::panic::catch_unwind(|| panic!("expected smoke-test panic")).is_err());
    }
    println!("hyprcross smoke passed");
}
'''

BARE_PROGRAM = r'''
#![no_std]
#![no_main]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop { core::hint::spin_loop(); } }
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! { loop { core::hint::spin_loop(); } }
'''


def run(argv, cwd, env, logfile, timeout):
    try:
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=timeout)
        logfile.write_bytes(result.stdout + result.stderr)
        return {"status": "pass" if result.returncode == 0 else "fail",
                "exit_code": result.returncode, "log": str(logfile)}
    except (OSError, subprocess.TimeoutExpired) as error:
        logfile.write_text(str(error))
        return {"status": "fail", "error": str(error), "log": str(logfile)}


def can_run(target, host):
    if target == host:
        return True
    if "-linux-" in target and "android" not in target and platform.system() == "Linux":
        arch = target.split("-")[0]
        if arch == host.split("-")[0] and target.endswith("musl"):
            return True
        arch = {"i686": "i386", "armv7": "arm", "riscv64gc": "riscv64",
                "powerpc64le": "ppc64le", "powerpc64": "ppc64", "powerpc": "ppc"}.get(arch, arch)
        return bool(shutil.which(f"qemu-{arch}-static") or shutil.which(f"qemu-{arch}"))
    if "windows-gnu" in target and target.startswith(("x86_64-", "i686-")):
        return bool(shutil.which("wine"))
    if "wasip" in target:
        return bool(shutil.which("wasmtime"))
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hyprcross", default=str(ROOT / "target/debug/hyprcross"))
    parser.add_argument("--builder", default="cargo", choices=["cargo", "igl"])
    parser.add_argument("--target", action="append", help="Only test this target (repeatable)")
    parser.add_argument("--out", type=Path, default=ROOT / "target/target-matrix")
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--native", action="store_true", help="Also compile a C library against each SDK's headers with cc-rs")
    args = parser.parse_args()
    binary = str(Path(args.hyprcross).resolve())
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["XDG_CACHE_HOME"] = str(out / "cache")
    env["CARGO_TARGET_DIR"] = str(out / "build")
    env["IGEL_HOME"] = str(out / "igel-home")
    env["CARGO_TERM_COLOR"] = "never"
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    host = re.search(r"^host: (.+)$", rustc, re.M)[1]
    source = (ROOT / "src/targets.rs").read_text().split("pub const ALL_TARGETS:")[1].split("];", 1)[0]
    targets = args.target or re.findall(r'"([\w-]+)"', source)
    report = {"date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "host": host, "rustc": rustc, "builder": args.builder, "native": args.native, "results": []}
    for target in targets:
        native = args.native and "-none-" not in target and not target.startswith("wasm")
        project = out / ("bare" if "-none-" in target else "native" if native else "std")
        (project / "src").mkdir(parents=True, exist_ok=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname = "hyprcross-smoke"\nversion = "0.0.0"\nedition = "2024"\n'
            '[workspace]\n[profile.dev]\npanic = "abort"\n' if "-none-" in target else
            '[package]\nname = "hyprcross-smoke"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n')
        (project / "src/main.rs").write_text(BARE_PROGRAM if "-none-" in target else STD_PROGRAM)
        if native:
            with (project / "Cargo.toml").open("a") as manifest:
                manifest.write('[build-dependencies]\ncc = "1"\n')
            (project / "build.rs").write_text('fn main() { cc::Build::new().file("probe.c").compile("sdk_probe"); }\n')
            (project / "probe.c").write_text('#include <stdint.h>\n#include <stdlib.h>\n#include <stdio.h>\n#include <string.h>\nint sdk_answer(void) { return (int)strlen("SDK") + 39; }\n')
            (project / "src/main.rs").write_text(STD_PROGRAM.replace(
                'fn main() {', 'unsafe extern "C" { fn sdk_answer() -> i32; }\nfn main() {\n    assert_eq!(unsafe { sdk_answer() }, 42);', 1))
        result = {"target": target}
        if "-none-" not in target and not target.startswith("wasm"):
            result["sdk"] = run([binary, "sdk", "install", target], project, env,
                                out / f"{target}.sdk.log", args.timeout)
        command = [binary, "build", "--builder", args.builder, "-t", target, "--offline"]
        result["build"] = run(command, project, env, out / f"{target}.build.log", args.timeout)
        if result["build"]["status"] == "pass" and can_run(target, host):
            command[1] = "run"
            result["run"] = run(command, project, env, out / f"{target}.run.log", args.timeout)
        else:
            result["run"] = {"status": "skip", "reason": "build failed" if result["build"]["status"] != "pass" else "no compatible runner installed"}
        report["results"].append(result)
        (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f'{target:42} build={result["build"]["status"]:4} run={result["run"]["status"]}', flush=True)
    return int(any(stage.get("status") == "fail" for row in report["results"]
                   for stage in row.values() if isinstance(stage, dict)))


if __name__ == "__main__":
    sys.exit(main())
