//! Fast interpreter instruction sequence profiler.
//!
//! Captures N-instruction sequences to identify super-instruction candidates.
//! Enabled via `profile` feature which passes `FAST_PROFILE_ENABLED` to C handlers.
//!
//! The profiler works with handler name strings passed directly from C macros,
//! eliminating the need for function pointer lookup tables.

extern crate std;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Maximum supported window size for sequence capture.
pub const MAX_WINDOW_SIZE: usize = 8;

// Global configuration
static ENABLED: AtomicBool = AtomicBool::new(false);
static WINDOW_SIZE: AtomicUsize = AtomicUsize::new(2);
static TOTAL_INSTRUCTIONS: AtomicU64 = AtomicU64::new(0);

// Global sequence storage
static SEQUENCES: OnceLock<Mutex<HashMap<SequenceKey, u64>>> = OnceLock::new();

// Interned handler name strings
static INTERNED_NAMES: OnceLock<Mutex<HashMap<&'static [u8], &'static str>>> = OnceLock::new();

/// Sequence key: array of handler names representing an instruction sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceKey {
    names: [&'static str; MAX_WINDOW_SIZE],
    len: usize,
}

impl SequenceKey {
    /// Format the sequence as human-readable string.
    pub fn format(&self) -> std::string::String {
        self.names[..self.len].join(" -> ")
    }

    /// Get the sequence length.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the op names as a slice.
    pub fn ops(&self) -> &[&'static str] {
        &self.names[..self.len]
    }

    /// Check if this sequence is fuseable (no control flow in middle positions).
    pub fn is_fuseable(&self) -> bool {
        if self.len <= 1 {
            return true;
        }
        for &name in &self.names[..self.len - 1] {
            if is_control_flow(name) {
                return false;
            }
        }
        true
    }
}

/// Check if a handler name represents a control flow instruction.
fn is_control_flow(name: &str) -> bool {
    matches!(
        name,
        "br" | "br_if" | "br_table"
        | "block" | "loop" | "if_" | "else_" | "end"
        | "call" | "call_indirect" | "return"
        | "return_call" | "return_call_indirect"
        | "unreachable"
    )
}

// Thread-local sliding window
std::thread_local! {
    static WINDOW: RefCell<SlidingWindow> = RefCell::new(SlidingWindow::new());
}

struct SlidingWindow {
    buffer: [&'static str; MAX_WINDOW_SIZE],
    count: usize,
    capacity: usize,
}

impl SlidingWindow {
    fn new() -> Self {
        Self {
            buffer: [""; MAX_WINDOW_SIZE],
            count: 0,
            capacity: 2,
        }
    }

    fn push(&mut self, name: &'static str) -> Option<SequenceKey> {
        if self.capacity == 1 {
            self.buffer[0] = name;
            self.count = 1;
            return Some(self.to_key());
        }

        if self.count < self.capacity {
            self.buffer[self.count] = name;
            self.count += 1;
            if self.count == self.capacity {
                return Some(self.to_key());
            }
            None
        } else {
            for i in 0..self.capacity - 1 {
                self.buffer[i] = self.buffer[i + 1];
            }
            self.buffer[self.capacity - 1] = name;
            Some(self.to_key())
        }
    }

    fn to_key(&self) -> SequenceKey {
        SequenceKey {
            names: self.buffer,
            len: self.count,
        }
    }

    fn set_capacity(&mut self, cap: usize) {
        self.capacity = cap.min(MAX_WINDOW_SIZE);
        self.count = 0;
    }

    fn clear(&mut self) {
        self.count = 0;
    }
}

// ============================================================================
// FFI Entry Point (called from C handlers)
// ============================================================================

/// Intern a C string to get a `&'static str` with deduplication.
fn intern_name(c_name: *const core::ffi::c_char) -> &'static str {
    let c_bytes = unsafe {
        let mut len = 0;
        let mut p = c_name;
        while *p != 0 {
            len += 1;
            p = p.add(1);
        }
        core::slice::from_raw_parts(c_name as *const u8, len)
    };

    let interned = INTERNED_NAMES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = interned.lock().unwrap();

    if let Some(&existing) = map.get(c_bytes) {
        return existing;
    }

