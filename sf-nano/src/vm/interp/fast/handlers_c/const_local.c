// Fast interpreter C handler implementations - Local and constant operations
// Implementations use SEM_* macros from semantics.h (single source of truth).
//
// This file is #included in vm_trampoline.c after semantics.h.

#include <stdint.h>

// Dereference the double pointer for direct access
#define fp (*pfp)

// =============================================================================
// Local Operations
// =============================================================================

// local_get: tos_pattern = { pop = 0, push = 1 }
FORCE_INLINE struct Instruction* impl_local_get(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx;
    uint16_t idx = local_get_decode_idx(pc);
    *p_dst = SEM_LOCAL_GET(fp, idx);
    return pc_next(pc);
}

// local_set: tos_pattern = { pop = 1, push = 0 }
FORCE_INLINE struct Instruction* impl_local_set(IMPL_PARAMS_POP1_PUSH0) {
    (void)ctx;
    uint16_t idx = local_set_decode_idx(pc);
    SEM_LOCAL_SET(fp, idx, *p_src);
    return pc_next(pc);
}

// local_tee: tos_pattern = { pop = 1, push = 1 }
FORCE_INLINE struct Instruction* impl_local_tee(IMPL_PARAMS_POP1_PUSH1) {
    (void)ctx;
    uint16_t idx = local_tee_decode_idx(pc);
    SEM_LOCAL_SET(fp, idx, *p_src);
    *p_dst = *p_src;  // tee doesn't change the value
    return pc_next(pc);
}

// =============================================================================
// Constant Push Operations - tos_pattern = { pop = 0, push = 1 }
// =============================================================================

FORCE_INLINE struct Instruction* impl_i32_const(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;
    int64_t value = const_decode_value(pc);
    *p_dst = (uint64_t)(uint32_t)(int32_t)value;
    return pc_next(pc);
}

FORCE_INLINE struct Instruction* impl_i64_const(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;
    int64_t value = const_decode_value(pc);
    *p_dst = (uint64_t)value;
    return pc_next(pc);
}

FORCE_INLINE struct Instruction* impl_f32_const(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;
    int64_t value = const_decode_value(pc);
    *p_dst = (uint64_t)(uint32_t)value;
    return pc_next(pc);
}

FORCE_INLINE struct Instruction* impl_f64_const(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;
    int64_t value = const_decode_value(pc);
    *p_dst = (uint64_t)value;
    return pc_next(pc);
}

#undef fp
