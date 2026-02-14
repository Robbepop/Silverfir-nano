# sf-nano: Minimal no_std WebAssembly 2.0 Interpreter

## Problem Statement

Silverfir-rs (sf-core) is a fully-featured WASM 3.0 interpreter/compiler with 3 backends (SSA, fast, inplace), WASI support, GC heap, and numerous std-dependent crates. For use cases requiring a **small binary and fast execution**, the full sf-core is too large.

**Goal:** Create `sf-nano` — a standalone `#![no_std]` crate that extracts the **fast interpreter** from sf-core, adapted for minimal binary size while preserving execution speed. WASM 2.0 compliance (MVP + bulk-memory, reference-types, multi-value, sign-extension, etc.). No built-in WASI; provide external function hooks so WASI can be supplied as an external crate. Must be able to run spectest and CoreMark (via external WASI).

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| WASM spec level | **WASM 2.0** (no GC/3.0) | Minimal binary; GC adds ~1,500 LOC across gc_heap, gc_type_check, ref_ops, type_context |
| Instruction fusion | **Always enabled** | Contributes ~40% of performance; non-negotiable |
| Profiler & trace | **Removed entirely** | Debug-only tools (~900 LOC); not needed in production |
| Validation | **Feature-gated** (`validate`, off by default) | ~3,580 LOC; not needed for trusted modules. Validator is pure validation only — jump table and max_stack_height computation removed (fast interpreter builds its own IR) |
| HashMap | **Replaced with Vec + linear scan** | Few imports in practice; avoids hashbrown dependency |
| External functions | **Function pointer**: `fn(&[Value], &mut [Value]) -> Result<(), WasmError>` | Zero-alloc, no_std friendly, multi-value via caller buffer |
| Thread-local stack | **Allocated stack, pointer stored in context** | No `thread_local!`; stack allocated and pointer held in VM |
| Rc/RefCell | **Removed** — use owned/borrowed directly | Single-module model; no shared cross-module state needed |
| Multi-module linking | **Single module only** | Simplest, smallest; host functions via external fn hooks |
| Linkable abstraction | **Removed entirely** | No inter-module linking → no LinkableData/LinkableInstance |
| Logging | **Removed** — no-op macros that compile to nothing | Zero binary impact |
| `paste` crate | **Keep** (build-dep, proc-macro) | Zero runtime cost; simplifies handler codegen |
| `std` feature | **None** — pure no_std only | Keeps the crate focused |

## Approach

**Fork & adapt** the fast interpreter code from sf-core. All code copied and rewritten for `#![no_std]` + `extern crate alloc`:

- `std::*` → `core::*` / `alloc::*` (Vec, Box, String, format!, etc.)
- Remove `Rc<RefCell<>>` → owned values, `&mut` borrows
- Remove `thiserror` → hand-implement `Display`
- Remove `num_enum` → hand-implement `TryFrom<u8>` for enums
- Remove `log`/`env_logger` → no-op macros
- Remove `smallvec` → `alloc::vec::Vec`
- Remove `anyhow`, `serde_json`, `rand`, `fixedbitset`, `filetime`, `windows-sys`
- Keep C handler/trampoline build pipeline (`cc` build-dep only)
- Strip all WASM 3.0 GC code paths (struct/array types, gc_heap, gc_type_check, GC ref ops)
- Strip profiler, trace, SSA interpreter, inplace interpreter, XIR backend, compiler pipeline

## Architecture

