#!/usr/bin/env python3
"""Measure warm direct SDK builds (no containers), checking every warm-up and sample.

Build release binaries first. Example:
  python3 benches/bench_runner.py --hyprcross target/release/hyprcross \
      --igl ../target/release/igl --runs 15
"""
import argparse
import datetime
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


def version(command):
    return subprocess.check_output(command, text=True).strip().splitlines()[0]


def cpu_model():
    try:
        for line in Path('/proc/cpuinfo').read_text().splitlines():
            if line.startswith('model name'):
                return line.split(':', 1)[1].strip()
    except OSError:
        pass
    return platform.processor()


def measure(argv, project, env, runs):
    def invoke():
        start = time.perf_counter()
        result = subprocess.run(argv, cwd=project, env=env, capture_output=True)
        elapsed = (time.perf_counter() - start) * 1000
        if result.returncode:
            sys.exit(f"failed: {argv}\n{result.stderr.decode(errors='replace')}")
        return elapsed
    invoke()
    samples = [round(invoke(), 3) for _ in range(runs)]
    return {'median_ms': round(statistics.median(samples), 3),
            'min_ms': min(samples), 'samples_ms': samples}


def executable(value):
    found = shutil.which(value)
    # Keep launcher symlinks such as ~/.cargo/bin/cargo intact. Resolving that
    # symlink produces the rustup binary and changes argv[0], so rustup no
    # longer knows which tool was requested.
    path = Path(found or value).absolute()
    if not path.is_file():
        sys.exit(f'executable not found: {value}')
    return str(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=int, default=15)
    parser.add_argument('--target', default='aarch64-unknown-linux-gnu')
    parser.add_argument('--project', type=Path, default=ROOT)
    parser.add_argument('--hyprcross', default=os.environ.get('HYPRCROSS', 'hyprcross'))
    parser.add_argument('--igl', default=os.environ.get('IGL', 'igl'))
    parser.add_argument('--builder', action='append', choices=['cargo', 'igl'])
    parser.add_argument('--out', type=Path, default=ROOT / 'benches/benchmark_results.json')
    args = parser.parse_args()
    if args.runs < 1:
        parser.error('--runs must be positive')
    binary = executable(args.hyprcross)
    builders = args.builder or ['cargo']
    paths = {b: executable(args.igl if b == 'igl' else b) for b in builders}
    host = next(line.split(': ', 1)[1] for line in subprocess.check_output(
        ['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
    env = os.environ.copy()
    env['PATH'] = os.pathsep.join([str(Path(binary).parent),
                                  *[str(Path(p).parent) for p in paths.values()], env.get('PATH', '')])
    env['CARGO_TERM_COLOR'] = 'never'
    cases = []
    for builder in builders:
        cases.extend([
            (f'{builder} build, host (without hyprcross)',
             [paths[builder], 'build', '--offline', '--target', host]),
            (f'hyprcross build, {builder}, host (direct)',
             [binary, 'build', '--builder', builder, '-t', host, '--offline']),
            (f'hyprcross build, {builder}, {args.target} (direct SDK)',
             [binary, 'build', '--builder', builder, '-t', args.target, '--offline']),
        ])
    cases.append(('hyprcross --version', [binary, '--version']))
    report = {'date': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'runs': args.runs, 'host': host, 'target': args.target,
              'workload': args.project.resolve().name,
              'method': 'one checked warm-up immediately before each case; sequential warm no-op builds; offline',
              'environment': {'cpu': cpu_model(), 'kernel': platform.release(),
                              'rustc': version(['rustc', '--version']),
                              'hyprcross': version([binary, '--version']),
                              **{b: version([p, '--version']) for b, p in paths.items()}},
              'results': []}
    for name, argv in cases:
        print(f'Warming and measuring {name} ...', file=sys.stderr, flush=True)
        result = measure(argv, args.project.resolve(), env, args.runs)
        print(f"{name}: median {result['median_ms']:.3f} ms; min {result['min_ms']:.3f} ms", flush=True)
        report['results'].append({'case': name, **result})
        args.out.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Results: {args.out}', file=sys.stderr)


if __name__ == '__main__':
    main()
