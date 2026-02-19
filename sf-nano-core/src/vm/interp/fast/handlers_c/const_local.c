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

// =============================================================================
// L0 Local Register Cache Operations
// =============================================================================

// init_l0: function prologue — swap fp[0]↔fp[K], set *p_l0
FORCE_INLINE struct Instruction* impl_init_l0(IMPL_PARAMS_BASE) {
    (void)ctx;
    uint16_t K = init_l0_decode_hot_local_idx(pc);
    if (K != 0) {
        uint64_t tmp = fp[0];
        fp[0] = fp[K];
        fp[K] = tmp;
    }
    *p_l0 = fp[0];
    return pc_next(pc);
}

// local_get_l0: push l0 to TOS
FORCE_INLINE struct Instruction* impl_local_get_l0(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;

    *p_dst = *p_l0;
    return pc_next(pc);
}

// local_set_l0: pop TOS to l0
FORCE_INLINE struct Instruction* impl_local_set_l0(IMPL_PARAMS_POP1_PUSH0) {
    (void)ctx; (void)pfp;

    *p_l0 = *p_src;
    return pc_next(pc);
}

// local_tee_l0: copy TOS to l0, keep TOS unchanged
FORCE_INLINE struct Instruction* impl_local_tee_l0(IMPL_PARAMS_POP1_PUSH1) {
    (void)ctx; (void)pfp; (void)p_dst;

    *p_l0 = *p_src;
    return pc_next(pc);
}

// =============================================================================
// L1 Local Register Cache Operations
// =============================================================================

// init_l1: function prologue — swap fp[1]↔fp[K], set *p_l1
FORCE_INLINE struct Instruction* impl_init_l1(IMPL_PARAMS_BASE) {
    (void)ctx;
    uint16_t K = init_l1_decode_hot_local_idx(pc);
    if (K != 1) {
        uint64_t tmp = fp[1];
        fp[1] = fp[K];
        fp[K] = tmp;
    }
    *p_l1 = fp[1];
    return pc_next(pc);
}

// local_get_l1: push l1 to TOS
FORCE_INLINE struct Instruction* impl_local_get_l1(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;

    *p_dst = *p_l1;
    return pc_next(pc);
}

// local_set_l1: pop TOS to l1
FORCE_INLINE struct Instruction* impl_local_set_l1(IMPL_PARAMS_POP1_PUSH0) {
    (void)ctx; (void)pfp;

    *p_l1 = *p_src;
    return pc_next(pc);
}

// local_tee_l1: copy TOS to l1, keep TOS unchanged
FORCE_INLINE struct Instruction* impl_local_tee_l1(IMPL_PARAMS_POP1_PUSH1) {
    (void)ctx; (void)pfp; (void)p_dst;

    *p_l1 = *p_src;
    return pc_next(pc);
}

// =============================================================================
// L2 Local Register Cache Operations
// =============================================================================

// init_l2: function prologue — swap fp[2]↔fp[K], set *p_l2
FORCE_INLINE struct Instruction* impl_init_l2(IMPL_PARAMS_BASE) {
    (void)ctx;
    uint16_t K = init_l2_decode_hot_local_idx(pc);
    if (K != 2) {
        uint64_t tmp = fp[2];
        fp[2] = fp[K];
        fp[K] = tmp;
    }
    *p_l2 = fp[2];
    return pc_next(pc);
}

// local_get_l2: push l2 to TOS
FORCE_INLINE struct Instruction* impl_local_get_l2(IMPL_PARAMS_POP0_PUSH1) {
    (void)ctx; (void)pfp;

    *p_dst = *p_l2;
    return pc_next(pc);
}

// local_set_l2: pop TOS to l2
FORCE_INLINE struct Instruction* impl_local_set_l2(IMPL_PARAMS_POP1_PUSH0) {
    (void)ctx; (void)pfp;

    *p_l2 = *p_src;
    return pc_next(pc);
}

// local_tee_l2: copy TOS to l2, keep TOS unchanged
FORCE_INLINE struct Instruction* impl_local_tee_l2(IMPL_PARAMS_POP1_PUSH1) {
    (void)ctx; (void)pfp; (void)p_dst;

    *p_l2 = *p_src;
    return pc_next(pc);
}

#undef fp
