// C compilation utilities
// Handles compilation of fast interpreter trampoline and handler C code

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

/// Track all C/H files in a directory for cargo rebuild.
fn track_c_files_in_dir(dir: &str) {
    println!("cargo:rerun-if-changed={}", dir);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some(OsStr::new("c"))
                || path.extension() == Some(OsStr::new("h"))
            {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

/// Compile the fast interpreter trampoline
pub fn compile_fast_trampoline(out_dir: &str) {
    let c_root = "src/vm/interp/fast/trampoline";
    let c_handlers_root = "src/vm/interp/fast/handlers_c";
    let fast_trampoline_src = format!("{}/vm_trampoline.c", c_root);

    if fs::metadata(&fast_trampoline_src).is_err() {
        println!("cargo:warning=Fast interpreter C trampoline not found, skipping compilation");
        return;
    }

    let mut build = cc::Build::new();
    build.compiler("clang");

    let fast_interp_out = PathBuf::from(out_dir).join("fast_interp");

    build
        .file(format!("{}/vm_trampoline.c", c_root))
        .include(c_root)
        .include(out_dir)
        .include(&fast_interp_out);

    let is_debug = env::var("PROFILE").unwrap_or_default() == "debug";
    let has_debug_info = env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok() || env::var("DEBUG").as_deref() == Ok("true") || env::var("DEBUG").as_deref() == Ok("2");

    if !is_debug {
        build.define("NDEBUG", None);
    }

    if env::var("CARGO_FEATURE_FUSION").is_ok() {
        build.define("FUSION_ENABLED", None);
    }

    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        build
            .compiler("clang-cl")
            .archiver("llvm-lib")
            .flag_if_supported("/O2")
            .flag_if_supported("/wd4100")
            .flag("/clang:-flto=thin")
            .flag("/clang:-fomit-frame-pointer")
            .flag("/clang:-foptimize-sibling-calls")
            .flag("/clang:-Wno-unused-parameter");

        if has_debug_info {
            build.flag("/Zi");
        }

        build.compile("libvm_trampoline.a");
    } else {
        build
            .flag("-O3")
            .flag("-ffast-math")
            .flag("-fno-finite-math-only")
            .flag("-foptimize-sibling-calls")
            .flag("-march=native")
            .flag("-Wno-unused-parameter");

        if has_debug_info {
            build.flag("-g").flag("-fno-omit-frame-pointer");
        } else {
            build.flag("-fomit-frame-pointer");
        }

        build.compile("vm_trampoline");
    }

    track_c_files_in_dir(c_root);
    track_c_files_in_dir(c_handlers_root);
}
