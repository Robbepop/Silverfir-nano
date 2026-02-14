// Fusion code generator: orchestrates production of fusion.rs, fusion_emit.rs,
// and fused C handlers from the [[fused]] entries in handlers.toml.
//
// Split into sub-modules:
//   op_classify      — shared op classification functions
//   gen_fusion_match  — Output 1: fast_fusion.rs (FusedOp enum, OpFuser, pattern matching)
//   gen_fusion_emit   — Output 2: fast_fusion_emit.rs (emit_fused, spill/fill, CodeEmitter methods)
//   gen_fusion_c      — Output 3: fast_fused_handlers.inc (StackSim, C handler bodies)

use super::types::HandlersFile;
use std::fs;
use std::path::PathBuf;

pub fn generate(handlers: &HandlersFile, out_dir: &PathBuf) {
    let fused = &handlers.fused;
    let categories = handlers.category_map();

    // Always write all three output files (empty stubs when no fused entries)
    // so that include!() and #include don't fail.

    // Output 1: fast_fusion.rs
    let fusion_rs = if fused.is_empty() {
        super::gen_fusion_match::generate_empty()
    } else {
        super::gen_fusion_match::generate(fused, &categories)
    };
    let fusion_path = out_dir.join("fast_fusion.rs");
    fs::write(&fusion_path, fusion_rs)
        .unwrap_or_else(|_| panic!("Failed to write {:?}", fusion_path));

    // Output 2: fast_fusion_emit.rs
    let fusion_emit_rs = if fused.is_empty() {
        super::gen_fusion_emit::generate_empty()
    } else {
        super::gen_fusion_emit::generate(fused, &categories)
    };
    let fusion_emit_path = out_dir.join("fast_fusion_emit.rs");
    fs::write(&fusion_emit_path, fusion_emit_rs)
        .unwrap_or_else(|_| panic!("Failed to write {:?}", fusion_emit_path));

    // Output 3: fast_fused_handlers.inc
    let fused_c = if fused.is_empty() {
        "// No fused handlers (handlers_fused.toml not found)\n".to_string()
    } else {
        super::gen_fusion_c::generate(fused, &categories)
    };
    let fused_c_path = out_dir.join("fast_fused_handlers.inc");
    fs::write(&fused_c_path, fused_c)
        .unwrap_or_else(|_| panic!("Failed to write {:?}", fused_c_path));
}
