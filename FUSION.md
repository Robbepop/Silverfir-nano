# Instruction Fusion

Silverfir-nano ships with a built-in fusion set (enabled by default), and most users do not
need to run custom per-app fusion.

The built-in set was derived from many different workloads and compiler output patterns
(not one specific app binary), and in typical real programs it already captures about
~90% of the benefit.

Custom fusion is still available as a size/perf tuning tool for niche workloads.

## Do You Need Custom Fusion?

Short answer: usually no.

- Use the default fusion-enabled build for general use
- Expect custom per-app fusion to provide smaller incremental gains in most cases
- The headline benchmark numbers are from fusion-enabled builds, not the ultra-minimal `~200KB` profile
- The `~200KB` size-first profile (`default-features = false`) is typically around `~40%` slower, but still fast (roughly wasm3-class)

## Overview

Instruction fusion combines 2–5 consecutive WebAssembly opcodes into a single handler,
eliminating dispatch overhead, TOS register shuffles, and branch mispredictions.
For custom tuning, the system is fully automatic: profile a workload, discover optimal
patterns, rebuild.

## Workflow

```
Profile               Discover              Build
───────               ────────              ─────
Run workload    →   Analyze traces    →   Code generation
with profiler       select candidates     Rust + C handlers
enabled             greedy algorithm      auto-generated
                    write TOML
```

## Quick Start

Build with profiling enabled and run the discovery tool:

```bash
# Build with profiling support
cargo build --features profile

# Single workload — discover top 500 patterns from coremark
cargo run --features profile --bin sf-nano-cli -- \
  discover-fusion --top 500 --window 5 ./benchmarks/coremark/coremark.wasm

# Output: handlers_fused_discovered.toml (in CWD)
```

To install the discovered patterns:

```bash
cp handlers_fused_discovered.toml sf-nano-core/src/vm/interp/fast/handlers_fused.toml
cargo build --release
```

## discover-fusion CLI Reference

```
USAGE:
  sf-nano-cli discover-fusion [OPTIONS] <wasm-file> [-- args...]
  sf-nano-cli discover-fusion [OPTIONS] --workload <wasm> [args...] ...
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --window <N>` | 4 | N-gram window size for profiling (2–8) |
| `-n, --top <N>` | 32 | Maximum number of fusion candidates to output |
| `--min-savings <N>` | 1000 | Minimum dispatch savings to include a candidate |
| `-o, --output <PATH>` | `handlers_fused_discovered.toml` | Output TOML file path |
| `--show-trie` | off | Print the pattern trie before discovery |
| `--workload <WASM> [ARGS...]` | — | Add a workload to profile (repeatable) |
| `-h, --help` | — | Print help message |

### Single Workload

Profile one WASM module. All arguments after the `.wasm` path (or after `--`) are
passed to the WASM program:

```bash
sf-nano-cli discover-fusion --top 500 --window 5 coremark.wasm
```

### Multi-Workload Merging

Profile multiple workloads and merge their statistics to discover patterns that
generalize well across diverse programs:

```bash
sf-nano-cli discover-fusion --top 500 --window 5 \
  --workload ./app1.wasm \
  --workload ./app2.wasm
```

Each `--workload` flag starts a new workload. Arguments after the `.wasm` path
(up to the next `--workload` or global option) are passed to that WASM program.

**How merging works:**

1. Each workload is profiled independently, capturing N-gram instruction sequences
2. Raw counts are normalized to frequencies (count ÷ total instructions)
3. Frequencies are averaged across all workloads
4. Averaged frequencies are scaled back to absolute counts using the mean total

This ensures:
- Patterns common to all workloads are ranked highest
- Workload-specific patterns are proportionally down-weighted
- Workloads of different sizes contribute equally (frequency normalization)

## Discovery Pipeline

The discovery algorithm processes profiled traces through these stages:

1. **Build pattern trie** — constructs an N-gram trie with counts for all prefix lengths
2. **Filter** — removes patterns with control flow in the middle, more than one memory op,
   or encoding budgets exceeding 192 bits (3 × 64-bit immediate slots)
3. **Validate TOS** — ensures the fused stack effect matches a supported pattern
4. **Greedy select** — picks highest-savings patterns first, adjusting counts for prefix overlaps
5. **Generate TOML** — writes `[[fused]]` entries with encoding fields, TOS patterns, and names

The algorithm automatically:
- Computes stack effects by simulating push/pop through the sequence
- Generates encoding field layouts (which immediates to pack, bit widths, source indices)
- Names fused instructions by abbreviating constituent ops (e.g., `local_get + i32_const + i32_add` → `get_const_add`)
- Handles name collisions with existing handlers

## Build Integration

The build system reads `handlers_fused.toml` and generates:

| Output | Contents |
|--------|----------|
| `fast_fusion.rs` | `FusedOp` enum, `OpFuser` pattern matcher |
| `fast_fusion_emit.rs` | `emit_fused()` dispatch, spill/fill helpers |
| `fast_fused_handlers.inc` | C handler implementations |
| `fast_c_wrappers.inc` | C `op_*` wrapper functions (fused section) |

## Performance vs Size Trade-off

| Configuration | Binary Size | Dispatch Overhead |
|--------------|-------------|-------------------|
| No fusion (`default-features = false`) | ~200 KB | Baseline (size-first, typically ~40% slower) |
| Fusion enabled (default) | ~1.1 MB | Significantly reduced |
| Custom fusion (profiled for your workload) | Varies | Usually incremental gains beyond default |

Notes:
- Fusion has diminishing returns: a full fusion set is about `~500KB`, but adding only `~100KB`
  can already recover roughly `~80%` of full-fusion performance.
- The `~1.1MB` full binary also includes `std` due to WASI support; if you do not need WASI,
  you can save several hundred KB.

## Example: Top Patterns from Coremark

```
get_const             [local_get → i32_const]                              1.8M savings (7.8%)
get_get               [local_get → local_get]                              1.2M savings (5.2%)
set_get               [local_set → local_get]                              1.1M savings (4.8%)
get_const_add         [local_get → i32_const → i32_add]                    1.1M savings (4.8%)
get_const_add_set_get [local_get → i32_const → i32_add → local_set → local_get]  1.0M savings (4.4%)
```

## Disabling Fusion

```toml
# In Cargo.toml dependency
sf-nano-core = { path = "...", default-features = false }
```

Or delete `handlers_fused.toml` — the build system generates empty stubs and the
interpreter falls back to one handler per opcode.
