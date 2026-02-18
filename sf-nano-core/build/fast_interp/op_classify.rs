// Op classification — single source of truth for fusible op categories.
// Shared by gen_fusion_match, gen_fusion_emit, gen_fusion_c.
//
// Categories are defined in handlers.toml via the `category` field.
// This module provides lookup-based classification using the category map
// built from HandlersFile::category_map().

use std::collections::HashMap;

use super::types::{FusedHandler, OpCategory, TosPattern, TosPatternString};

/// Map from expanded handler name to its OpCategory.
pub type CategoryMap = HashMap<String, OpCategory>;

/// Pure expression binops: pop 2, push 1, no side effects.
pub fn is_pure_binop(categories: &CategoryMap, op: &str) -> bool {
    categories.get(op) == Some(&OpCategory::PureBinop)
}

/// Trapping binops: pop 2, push 1, needs ctx for trap path.
pub fn is_trapping_binop(categories: &CategoryMap, op: &str) -> bool {
    categories.get(op) == Some(&OpCategory::TrappingBinop)
}

/// Pure expression unary ops: pop 1, push 1, no side effects.
/// Includes conversions (wrap, extend, reinterpret, float convert).
pub fn is_pure_unary(categories: &CategoryMap, op: &str) -> bool {
    categories.get(op) == Some(&OpCategory::PureUnary)
}

/// Load ops: pop 1 (addr), push 1 (value), needs ctx, has MemArg immediate.
pub fn is_load_op(categories: &CategoryMap, op: &str) -> bool {
    categories.get(op) == Some(&OpCategory::Load)
}

/// Store ops: pop 2 (addr, value), push 0, needs ctx, has MemArg immediate.
pub fn is_store_op(categories: &CategoryMap, op: &str) -> bool {
    categories.get(op) == Some(&OpCategory::Store)
}

/// Check if an op is an l0 local register op (hot local cached in register).
pub fn is_l0_local_op(op: &str) -> bool {
    matches!(op, "local_get_l0" | "local_set_l0" | "local_tee_l0")
}

/// Check if an op is any kind of local op (l0 or general).
pub fn is_any_local_op(op: &str) -> bool {
    matches!(op, "local_get" | "local_set" | "local_tee"
        | "local_get_l0" | "local_set_l0" | "local_tee_l0")
}

/// Check if an op is any known fusible op.
pub fn is_fusible_op(categories: &CategoryMap, op: &str) -> bool {
    matches!(op, "local_get" | "local_set" | "local_tee"
        | "local_get_l0" | "local_set_l0" | "local_tee_l0"
        | "i32_const" | "i64_const" | "br_if" | "if_")
        || categories.contains_key(op)
}

/// Check if an op is a comparison/test op (produces a boolean result).
/// Used by branch-aware pattern yielding: patterns ending with a comparison
/// should yield to branch fusion when followed by br_if or if_.
pub fn is_comparison_op(op: &str) -> bool {
    matches!(
        op,
        "i32_eq" | "i32_ne" | "i32_eqz"
        | "i32_lt_s" | "i32_lt_u" | "i32_gt_s" | "i32_gt_u"
        | "i32_le_s" | "i32_le_u" | "i32_ge_s" | "i32_ge_u"
        | "i64_eq" | "i64_ne" | "i64_eqz"
        | "i64_lt_s" | "i64_lt_u" | "i64_gt_s" | "i64_gt_u"
        | "i64_le_s" | "i64_le_u" | "i64_ge_s" | "i64_ge_u"
        | "f32_eq" | "f32_ne" | "f32_lt" | "f32_gt" | "f32_le" | "f32_ge"
        | "f64_eq" | "f64_ne" | "f64_lt" | "f64_gt" | "f64_le" | "f64_ge"
    )
}

/// Convert snake_case to UPPER_SNAKE_CASE
pub fn to_upper_snake(name: &str) -> String {
    name.to_uppercase()
}

/// Map a pattern op name to its Opcode constant name.
/// All fusible ops follow the convention: snake_case → UPPER_SNAKE_CASE.
/// Special cases: "if_" → "IF", l0 ops map to their base Wasm opcode.
pub fn pattern_op_to_opcode(categories: &CategoryMap, op: &str) -> String {
    assert!(is_fusible_op(categories, op), "Unknown pattern op for Opcode mapping: {}", op);
    match op {
        "if_" => "IF".to_string(),
        "local_get_l0" => "LOCAL_GET".to_string(),
        "local_set_l0" => "LOCAL_SET".to_string(),
        "local_tee_l0" => "LOCAL_TEE".to_string(),
        _ => op.to_uppercase(),
    }
}

/// Determine the Immediate variant for a pattern op.
pub fn immediate_variant(categories: &CategoryMap, op: &str) -> &'static str {
    match op {
        "local_get" | "local_set" | "local_tee"
        | "local_get_l0" | "local_set_l0" | "local_tee_l0" => "LocalIndex",
        "i32_const" => "I32",
        "i64_const" => "I64",
        "br_if" => "LabelIndex",
        "if_" => "Block",
        _ => match categories.get(op) {
            Some(OpCategory::Load) | Some(OpCategory::Store) => "MemArg",
            Some(OpCategory::PureBinop) | Some(OpCategory::TrappingBinop) | Some(OpCategory::PureUnary) => "None",
            _ => panic!("Unknown pattern op for Immediate mapping: {}", op),
        }
    }
}

/// Get the first WasmOpcode for the pattern (used in TempInst)
pub fn first_wasm_opcode(categories: &CategoryMap, fused: &FusedHandler) -> String {
    let opcode_name = pattern_op_to_opcode(categories, &fused.pattern[0]);
    format!("WasmOpcode::OP({})", opcode_name)
}

/// Check if pattern has a br_if branch target
pub fn has_branch(fused: &FusedHandler) -> bool {
    fused.pattern.iter().any(|op| op == "br_if")
}

/// Check if pattern contains if_ (conditional block entry)
pub fn has_if(fused: &FusedHandler) -> bool {
    fused.pattern.iter().any(|op| op == "if_")
}

/// Check if pattern has any branch-like target (br_if or if_)
pub fn has_branch_or_if(fused: &FusedHandler) -> bool {
    fused.pattern.iter().any(|op| op == "br_if" || op == "if_")
}

/// Get pop/push for tos_pattern
pub fn get_pop_push(fused: &FusedHandler) -> (u8, u8) {
    match &fused.tos_pattern {
        Some(TosPattern::PopPush { pop, push }) => (*pop, *push),
        Some(TosPattern::String(TosPatternString::None)) => (0, 0),
        _ => panic!("Fused handler {} has no tos_pattern", fused.op),
    }
}

pub fn is_tos_none(fused: &FusedHandler) -> bool {
    matches!(
        &fused.tos_pattern,
        Some(TosPattern::String(TosPatternString::None))
    )
}

/// Determine whether ctx is used by the handler (memory ops and trapping ops need it).
pub fn needs_ctx(categories: &CategoryMap, fused: &FusedHandler) -> bool {
    fused
        .pattern
        .iter()
        .any(|op| {
            matches!(
                categories.get(op.as_str()),
                Some(OpCategory::Load) | Some(OpCategory::Store) | Some(OpCategory::TrappingBinop)
            )
        })
}