```
crates/sf-nano/
├── Cargo.toml                # no_std, zero runtime deps
├── build.rs                  # C handler compilation (from sf-core build/)
├── build/                    # Build-time codegen (subset of sf-core's)
│   ├── compile.rs
│   └── fast_interp/          # Handler codegen for fast interpreter
├── src/
│   ├── lib.rs                # #![no_std], extern crate alloc, public API
│   ├── error.rs              # WasmError (hand-impl Display, no thiserror/Backtrace)
│   ├── value_type.rs         # WASM 2.0 value types (hand-impl TryFrom, no num_enum)
│   ├── opcodes.rs            # WASM 2.0 opcodes (hand-impl TryFrom, no num_enum)
│   ├── constants.rs          # WASM spec constants
│   ├── op_decoder.rs         # Streaming opcode decoder
│   ├── module/
│   │   ├── mod.rs            # Module struct & parsing entry
│   │   ├── parser.rs         # WASM binary parser
│   │   ├── entities.rs       # Simplified: FunctionSpec, TableSpec, MemorySpec, etc.
│   │   │                     #   (no LinkableData, no Rc<RefCell<>>, no GC types)
│   │   ├── type_context.rs   # Function-type-only equivalence (no GC type hierarchy)
│   │   ├── type_defs.rs      # FunctionType only (no StructType/ArrayType/DefType)
│   │   ├── validator.rs      # [feature = "validate"] Module validation
│   │   └── builder.rs        # Module builder
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── leb128.rs         # LEB128 encoding (hand-impl errors)
│   │   ├── limits.rs         # Memory/table limits (hand-impl errors)
│   │   └── payload.rs        # Binary stream reader (hand-impl errors)
│   └── vm/
│       ├── mod.rs            # VM struct: parse → instantiate → invoke
│       ├── store.rs          # Simplified Store (Vec-based, single module, no HashMap)
│       ├── entities.rs       # Simplified instances (owned, no Rc<RefCell<>>)
│       ├── value.rs          # RefHandle (funcref/externref only), Value enum
│       └── interp/
│           ├── mod.rs        # Direct fast interpreter entry (no backend selection)
│           ├── raw_value.rs  # RawValue = u64, zero-cost conversions
│           ├── stack.rs      # InterpreterStack (pointer-based, no thread_local)
│           └── fast/         # === THE FAST INTERPRETER ===
│               ├── mod.rs
│               ├── instruction.rs    # 32-byte Instruction struct
│               ├── encoding.rs       # (generated from encoding.toml)
│               ├── context.rs        # Execution context (owned refs, no Rc)
│               ├── fast_code.rs      # FastCode storage (Box<[Instruction]>)
│               ├── frame_layout.rs   # Stack frame layout constants
│               ├── precompile.rs     # Module → fast IR compilation
│               ├── runtime.rs        # Eval entry point (no thread_local)
│               ├── builder/          # Single-pass WASM → fast IR compiler
│               │   ├── mod.rs
│               │   ├── context.rs
│               │   ├── dispatch.rs   # (GC opcodes stripped)
│               │   ├── emitter.rs
│               │   ├── finalizer.rs
│               │   ├── stack.rs
│               │   └── temp_inst.rs
│               ├── handlers/         # Rust handlers
│               │   ├── mod.rs
│               │   ├── common.rs
│               │   ├── call.rs       # Includes external fn call via fn pointer
│               │   ├── control.rs
│               │   ├── global.rs
│               │   ├── memory.rs
│               │   ├── ref_ops.rs    # ref.null, ref.is_null, ref.func only (no GC)
│               │   └── table.rs
│               ├── handlers_c/       # C handlers (perf-critical ops)
│               │   ├── semantics.h
│               │   ├── arithmetic.c
│               │   ├── bitwise.c
│               │   ├── call.c
│               │   ├── comparison.c
│               │   ├── const_local.c
│               │   ├── control.c
│               │   ├── conversion.c
│               │   ├── float_ops.c
│               │   ├── memory.c
│               │   ├── spill_fill.c
│               │   └── unary.c
│               └── trampoline/       # VM entry/exit
│                   ├── vm_trampoline.c
│                   └── vm_trampoline.h
└── tests/                    # Integration tests
```

## Dependency Budget

| Dependency | Type | Decision |
|-----------|------|----------|
| `paste` | proc-macro (build) | ✅ Keep — zero runtime cost |
| `cc` | build-dep | ✅ Keep — C handler compilation |
| `num_enum` | runtime | ❌ Remove → hand-impl `TryFrom` |
| `thiserror` | runtime | ❌ Remove → hand-impl `Display` |
| `anyhow` | runtime | ❌ Remove |
| `log` | runtime | ❌ Remove → no-op macros |
| `env_logger` | runtime | ❌ Remove |
| `smallvec` | runtime | ❌ Remove → `Vec` |
| `serde_json` | runtime | ❌ Remove |
| `rand` | runtime | ❌ Remove |
| `fixedbitset` | runtime | ❌ Remove |
| `filetime` | runtime | ❌ Remove |
| `windows-sys` | runtime | ❌ Remove |

