//! Test discovery and filtering

use std::{fs, path::Path};

pub fn find_wast_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut wast_files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "wast") {
                wast_files.push(path);
            } else if path.is_dir() {
                wast_files.extend(find_wast_files(&path));
            }
        }
    }

    wast_files.sort();
    wast_files
}

/// Determines if a test should be skipped based on feature support.
///
/// sf-nano targets WebAssembly 2.0 only (no GC, no SIMD, no exceptions, no tail calls).
/// The official test suite includes tests for WebAssembly 3.0 features that we skip.
pub fn should_skip_test(test_name: &str) -> bool {
    // Skip SIMD/vector tests
    if test_name.starts_with("simd_") || test_name.starts_with("relaxed_") {
        return true;
    }

    // Skip SIMD lane-based tests (e.g., i32x4, f64x2, i16x8, etc.)
    let simd_patterns = ["x2", "x4", "x8", "x16"];
    for pattern in &simd_patterns {
        if test_name.contains(pattern) {
            return true;
        }
    }

    // Skip GC proposal tests (not in WASM 2.0)
    if test_name.starts_with("array")
        || test_name.starts_with("struct")
        || test_name.starts_with("i31")
        || test_name.starts_with("extern_")
        || test_name.starts_with("any_")
        || test_name == "ref_cast"
        || test_name == "ref_test"
        || test_name == "ref_eq"
        || test_name == "ref_as_non_null"
        || test_name.starts_with("ref.") // ref.wast in proposals
        || test_name.starts_with("br_on_cast")
        || test_name == "br_on_null"
        || test_name == "br_on_non_null"
        || test_name == "call_ref"
    {
        return true;
    }

    // Skip type-rec and type-subtyping (GC / wast crate encoder bugs)
    if test_name == "type-rec" || test_name == "type-subtyping" {
        return true;
    }

    // Skip Exception Handling proposal
    if test_name.starts_with("tag")
        || test_name.starts_with("throw")
        || test_name == "rethrow"
        || test_name.starts_with("try_")
        || test_name.starts_with("instance")
    {
        return true;
    }

    // Skip Tail Call proposal
    if test_name.starts_with("return_call") {
        return true;
    }

    // Skip tests requiring full multi-instance linking (shared tables/memories)
    if test_name == "elem" || test_name == "linking" {
        return true;
    }

    // Skip proposal tests
    test_name.starts_with("proposals/")
        || test_name.starts_with("proposals\\")
        || test_name.contains("/proposals/")
        || test_name.contains("\\proposals\\")
}
