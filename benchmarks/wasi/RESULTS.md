# WASI Benchmark Results

Run with `run_tests.py` on macOS (Apple Silicon).

## Results

| Test | Silverfir | wasm3 | wasmi | WAMR (fast) | wasmtime (winch) | wasmtime (cranelift) |
|------|-----------|-------|-------|-------------|------------------|----------------------|
| mandelbrot/mandel.wasm | 2992 ms | 3955 ms | 12031 ms | 6512 ms | 2142 ms | 852 ms |
| c-ray/c-ray.wasm | 3699 ms | 5052 ms | 8801 ms | 7228 ms | 1575 ms | 404 ms |
| stream (Copy) | 7805 MB/s | 3326 MB/s | 1799 MB/s | 2926 MB/s | 15089 MB/s | 44067 MB/s |
| stream (Scale) | 8742 MB/s | 4491 MB/s | 2113 MB/s | 3903 MB/s | 26553 MB/s | 49644 MB/s |
| stream (Add) | 10355 MB/s | 4907 MB/s | 2726 MB/s | 4597 MB/s | 24407 MB/s | 48280 MB/s |
| stream (Triad) | 8634 MB/s | 4639 MB/s | 2336 MB/s | 4209 MB/s | 21288 MB/s | 48280 MB/s |
| brotli/brotli.wasm | FAIL | 1.062 s | FAIL | 1.710 s | 0.341 s | 0.222 s |
| coremark | 8866 | 4235 | 2172 | 3195 | 9071 | 14964 |
| lua/fib | 6.82 s | 9.94 s | 16.82 s | 13.35 s | 6.14 s | 4.58 s |

## Notes

- Silverfir: `sf-nano-cli` (release build, 1000 fused patterns, max-freq merge, window=8)
- wasm3: `build-release/wasm3`
- wasmi: `wasmi_cli` v0.42
- WAMR: `iwasm-2.4.3` (fast interpreter, release build)
- wasmtime (winch): single-pass JIT compiler (`-C compiler=winch`)
- wasmtime (cranelift): optimizing JIT compiler (default)
- Stream: higher MB/s is better; all others: lower is better (except CoreMark: higher is better)
- brotli fails on Silverfir (out of bounds memory access) and wasmi (exit code 2)
