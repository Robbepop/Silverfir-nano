# WASI Benchmark Results

Run with `run_tests.py` on macOS (Apple Silicon).

## Results

| Test | Silverfir | wasm3 | wasmi | WAMR (fast) | wasmtime (winch) | wasmtime (cranelift) |
|------|-----------|-------|-------|-------------|------------------|----------------------|
| mandelbrot/mandel.wasm | 2988 ms | 3955 ms | 11820 ms | 6512 ms | 2142 ms | 852 ms |
| c-ray/c-ray.wasm | 3684 ms | 5052 ms | — | 7228 ms | 1575 ms | 404 ms |
| stream (Copy) | 7635 MB/s | 3326 MB/s | 1814 MB/s | 2926 MB/s | 15089 MB/s | 44067 MB/s |
| stream (Scale) | 9346 MB/s | 4491 MB/s | 2020 MB/s | 3903 MB/s | 26553 MB/s | 49644 MB/s |
| stream (Add) | 10733 MB/s | 4907 MB/s | 2709 MB/s | 4597 MB/s | 24407 MB/s | 48280 MB/s |
| stream (Triad) | 9122 MB/s | 4639 MB/s | 2342 MB/s | 4209 MB/s | 21288 MB/s | 48280 MB/s |
| coremark | 9283 | 4235 | 2136 | 3195 | 9071 | 14964 |
| lua/fib | 5.93 s | 9.94 s | 17.25 s | 13.35 s | 6.14 s | 4.58 s |
| lua/sunfish | 1171 | 764 | 484 | 635 | 1657 | 2703 |
| lua/json_bench | 2391 | 1603 | 1163 | 1427 | 5351 | 9616 |

## Notes

- Silverfir: `sf-nano-cli` (release build, 1500 fused patterns, max-freq merge, window=8)
- wasm3: `build-release/wasm3` 79d412ea5fcf92f0efe658d52827a0e0a96ff442
- wasmi: `wasmi_cli` v1.0.9
- WAMR: `iwasm-2.4.3` (fast interpreter, release build)
- wasmtime (winch): single-pass JIT compiler (`-C compiler=winch`)
- wasmtime (cranelift): optimizing JIT compiler (default)
- Stream: higher MB/s is better; all others: lower is better (except CoreMark, sunfish, json_bench: higher is better)
- c-ray/wasmi: fails with exit code 2 (stdin piping issue)
