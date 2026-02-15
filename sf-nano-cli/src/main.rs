#[cfg(feature = "profile")]
mod discover_fusion;

use sf_nano_core::wasi::{set_wasi_ctx, wasi_imports, WasiContextBuilder};
use sf_nano_core::Instance;

use std::path::PathBuf;
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: sf-nano-cli <wasm-file> [args...]");
        #[cfg(feature = "profile")]
        eprintln!("       sf-nano-cli discover-fusion [options] <wasm-file>");
        process::exit(1);
    }

    // Check for subcommands
    #[cfg(feature = "profile")]
    if args[1] == "discover-fusion" {
        discover_fusion::run_from_args(&args[2..]);
        return;
    }

    let path = PathBuf::from(&args[1]);
    let prog_args: Vec<String> = args[2..].to_vec();

    // Read WASM binary
    let data = fs::read(&path).unwrap_or_else(|err| {
        eprintln!("Error reading '{}': {}", path.display(), err);
        process::exit(1);
    });

    let module_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("module");

    // Build WASI context
    let mut wasi_args = vec![module_name.to_string()];
    wasi_args.extend(prog_args);

    let ctx = WasiContextBuilder::new()
        .args(&wasi_args)
        .preopen_dir("/", ".")
        .inherit_env()
        .build();
    set_wasi_ctx(ctx);

    // Create instance with WASI imports
    let imports = wasi_imports();
    let mut instance = Instance::new(&data, &imports).unwrap_or_else(|err| {
        eprintln!("Error instantiating module: {}", err);
        process::exit(1);
    });

    // Invoke _start, fallback to main
    let result = instance.invoke("_start", &[]);
    let result = match result {
        Err(ref err) if err.to_string().contains("not found") => {
            instance.invoke("main", &[])
        }
        _ => result,
    };

    match result {
        Ok(_) => {}
        Err(err) => {
            if let Some(code) = err.exit_code() {
                process::exit(code);
            }
            eprintln!("Error: {}", err);
            process::exit(1);
        }
    }
}
