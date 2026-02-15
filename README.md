# Silverfir-nano

A blazing-fast, ultra-compact WebAssembly 2.0 interpreter built from the ground up for performance, portability, and minimal footprint.

## Highlights

- **Extreme performance** — likely the fastest pure interpreter in the world *(benchmarks coming soon)*
- **Ultra-compact** — the `no_std` core is only ~200KB stripped, with zero runtime dependencies
- **`no_std`** — the core library requires only `alloc`; runs anywhere from embedded to bare-metal
- **Full WebAssembly 2.0** — multi-value, reference types, bulk memory operations, and more
- **Configurable fusion** — profile-guided instruction fusion for workload-specific optimization

## Performance

Silverfir-nano uses a carefully engineered interpreter architecture that eliminates overhead at every level:

| Technique | Impact |
|-----------|--------|
| `preserve_none` calling convention | Maximizes register usage across handler boundaries |
| TOS (Top-of-Stack) registers | Keeps 4 hot values in CPU registers, eliminating memory round-trips |
| Tail-call dispatch with next-handler preloading | Zero-overhead dispatch; branch predictor sees direct calls |
| Cross-language LTO | Rust + C handlers optimized as a single compilation unit |
| Fixed 32-byte instruction encoding | Cache-friendly, branchless decode |
| Instruction fusion | Merges 2–5 consecutive Wasm opcodes into single handlers |

> Benchmark data will be added here.

## Binary Size

| Build | Size | Features |
|-------|------|----------|
| `sf-nano-cli-minimal` (release) | **~200 KB** | `no_std`, no WASI, no fusion |
| `sf-nano-cli` (release) | ~1.1 MB | Full: WASI + fusion |

The minimal build includes the complete WebAssembly 2.0 interpreter with **zero external runtime dependencies**.

## `no_std`

The core library (`sf-nano-core`) is fully `#![no_std]`. It requires only `alloc` — no filesystem, no threads, no OS.
This makes it suitable for:

- Embedded systems and microcontrollers
- Custom runtimes and sandboxes
- Bare-metal environments
- Any platform with a heap allocator

## Instruction Fusion

Silverfir-nano supports profile-guided instruction fusion — automatically discovering and generating
optimized fused handlers from real workloads. This allows flexible trade-off between performance and
binary size, and workload-specific optimization.

See [FUSION.md](FUSION.md) for details.

## WebAssembly 2.0 Compatibility

Full support for the WebAssembly 2.0 specification:

- ✅ Multi-value returns
- ✅ Reference types (`funcref`, `externref`)
- ✅ Bulk memory operations
- ✅ Multiple tables
- ✅ Mutable globals import/export

Tested against the official [WebAssembly spec testsuite](https://github.com/WebAssembly/spec/tree/main/test).

## Project Structure

```
sf-nano-core/          Core interpreter library (no_std)
sf-nano-cli/           Full CLI runner (WASI + fusion)
sf-nano-cli-minimal/   Minimal no_std CLI runner (~200KB)
sf-nano-spectest/      WebAssembly spec test runner
sf-nano-wasitest/      WASI test runner
benchmarks/            Benchmark Wasm binaries
```

## Building

```bash
# Full build (WASI + fusion)
cargo build --release

# Release with debug symbols
cargo build --profile release-with-debug
```

## Usage

```bash
# Run a WASI program
sf-nano-cli program.wasm [args...]
```

## License

MIT / Apache-2.0
