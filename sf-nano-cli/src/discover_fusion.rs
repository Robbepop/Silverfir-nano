//! CLI command for automatic fusion candidate discovery.

use sf_nano_core::vm::interp::fast::fusion_discovery::{self, DiscoveryConfig};
use sf_nano_core::vm::interp::fast::pattern_trie::PatternTrie;
use sf_nano_core::vm::interp::fast::profiler;
use sf_nano_core::wasi::{set_wasi_ctx, wasi_imports, WasiContextBuilder};
use sf_nano_core::Instance;

use std::collections::HashSet;
use std::path::PathBuf;
use std::{fs, process};

/// Default output path for discovered fusion patterns.
const DEFAULT_FUSED_TOML: &str = "handlers_fused_discovered.toml";

pub struct DiscoverFusionArgs {
    pub path: PathBuf,
    pub prog_args: Vec<String>,
    pub max_window: usize,
    pub top: usize,
    pub min_savings: u64,
    pub output: Option<PathBuf>,
    pub show_trie: bool,
}

impl Default for DiscoverFusionArgs {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            prog_args: Vec::new(),
            max_window: 4,
            top: 32,
            min_savings: 1000,
            output: None,
            show_trie: false,
        }
    }
}

/// Parse discover-fusion args from command line.
/// Format: discover-fusion [options] <wasm-file> [-- args...]
pub fn parse_args(args: &[String]) -> DiscoverFusionArgs {
    let mut result = DiscoverFusionArgs::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--window" | "-w" => {
                i += 1;
                if i < args.len() {
                    result.max_window = args[i].parse().unwrap_or(4);
                }
            }
            "--top" | "-n" => {
                i += 1;
                if i < args.len() {
                    result.top = args[i].parse().unwrap_or(32);
                }
            }
            "--min-savings" => {
                i += 1;
                if i < args.len() {
                    result.min_savings = args[i].parse().unwrap_or(1000);
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    result.output = Some(PathBuf::from(&args[i]));
                }
            }
            "--show-trie" => {
                result.show_trie = true;
            }
            "--" => {
                result.prog_args = args[i + 1..].to_vec();
                break;
            }
            _ => {
                if result.path.as_os_str().is_empty() {
                    result.path = PathBuf::from(&args[i]);
                } else {
                    // Remaining args are program args
                    result.prog_args = args[i..].to_vec();
                    break;
                }
            }
        }
        i += 1;
    }

    if result.path.as_os_str().is_empty() {
        eprintln!("Usage: sf-nano-cli discover-fusion [options] <wasm-file> [-- args...]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -w, --window <N>       Window size for N-gram capture (default: 4)");
        eprintln!("  -n, --top <N>          Maximum fusion candidates (default: 32)");
        eprintln!("  --min-savings <N>      Minimum dispatch savings threshold (default: 1000)");
        eprintln!("  -o, --output <path>    Output path for handlers_fused.toml");
        eprintln!("  --show-trie            Print the pattern trie");
        process::exit(1);
    }

    result
}

pub fn run_from_args(args: &[String]) {
    let cmd = parse_args(args);
    run(cmd);
}