**Result: 0 runtime dependencies.** Build-only: `cc`, `paste`.

## What Gets Removed vs sf-core

### Entire subsystems removed
- SSA interpreter, inplace interpreter
- Compiler pipeline (frontend/middle/backends, XIR)
- WASI implementation
- GC heap (`gc_heap.rs`, `gc_type_check.rs`)
- Profiler (`profiler.rs` — 606 LOC)
- Trace (`trace.rs` — 292 LOC)
- `expr_eval.rs` (SSA helper)
- `jump_table.rs` (unused by fast interp)

### Code stripped from remaining files
- **type_defs.rs**: Remove `StructType`, `ArrayType`, `CompositeType::Struct/Array`, `DefType` subtyping, recursion groups, `StorageType::Packed` → keep only `FunctionType`
- **type_context.rs**: Remove GC type equivalence (~47% of file), struct/array field checking, recursion group handling → keep only function type equivalence
- **value_type.rs**: Remove GC heap types (structref, arrayref, i31ref, eqref, etc.) → keep i32/i64/f32/f64/funcref/externref
- **opcodes.rs**: Remove GC opcodes (struct.new, array.new, ref.cast, ref.test, i31 ops, etc.)
- **op_decoder.rs**: Remove GC opcode decoding paths
- **entities.rs**: Remove `LinkableData`/`LinkableInstance` trait hierarchy, `Rc<RefCell<>>` wrappers → flatten to owned specs
- **ref_ops.rs**: Remove GC handlers (struct/array/cast/test/i31 — ~600 LOC) → keep only ref.null, ref.is_null, ref.func (~130 LOC)
- **store.rs**: Remove `gc_heap` field, simplify to single-module Vec-based storage
- **runtime.rs**: Remove multi-module management, WASI → minimal external fn registry using `Vec<(String, String, ExternalFn)>`

### Patterns changed
- `Rc<RefCell<T>>` → owned `T` or `&mut T`
- `HashMap` → `Vec` + linear scan
- `thread_local!` → allocated stack, pointer in VM context
- `dyn ExternalFunction` trait → `fn(&[Value], &mut [Value]) -> Result<(), WasmError>`

## Public API

```rust
#![no_std]
extern crate alloc;

/// Parse a WASM binary into a Module.
pub fn parse(bytes: &[u8]) -> Result<Module, WasmError>;

/// Validate a parsed Module (only available with `validate` feature).
#[cfg(feature = "validate")]
pub fn validate(module: &Module) -> Result<(), WasmError>;

/// A WASM module instance ready for execution.
pub struct Instance { /* ... */ }

/// External function signature: reads args, writes results into buffer.
pub type ExternalFn = fn(&[Value], &mut [Value]) -> Result<(), WasmError>;

impl Instance {
    /// Instantiate a module with external function bindings.
    /// `imports`: list of (module_name, function_name, function_pointer).
    pub fn new(
        module: &Module,
        imports: &[(&str, &str, ExternalFn)],
    ) -> Result<Self, WasmError>;

    /// Invoke an exported function by name.
    pub fn invoke(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Vec<Value>, WasmError>;
}
```

## Key Adaptations

### 1. no_std Porting
- `#![no_std]` + `extern crate alloc`
- `std::vec::Vec` → `alloc::vec::Vec`
- `std::boxed::Box` → `alloc::boxed::Box`
- `std::string::String` → `alloc::string::String`
- `alloc::format!` for formatting
- `core::cell::{Cell, RefCell}` where interior mutability still needed
- `core::fmt` for Display
- `core::ffi::{c_char, CStr}` (stable since Rust 1.64)
- Remove `Backtrace` (not available in no_std)
- Remove `std::error::Error` trait impls

### 2. Error System
- Hand-implement `Display` for all error types (no `thiserror`)
- Remove `Backtrace` capture from `WasmError`
- Keep `Box<WasmErrorInner>` pattern (works with `alloc`)
- Utility errors (`PayloadError`, `LimitsError`, `LEB128Error`) hand-implement `Display`

### 3. Enums (no num_enum)
```rust
impl TryFrom<u8> for ValueType {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, ()> { match v { ... } }
}
```

### 4. Stack Management
- No `thread_local!` — stack is allocated (e.g. `Vec<u64>`) and pointer stored in VM/Instance
- Stack base/end pointers passed into fast interpreter context
- Caller controls stack size at `Instance::new()` time

