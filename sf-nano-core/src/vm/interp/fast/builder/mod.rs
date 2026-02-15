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
    let mut stack = StackTracker::new(params_count, locals_count, results_count);
    let mut emitter = CodeEmitter::new();

    // SP-based model: no prologue needed
    // - Locals are zeroed in runtime.rs (for entry) or call_local (for calls)
    // - sp is set to fp + params + locals by caller

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