pub fn run(cmd: DiscoverFusionArgs) {
    // Enable profiling before loading module
    profiler::enable(cmd.max_window);
    eprintln!(
        "Profiling (fusion disabled): {} (window size: {})",
        cmd.path.display(),
        cmd.max_window
    );

    // Read WASM binary
    let data = fs::read(&cmd.path).unwrap_or_else(|err| {
        eprintln!("Error reading '{}': {}", cmd.path.display(), err);
        process::exit(1);
    });

    let module_name = cmd.path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("module");

    // Build WASI context
    let mut wasi_args = vec![module_name.to_string()];
    wasi_args.extend(cmd.prog_args.clone());

    let ctx = WasiContextBuilder::new()
        .args(&wasi_args)
        .preopen_dir("/", ".")
        .inherit_env()
        .build();
    set_wasi_ctx(ctx);

    // Create instance and run
    let imports = wasi_imports();
    let mut instance = Instance::new(&data, &imports).unwrap_or_else(|err| {
        eprintln!("Error instantiating module: {}", err);
        process::exit(1);
    });

    let result = instance.invoke("_start", &[]);
    let exit_code = match result {
        Ok(_) => None,
        Err(ref err) if err.to_string().contains("not found") => {
            match instance.invoke("main", &[]) {
                Ok(_) => None,
                Err(ref err) => err.exit_code(),
            }
        }
        Err(ref err) => err.exit_code(),
    };

    // Collect stats
    let stats = profiler::take_stats();

    if stats.total_instructions == 0 {
        eprintln!("No instructions were profiled.");
        if let Some(code) = exit_code {
            process::exit(code);
        }
        return;
    }

    // Build pattern trie
    eprintln!("Building pattern trie...");
    let trie = PatternTrie::from_stats(&stats);

    // Print trie stats
    let depth_stats = trie.depth_stats();
    eprintln!();
    eprintln!("Trie Statistics");
    eprintln!("{}", "-".repeat(50));
    eprintln!("Total instructions profiled: {}", stats.total_instructions);
    let mut depths: Vec<_> = depth_stats.iter().collect();
    depths.sort_by_key(|(d, _)| *d);
    for (depth, count) in &depths {
        eprintln!("  Unique {}-grams: {}", depth, count);
    }
    eprintln!();

    if cmd.show_trie {
        trie.print_tree(cmd.max_window, cmd.min_savings / (cmd.max_window as u64));
        eprintln!();
    }

    // Load handler op names to avoid name collisions
    let reserved_names = load_handler_names();
    eprintln!("Reserved handler names: {}", reserved_names.len());
    eprintln!();

    // Run discovery
    let config = DiscoveryConfig {
        max_candidates: cmd.top,
        min_savings: cmd.min_savings,
        reserved_names,
    };

    let candidates = fusion_discovery::discover(&trie, &config);

    if candidates.is_empty() {
        eprintln!("No new fusion candidates found above threshold.");
        if let Some(code) = exit_code {
            process::exit(code);
        }
        return;
    }

    // Print summary
    eprintln!("Discovered Candidates");
    eprintln!("{}", "-".repeat(70));
    let total_savings: u64 = candidates.iter().map(|c| c.savings).sum();
    let total_pct = if stats.total_instructions > 0 {
        (total_savings as f64 / stats.total_instructions as f64) * 100.0
    } else {
        0.0
    };

    for (i, c) in candidates.iter().enumerate() {
        let pct = if stats.total_instructions > 0 {
            (c.savings as f64 / stats.total_instructions as f64) * 100.0
        } else {
            0.0
        };
        eprintln!(
            "  {:>2}. {} [{}] count={}, savings={} ({:.2}%)",
            i + 1,
            c.name,
            c.pattern.join(" -> "),
            c.effective_count,
            c.savings,
            pct
        );
    }
    eprintln!();
    eprintln!(
        "Total estimated dispatch reduction: {} ({:.2}%)",
        total_savings, total_pct
    );
    eprintln!();

    // Generate TOML
    let toml = fusion_discovery::format_all_toml(&candidates);

    // Output
    let output_path = cmd.output.unwrap_or_else(|| PathBuf::from(DEFAULT_FUSED_TOML));
    fs::write(&output_path, &toml).unwrap_or_else(|err| {
        eprintln!("Error writing output file: {}", err);
        process::exit(1);
    });
    eprintln!("Written {} fused patterns to: {}", candidates.len(), output_path.display());
    eprintln!("Rebuild with: cargo build --release");

    if let Some(code) = exit_code {
        process::exit(code);
    }
}

/// Load [[handler]] op names from handlers.toml to avoid name collisions.
fn load_handler_names() -> HashSet<String> {
    // Try to find handlers.toml relative to the binary or CWD
    let candidates = [
        PathBuf::from("sf-nano-core/src/vm/interp/fast/handlers.toml"),
        PathBuf::from("../sf-nano-core/src/vm/interp/fast/handlers.toml"),
    ];

    for toml_path in &candidates {
        if let Ok(content) = fs::read_to_string(toml_path) {
            return parse_handler_names(&content);
        }
    }

    eprintln!("Warning: Could not find handlers.toml");
    HashSet::new()
}

fn parse_handler_names(content: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut in_handler = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[handler]]" {
            in_handler = true;
            continue;
        }
        if trimmed.starts_with("[[") {
            in_handler = false;
            continue;
        }
        if in_handler && trimmed.starts_with("op = ") {
            if let Some(rest) = trimmed.strip_prefix("op = ") {
                let rest = rest.trim().trim_matches('"');
                names.insert(rest.to_string());
            }
        }
    }
    names
}
