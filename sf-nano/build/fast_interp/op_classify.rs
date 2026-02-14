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

/// Check if an op is any known fusible op.
pub fn is_fusible_op(categories: &CategoryMap, op: &str) -> bool {
    matches!(op, "local_get" | "local_set" | "local_tee" | "i32_const" | "i64_const" | "br_if")
        || categories.contains_key(op)
}

/// Convert snake_case to UPPER_SNAKE_CASE
pub fn to_upper_snake(name: &str) -> String {
    name.to_uppercase()
}

/// Map a pattern op name to its Opcode constant name.
/// All fusible ops follow the convention: snake_case → UPPER_SNAKE_CASE.
pub fn pattern_op_to_opcode(categories: &CategoryMap, op: &str) -> String {
    assert!(is_fusible_op(categories, op), "Unknown pattern op for Opcode mapping: {}", op);
    op.to_uppercase()
}

/// Determine the Immediate variant for a pattern op.
pub fn immediate_variant(categories: &CategoryMap, op: &str) -> &'static str {
    match op {
        "local_get" | "local_set" | "local_tee" => "LocalIndex",
        "i32_const" => "I32",
        "i64_const" => "I64",
        "br_if" => "LabelIndex",
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

/// Check if pattern has a branch target (contains br_if)
pub fn has_branch(fused: &FusedHandler) -> bool {
    fused.pattern.iter().any(|op| op == "br_if")
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
