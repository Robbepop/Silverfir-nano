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

### Step 1: Profile (optional, only for custom fusion)

Most users can skip this step and use the built-in fusion set.

Build with the `trace` feature and run a representative workload. The profiler captures
N-instruction sliding windows (configurable, up to 8-grams) and records handler sequence
frequencies using a lock-free recording path.

```bash
# Build with profiling enabled
# (profiler hooks into every handler dispatch via FAST_PROFILE_ENABLED)

# Run target workload — the profiler captures instruction sequence statistics
sf-nano-cli workload.wasm
```

### Step 2: Discover (optional, only for custom fusion)

The discovery tool analyzes profiled sequences through a multi-stage pipeline:

1. **Normalize** — aggregates all TOS variants (D1–D4) of each opcode together
2. **Build pattern trie** — constructs an N-gram trie with counts for all prefix lengths
3. **Filter** — removes patterns with control flow in the middle, more than one memory op,
   or encoding budgets exceeding 192 bits (3 × 64-bit immediate slots)
4. **Validate TOS** — ensures the fused stack effect matches a supported pattern
5. **Greedy select** — picks highest-savings patterns first, adjusting counts for prefix overlaps
6. **Generate TOML** — writes `handlers_fused.toml` with encoding fields, TOS patterns, and names

The discovery algorithm automatically:
- Computes stack effects by simulating push/pop through the sequence
- Generates encoding field layouts (which immediates to pack, bit widths, source indices)
- Names fused instructions by abbreviating constituent ops (e.g., `local_get + i32_const + i32_add` → `get_const_add`)
- Handles name collisions with existing handlers

### Step 3: Build

Rebuild the project. The build system reads `handlers_fused.toml` and generates:

| Output | Contents |
|--------|----------|
| `fast_fusion.rs` | `FusedOp` enum, `OpFuser` pattern matcher |
| `fast_fusion_emit.rs` | `emit_fused()` dispatch, spill/fill helpers |
| `fast_fused_handlers.inc` | C handler implementations |
| `fast_c_wrappers.inc` | C `op_*` wrapper functions (fused section) |

```bash
cargo build --release
```

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

## Example: Top Patterns from Spec Tests

```
get_const        [local_get → i32_const]           2.9M dispatch savings
get_get          [local_get → local_get]            2.0M dispatch savings
set_get          [local_set → local_get]            1.8M dispatch savings
get_const_add    [local_get → i32_const → i32_add]  1.8M dispatch savings
```

## Disabling Fusion

```toml
# In Cargo.toml dependency
sf-nano-core = { path = "...", default-features = false }
```

Or delete `handlers_fused.toml` — the build system generates empty stubs and the
interpreter falls back to one handler per opcode.
