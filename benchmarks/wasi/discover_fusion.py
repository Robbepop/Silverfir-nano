#!/usr/bin/env python3
"""Run discover-fusion across all WASI benchmarks to generate a merged fusion table.

Builds with --features profile, then runs discover-fusion with all workloads
combined. Each workload is first tested individually with a timeout to skip
workloads that are too slow for profiling.

Usage:
    python3 discover_fusion.py                  # default: top 500, window 8
    python3 discover_fusion.py --top 300 -w 4   # custom settings
    python3 discover_fusion.py --install         # also copy result to build tree
"""

import argparse
import os
import subprocess
import sys
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, "..", ".."))
CLI = os.path.join(REPO_ROOT, "target", "release", "sf-nano-cli")
OUTPUT_DEFAULT = os.path.join(REPO_ROOT, "handlers_fused_discovered.toml")
INSTALL_PATH = os.path.join(
    REPO_ROOT, "sf-nano-core", "src", "vm", "interp", "fast", "handlers_fused.toml"
)

# Per-workload timeout in seconds.
DEFAULT_TIMEOUT = 30

# Workloads for the profiling interpreter.
# Excluded: stream (memory-bound, not compute).
WORKLOADS = [
    {
        "name": "mandelbrot/mandel.wasm",
        "cwd": os.path.join(SCRIPT_DIR, "mandelbrot"),
        "args": ["mandel.wasm", "16", "2e3"],
    },
    {
        "name": "c-ray/c-ray.wasm",
        "cwd": os.path.join(SCRIPT_DIR, "c-ray"),
        "args": ["c-ray.wasm", "-s", "32x32", "-i", "scene"],
    },
    {
        "name": "coremark/coremark.wasm",
        "cwd": os.path.join(SCRIPT_DIR, "coremark"),
        "args": ["coremark.wasm"],
    },
    {
        "name": "lua/fib_small",
        "cwd": os.path.join(SCRIPT_DIR, "lua"),
        "args": ["lua.wasm", "fib_small.lua"],
    },
    {
        "name": "lua/sunfish",
        "cwd": os.path.join(SCRIPT_DIR, "lua"),
        # nodes=20 time=3: reduce work for profiling (1000 nodes = 3min+ under profiling)
        "args": ["lua.wasm", "sunfish.lua", "20", "3"],
    },
    {
        "name": "lua/json_bench",
        "cwd": os.path.join(SCRIPT_DIR, "lua"),
        # time=3 users=20: reduce work for profiling (200 users = very slow under profiling)
        "args": ["lua.wasm", "json_bench.lua", "3", "20"],
    },
]


def build_profiling():
    print("Building with --features profile ...")
    r = subprocess.run(
        ["cargo", "build", "--release", "--features", "profile", "--bin", "sf-nano-cli"],
        cwd=REPO_ROOT,
    )
    if r.returncode != 0:
        print("ERROR: build failed", file=sys.stderr)
        sys.exit(1)
    print()


def workload_cli_args(w):
    """Return the --workload ... --dir ... fragment for a single workload."""
    args = ["--workload", os.path.join(w["cwd"], w["args"][0])]
    args.extend(w["args"][1:])
    args.extend(["--dir", w["cwd"]])
    return args


def test_workload(w, timeout, window):
    """Run a single workload with timeout. Returns (ok, elapsed_secs)."""
    wasm_path = os.path.join(w["cwd"], w["args"][0])
    if not os.path.exists(wasm_path):
        return None, 0  # file not found

    cmd = [CLI, "discover-fusion", "--top", "1", "--window", str(window),
           "-o", "/dev/null"]
    cmd.extend(workload_cli_args(w))

    t0 = time.time()
    try:
        subprocess.run(cmd, cwd=REPO_ROOT, timeout=timeout,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        elapsed = time.time() - t0
        return True, elapsed
    except subprocess.TimeoutExpired:
        elapsed = time.time() - t0
        return False, elapsed


def run_discover(workloads, top, window, output):
    """Run combined discover-fusion with the given workloads."""
    cmd = [CLI, "discover-fusion", "--top", str(top), "--window", str(window),
           "--output", output]

    for w in workloads:
        cmd.extend(workload_cli_args(w))

    print(f"Running discover-fusion (top={top}, window={window}) ...")
    print(f"  Workloads: {len(workloads)}")
    print(f"  Output: {output}")
    print()

    r = subprocess.run(cmd, cwd=REPO_ROOT, stdout=subprocess.DEVNULL)
    return r.returncode


def main():
    parser = argparse.ArgumentParser(
        description="Discover fusion patterns from WASI benchmarks")
    parser.add_argument("--top", type=int, default=500,
                        help="Max fusion candidates (default: 500)")
    parser.add_argument("-w", "--window", type=int, default=8,
                        help="N-gram window size (default: 8)")
    parser.add_argument("-o", "--output", default=OUTPUT_DEFAULT,
                        help="Output TOML path")
    parser.add_argument("--install", action="store_true",
                        help="Copy result to handlers_fused.toml in build tree")
    parser.add_argument("--skip-build", action="store_true",
                        help="Skip cargo build (use existing binary)")
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT,
                        help=f"Per-workload timeout in seconds (default: {DEFAULT_TIMEOUT})")
    args = parser.parse_args()

    if not args.skip_build:
        build_profiling()

    if not os.path.exists(CLI):
        print(f"ERROR: {CLI} not found", file=sys.stderr)
        print("Run without --skip-build to build first", file=sys.stderr)
        sys.exit(1)

    # Test each workload individually with timeout
    print(f"Testing workloads (timeout={args.timeout}s each):")
    print(f"{'Workload':<30} {'Status':<12} {'Time':>8}")
    print("-" * 52)

    passing = []
    for w in WORKLOADS:
        ok, elapsed = test_workload(w, args.timeout, args.window)
        if ok is None:
            status = "SKIP (missing)"
            print(f"{w['name']:<30} {status:<12} {'':>8}")
        elif ok:
            status = "OK"
            print(f"{w['name']:<30} {status:<12} {elapsed:>7.1f}s")
            passing.append(w)
        else:
            status = "TIMEOUT"
            print(f"{w['name']:<30} {status:<12} {elapsed:>7.1f}s")

    print("-" * 52)
    print(f"{len(passing)}/{len(WORKLOADS)} workloads passed")
    print()

    if not passing:
        print("ERROR: no workloads completed within timeout", file=sys.stderr)
        sys.exit(1)

    # Run combined discovery with passing workloads
    sys.stdout.flush()
    rc = run_discover(passing, args.top, args.window, args.output)
    if rc != 0:
        print(f"\nERROR: discover-fusion exited with code {rc}", file=sys.stderr)
        sys.exit(rc)

    print(f"\nWrote {args.output}")

    if args.install:
        import shutil
        shutil.copy2(args.output, INSTALL_PATH)
        print(f"Installed to {INSTALL_PATH}")
        print("Run 'cargo build --release' to rebuild with new fusion table")


if __name__ == "__main__":
    main()