### 5. Ownership Model (no Rc/RefCell)
- Single-module: Instance owns all tables, memories, globals directly
- `MemInst` = `Vec<u8>` (owned)
- `TableInst` = `Vec<RefHandle>` (owned)
- `GlobalInst` = `Value` (owned, mutable via `&mut`)
- `FunctionInst` = `FunctionSpec` + optional `ExternalFn` pointer
- No shared state across modules — simplifies lifetime management

### 6. External Function Calls
```rust
// Registration at instantiation:
Instance::new(module, &[
    ("wasi_snapshot_preview1", "fd_write", my_fd_write),
    ("wasi_snapshot_preview1", "proc_exit", my_proc_exit),
])

// Signature (zero-alloc, multi-value):
fn my_fd_write(args: &[Value], results: &mut [Value]) -> Result<(), WasmError> {
    let fd = args[0].as_i32();
    // ... do work ...
    results[0] = Value::I32(bytes_written);
    Ok(())
}
```

### 7. Logging → No-op Macros
```rust
macro_rules! log_debug { ($($t:tt)*) => {} }
macro_rules! log_info  { ($($t:tt)*) => {} }
macro_rules! log_warn  { ($($t:tt)*) => {} }
macro_rules! log_error { ($($t:tt)*) => {} }
```

## Workplan

### Phase 1: Scaffold & Foundation
- [ ] Create `crates/sf-nano/` directory and `Cargo.toml` (no_std, alloc)
- [ ] Add sf-nano to workspace `Cargo.toml`
- [ ] Create `src/lib.rs` with `#![no_std]` + `extern crate alloc` + no-op log macros
- [ ] Copy & adapt `constants.rs` (trivial, no deps)
- [ ] Copy & adapt `utils/leb128.rs` (replace thiserror with hand-impl Display)
- [ ] Copy & adapt `utils/limits.rs` (replace thiserror with hand-impl Display)
- [ ] Copy & adapt `utils/payload.rs` (replace thiserror with hand-impl Display)
- [ ] Copy & adapt `wasmerror.rs` → `error.rs` (remove thiserror, Backtrace, std::error::Error)

### Phase 2: Type System & Opcodes
- [ ] Copy & adapt `value_type.rs` (remove num_enum, strip GC heap types)
- [ ] Copy & adapt `opcodes.rs` (remove num_enum, strip GC opcodes)
- [ ] Copy & adapt `op_decoder.rs` (strip GC decode paths)
- [ ] Copy & adapt `module/type_defs.rs` (FunctionType only — remove StructType, ArrayType, DefType hierarchy)
- [ ] Copy & adapt `module/type_context.rs` (function-type equivalence only — strip GC type checking)

### Phase 3: Module Layer
- [ ] Copy & adapt `module/entities.rs` (flatten: remove LinkableData, Rc<RefCell<>>, GC types)
- [ ] Copy & adapt `module/parser.rs` (strip GC section parsing)
- [ ] Copy & adapt `module/builder.rs` (simplify for single-module)
- [ ] Copy & adapt `module/validator.rs` + sub-modules (behind `validate` feature flag, strip GC validation, remove jump table/stack height computation)

### Phase 4: VM Infrastructure
- [ ] Copy & adapt `vm/value.rs` (RefHandle: funcref/externref only, no GC refs)
- [ ] Copy & adapt `vm/entities.rs` (owned instances, no Rc<RefCell<>>, no multi-module ranges)
- [ ] Create simplified `vm/store.rs` (Vec-based, single module, no HashMap, no gc_heap)
- [ ] Copy & adapt `vm/interp/raw_value.rs`
- [ ] Copy & adapt `vm/interp/stack.rs` (pointer-based, no thread_local)

