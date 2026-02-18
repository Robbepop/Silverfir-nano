//! Modular IR builder for the fast interpreter.
//!
//! Architecture:
//! - `context`: Function metadata and type resolution
//! - `stack`: Compile-time stack tracking
//! - `emitter`: Instruction emission
//! - `finalizer`: Compact, patch, and build final instructions
//! - `fusion`: Instruction fusion pattern matching
//! - `dispatch`: Opcode-to-handler dispatch

mod context;
mod dispatch;
mod emitter;
mod finalizer;
mod hot_local;
// Auto-generated from handlers.toml [[fused]] entries.
// See `build/fast_interp/gen_fusion.rs` for the generator.
#[cfg(feature = "fusion")]
include!(concat!(env!("OUT_DIR"), "/fast_interp/fast_fusion.rs"));
#[cfg(feature = "fusion")]
include!(concat!(env!("OUT_DIR"), "/fast_interp/fast_fusion_emit.rs"));
mod stack;
mod temp_inst;

pub use context::CompileContext;
pub use emitter::CodeEmitter;
pub use stack::{BlockKind, ControlFrame, StackTracker};
pub use temp_inst::TempInst;

use crate::{
    module::entities::FunctionSpec,
    vm::{
        entities::ModuleInst,
        interp::fast::instruction::Instruction,
        store::Store,
    },
    error::WasmError,
};

use alloc::rc::Rc;
use crate::module::type_defs::FunctionType;

/// Baseline IR builder: one fast instruction per Wasm opcode.
///
/// This struct provides the same interface as the old monolithic FastOpBuilder
/// but uses the modular builder internally.
#[derive(Default)]
pub struct FastOpBuilder;

impl FastOpBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build the fast IR for a function, threading control-flow and packing branch fixups.
    ///
    /// On success, attaches IR to `function` via `set_fast_ir` and returns the entry pointer.
    ///
    /// `types` is the module's type section, needed for resolving TypeIndex block types
    /// (multi-value blocks). If None, TypeIndex blocks will be treated as (0, 0) arity.
    ///
    /// `store` and `module` are used to look up function signatures for CALL stack tracking.
    pub fn build_for_function(
        &mut self,
        function: &FunctionSpec,
        types: Option<&[Rc<FunctionType>]>,
        store: &Store,
        module: &ModuleInst,
    ) -> Result<*mut Instruction, WasmError> {
        build_for_function(function, types, store, module)
    }
}

/// Build fast IR for a function.
///
/// This is the main entry point. It orchestrates:
/// 1. Decode Wasm opcodes
/// 2. Dispatch each opcode to update slots and emit instructions
/// 3. Finalize: compact, patch, fixup, build
///
/// All internal calls emit `call_internal`. During module precompilation,
/// same-module calls are optimized to `call_local` via in-place patching.
pub fn build_for_function(
    function: &FunctionSpec,
    types: Option<&[Rc<FunctionType>]>,
    store: &Store,
    module: &ModuleInst,
) -> Result<*mut Instruction, WasmError> {
    // Setup
    let code = function.code();
    let func_type = function.func_type();
    let params_count = func_type.params().len();
    let results_count = func_type.results().len();
    let locals_count = function.locals().len();

    let ctx = CompileContext::new(types, store, module, results_count);
    let frame_size = params_count + locals_count;
    let hot_local = hot_local::find_hot_local(code, frame_size);
    let mut stack = StackTracker::new(params_count, locals_count, results_count, hot_local);
    let mut emitter = CodeEmitter::new();

    // Always emit init_l0 to maintain the l0 = fp[0] invariant.
    //
    // Call/return handlers unconditionally spill/fill l0 via fp[0], so l0 must
    // always mirror fp[0] — even in functions that have no hot local.
    //
    // Special case: frame_size == 0 (no params, no locals).
    // fp[0] is the return_pc metadata slot, NOT a local. init_l0(K=0) loads
    // the return address into l0, which is meaningless but harmless: the
    // spill/fill cycle writes it back unchanged, and no local_get_l0/set_l0
    // instructions are emitted (there are no locals to access). Without this,
    // l0 would remain 0 (from run_trampoline) and the call_local spill would
    // overwrite return_pc with zero, corrupting the stack.
    let init_k = hot_local.unwrap_or(0);
    emitter.emit_init_l0(init_k);

    // Decode and dispatch (includes fusion at decode time)
    dispatch::decode_and_dispatch(code, &ctx, &mut stack, &mut emitter)?;

    // Finalize (br_table data is now stored inline in the instruction stream)
    let code_box = finalizer::finalize(emitter.take_temps(), &mut stack);

    // Store in function spec
    use crate::vm::interp::fast::fast_code::{FastCode, create_fast_code};
    let (fast_code, fast_cache) = create_fast_code(code_box, params_count, locals_count, results_count);
    let entry = fast_cache.entry();
    function.set_fast_code(fast_code, fast_cache);

    Ok(entry)
}