    // Leak a copy to get 'static lifetime
    let owned = std::string::String::from(core::str::from_utf8(c_bytes).unwrap_or("?"));
    let leaked: &'static str = std::boxed::Box::leak(owned.into_boxed_str());
    map.insert(unsafe { core::slice::from_raw_parts(leaked.as_ptr(), leaked.len()) }, leaked);
    leaked
}

/// FFI entry point called from C handlers when profiling is enabled.
#[no_mangle]
pub unsafe extern "C" fn fast_profile_record(name: *const core::ffi::c_char) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let interned = intern_name(name);
    record_impl(interned);
}

fn record_impl(name: &'static str) {
    TOTAL_INSTRUCTIONS.fetch_add(1, Ordering::Relaxed);

    WINDOW.with(|w| {
        if let Some(key) = w.borrow_mut().push(name) {
            if let Some(map) = SEQUENCES.get() {
                if let Ok(mut counts) = map.lock() {
                    *counts.entry(key).or_insert(0) += 1;
                }
            }
        }
    });
}

// ============================================================================
// Public API
// ============================================================================

/// Enable profiling with specified window size (1-8).
pub fn enable(window_size: usize) {
    let ws = window_size.clamp(1, MAX_WINDOW_SIZE);
    WINDOW_SIZE.store(ws, Ordering::Relaxed);

    SEQUENCES.get_or_init(|| Mutex::new(HashMap::new()));
    INTERNED_NAMES.get_or_init(|| Mutex::new(HashMap::new()));

    WINDOW.with(|w| {
        let mut window = w.borrow_mut();
        window.set_capacity(ws);
        window.clear();
    });

    ENABLED.store(true, Ordering::Release);
}

/// Disable profiling and return collected statistics.
pub fn take_stats() -> FastProfileStats {
    ENABLED.store(false, Ordering::Release);

    let total = TOTAL_INSTRUCTIONS.swap(0, Ordering::Relaxed);
    let window_size = WINDOW_SIZE.load(Ordering::Relaxed);

    let sequences = SEQUENCES
        .get()
        .and_then(|m| m.lock().ok())
        .map(|mut m| core::mem::take(&mut *m))
        .unwrap_or_default();

    WINDOW.with(|w| w.borrow_mut().clear());

    FastProfileStats {
        total_instructions: total,
        window_size,
        sequences,
    }
}

/// Check if profiling is currently enabled.
#[inline]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

// ============================================================================
// Statistics
// ============================================================================

/// Profiling statistics for instruction sequences.
pub struct FastProfileStats {
    pub total_instructions: u64,
    pub window_size: usize,
    /// Sequence counts: sequence -> execution count.
    pub sequences: HashMap<SequenceKey, u64>,
}

impl FastProfileStats {
    /// Get top N sequences by frequency.
    pub fn top_sequences(&self, n: usize) -> std::vec::Vec<(&SequenceKey, u64)> {
        let mut vec: std::vec::Vec<_> = self.sequences.iter().map(|(k, &v)| (k, v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(n);
        vec
    }

    /// Get top N fuseable sequences by frequency.
    pub fn top_fuseable_sequences(&self, n: usize) -> std::vec::Vec<(&SequenceKey, u64)> {
        let mut vec: std::vec::Vec<_> = self
            .sequences
            .iter()
            .filter(|(k, _)| k.is_fuseable())
            .map(|(k, &v)| (k, v))
            .collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(n);
        vec
    }

    /// Aggregate sequences — already normalized since we use base handler names.
    pub fn aggregate_normalized(&self) -> HashMap<SequenceKey, u64> {
        self.sequences.clone()
    }

    /// Get top N fuseable sequences by frequency.
    pub fn top_fuseable_reduction_potential(&self) -> std::vec::Vec<(&SequenceKey, u64, u64)> {
        let mut results: std::vec::Vec<_> = self
            .sequences
            .iter()
            .filter(|(seq, _)| seq.is_fuseable())
            .map(|(seq, &count)| {
                let saved = (seq.len.saturating_sub(1) as u64) * count;
                (seq, count, saved)
            })
            .collect();
        results.sort_by(|a, b| b.2.cmp(&a.2));
        results
    }
}