### Phase 5: Fast Interpreter Core
- [ ] Set up `build.rs` for C handler compilation (adapt from sf-core build/)
- [ ] Copy build-time codegen for fast_interp (build/fast_interp/)
- [ ] Copy & adapt `fast/instruction.rs`
- [ ] Copy & adapt `fast/encoding.rs` (generated) + encoding.toml
- [ ] Copy & adapt `fast/context.rs` (owned refs instead of Rc)
- [ ] Copy & adapt `fast/fast_code.rs` (Box<[Instruction]>, no Pin<Rc<>>)
- [ ] Copy & adapt `fast/frame_layout.rs` + frame_layout.h
- [ ] Copy `fast/handlers_c/` (C files — no Rust changes needed)
- [ ] Copy `fast/trampoline/` (C files)
- [ ] Copy & adapt `fast/handlers/` (strip GC handlers from ref_ops.rs, update call.rs for fn-pointer externals)
- [ ] Copy & adapt `fast/builder/` (strip GC opcodes from dispatch.rs)
- [ ] Copy & adapt `fast/precompile.rs` (simplify for single-module)
- [ ] Copy & adapt `fast/runtime.rs` (pointer-based stack, no thread_local)
- [ ] Copy & adapt `fast/mod.rs` (remove profiler/trace modules)

### Phase 6: Public API
- [ ] Create `vm/mod.rs` — the `Instance` struct with `new()` + `invoke()`
- [ ] Wire up: `parse()` → `Instance::new(module, imports)` → `instance.invoke(name, args)`
- [ ] Implement external function dispatch in call handler
- [ ] Create `vm/interp/mod.rs` (direct fast interpreter entry, no backend selection)

### Phase 7: Build & Test
- [ ] Verify `cargo build` compiles sf-nano
- [ ] Port basic unit tests from sf-core

#### 7a: WASM 2.0 Spectest
- [ ] Create `crates/sf-nano-spectest/` test harness crate (depends on sf-nano)
- [ ] Checkout spectest suite to a WASM 2.0 commit (current sf-spectest uses 3.0)
- [ ] Adapt sf-spectest harness to use sf-nano's `parse()` → `Instance::new()` → `invoke()` API
- [ ] Run spectest, fix failures until passing

#### 7b: WASI Test
- [ ] Create `crates/sf-nano-wasi/` crate — external WASI implementation
- [ ] Implement WASI preview1 functions as `ExternalFn` (`fn(&[Value], &mut [Value]) -> Result<(), WasmError>`)
- [ ] Wire WASI functions into sf-nano via `Instance::new(module, wasi_imports)`
- [ ] Run sf-wasi-test suite against sf-nano + sf-nano-wasi, fix failures

#### 7c: CoreMark
- [ ] Run CoreMark WASM binary via sf-nano + sf-nano-wasi
- [ ] Performance benchmark comparison vs sf-core fast interpreter

#### 7d: Binary Size
- [ ] Binary size measurement (`opt-level = "z"`, LTO, `panic = "abort"`)
- [ ] Profile and identify largest contributors to binary size

### Phase 8: Polish
- [ ] Documentation & README
- [ ] Clean up dead code paths
- [ ] Ensure `#[cfg(feature = "validate")]` compiles both with and without

## Binary Size Optimization

Target profile for release:
```toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit
panic = "abort"        # No unwinding
strip = true           # Strip symbols
```

Additional techniques:
- `#[inline(never)]` on cold error paths
- Avoid monomorphization bloat (prefer dynamic dispatch for error paths)
- C handlers compiled with `-Os` instead of `-O3` if size > speed needed

## Notes

1. **C Handlers are essential for performance** — the fast interpreter's speed comes from C-implemented arithmetic/bitwise/float handlers with tail-call dispatch. Preserved identically from sf-core.

2. **Instruction fusion is kept** — 500 fused handlers contribute ~40% of interpreter performance. The binary size cost is worth the speed gain.

3. **`paste` crate** is a proc-macro for handler codegen. Zero runtime footprint, no_std compatible.

4. **encoding.toml and handlers.toml** drive build-time code generation. Copied with the build infrastructure. handlers_fused.toml contains the 500 fusion patterns.

5. **Validator is optional** — behind `validate` feature. For production with trusted WASM, skip validation for smaller binary. The fast interpreter builds its own IR and doesn't depend on validator outputs (no jump table or stack height needed).

6. **Single-module constraint** — simplifies everything dramatically. External functions (including WASI) are provided as function pointers at instantiation time. A separate crate can implement WASI and pass function pointers in.

7. **No `Rc<RefCell<>>`** — single-module ownership means Instance owns everything. This eliminates runtime overhead of reference counting and borrow checking, and is fully compatible with no_std (no alloc::rc needed).
