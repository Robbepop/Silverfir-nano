//! Interpreter context & hot state layout.

use core::ptr::NonNull;
use core::ffi::c_char;

use crate::vm::entities::ModuleInst;
use crate::vm::store::Store;
use crate::error::WasmError;

/// Opaque context passed across the C trampoline boundary.
///
/// SAFETY: Field order must match vm_trampoline.h CtxHot struct.
#[repr(C)]
pub struct Context {
    pub stack_end: *mut u64,
    pub call_depth: u64,
    pub mem0_base: *mut u8,
    pub mem0_size: u64,
    pub trap_message: *const c_char,
    pub term_inst: *mut u8,
    pub store: *mut Store,
    pub current_module: *const ModuleInst,
    pub error: Option<WasmError>,
}

impl Context {
    #[inline]
    pub fn new(
        store: *mut Store,
        current_module: *const ModuleInst,
        stack_end: *mut u64,
        mem0_base: *mut u8,
        mem0_size: u64,
    ) -> Self {
        Self {
            store,
            current_module,
            stack_end,
            call_depth: 0,
            mem0_base,
            mem0_size,
            trap_message: core::ptr::null(),
            term_inst: core::ptr::null_mut(),
            error: None,
        }
    }

    #[inline]
    pub fn store(&self) -> &Store {
        unsafe { &*self.store }
    }

    #[inline]
    pub fn store_mut(&self) -> &mut Store {
        unsafe { &mut *self.store }
    }

    #[inline]
    pub fn current_module(&self) -> Option<&ModuleInst> {
        if self.current_module.is_null() {
            None
        } else {
            Some(unsafe { &*self.current_module })
        }
    }
}
