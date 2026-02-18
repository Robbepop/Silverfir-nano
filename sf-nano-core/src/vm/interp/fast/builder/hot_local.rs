//! Hot local analysis for the L0 register cache.
//!
//! Walks raw Wasm bytecode and counts local variable accesses weighted by
//! loop nesting depth. The local with the highest weight becomes the hot
//! local (cached in l0).

use alloc::vec::Vec;
use crate::utils::leb128;

/// Read an unsigned LEB128 u32 from `code` at position `i`, returning (value, new_position).
/// Silently returns 0 on malformed input (best-effort for analysis).
#[inline]
fn read_u32(code: &[u8], i: usize) -> (u32, usize) {
    match leb128::read_leb128_u32(&code[i..]) {
        Ok((val, len)) => (val, i + len),
        Err(_) => (0, i + 1),
    }
}

/// Read a signed LEB128 i32 from `code` at position `i`, returning (value, new_position).
#[inline]
fn read_i32(code: &[u8], i: usize) -> (i32, usize) {
    match leb128::read_leb128_i32(&code[i..]) {
        Ok((val, len)) => (val, i + len),
        Err(_) => (0, i + 1),
    }
}

/// Read a signed LEB128 i64 from `code` at position `i`, returning (value, new_position).
#[inline]
fn read_i64(code: &[u8], i: usize) -> (i64, usize) {
    match leb128::read_leb128_i64(&code[i..]) {
        Ok((val, len)) => (val, i + len),
        Err(_) => (0, i + 1),
    }
}

/// Find the hottest local variable in a function body.
///
/// Returns the index of the local with the highest loop-depth-weighted
/// access count, or None if the function has no locals/params (frame_size == 0).
///
/// `code` is the raw Wasm function body bytecode (starting after the locals section).
/// `frame_size` is params_count + locals_count.
pub fn find_hot_local(code: &[u8], frame_size: usize) -> Option<u32> {
    if frame_size == 0 {
        return None;
    }

    let mut weights: Vec<u64> = alloc::vec![0u64; frame_size];
    let mut loop_depth: u32 = 0;
    let mut i = 0;

    while i < code.len() {
        let opcode = code[i];
        i += 1;

        match opcode {
            // block, if: enter block (not loop)
            0x02 | 0x04 => {
                i = skip_block_type(code, i);
            }
            // loop: increment depth
            0x03 => {
                loop_depth = loop_depth.saturating_add(1);
                i = skip_block_type(code, i);
            }
            // end: decrement loop depth if we were in a loop
            // (simplified: we can't distinguish loop-end from block-end
            //  without a control stack, but the weighting is approximate anyway)
            0x0B => {
                loop_depth = loop_depth.saturating_sub(1);
            }
            // local.get, local.set, local.tee (0x20, 0x21, 0x22)
            0x20 | 0x21 | 0x22 => {
                let (idx, new_i) = read_u32(code, i);
                i = new_i;
                if (idx as usize) < frame_size {
                    let w = loop_weight(loop_depth);
                    weights[idx as usize] = weights[idx as usize].saturating_add(w);
                }
            }
            // Instructions with a single LEB128 immediate
            // br, br_if, call, global.get/set, table.get/set, ref.func, memory.size/grow
            0x0C | 0x0D | 0x10 | 0x23 | 0x24 | 0x25 | 0x26 | 0xD2
            | 0x3F | 0x40 => {
                let (_, new_i) = read_u32(code, i);
                i = new_i;
            }
            // call_indirect: type_idx + table_idx
            0x11 => {
                let (_, new_i) = read_u32(code, i);
                let (_, new_i2) = read_u32(code, new_i);
                i = new_i2;
            }
            // br_table
            0x0E => {
                let (count, new_i) = read_u32(code, i);
                i = new_i;
                for _ in 0..=count {
                    let (_, new_i2) = read_u32(code, i);
                    i = new_i2;
                }
            }
            // Memory load/store instructions (0x28-0x3E): alignment + offset
            0x28..=0x3E => {
                let (_, new_i) = read_u32(code, i); // align
                let (_, new_i2) = read_u32(code, new_i); // offset
                i = new_i2;
            }
            // i32.const
            0x41 => {
                let (_, new_i) = read_i32(code, i);
                i = new_i;
            }
            // i64.const
            0x42 => {
                let (_, new_i) = read_i64(code, i);
                i = new_i;
            }
            // f32.const
            0x43 => {
                i += 4;
            }
            // f64.const
            0x44 => {
                i += 8;
            }
            // select_t (0x1C): has a type count + types
            0x1C => {
                let (count, new_i) = read_u32(code, i);
                i = new_i;
                for _ in 0..count {
                    let (_, new_i2) = read_u32(code, i);
                    i = new_i2;
                }
            }
            // FC prefix (0xFC): bulk memory, saturating truncation
            0xFC => {
                let (sub_opcode, new_i) = read_u32(code, i);
                i = new_i;
                match sub_opcode {
                    // memory.init: data_idx + mem_idx
                    8 => {
                        let (_, new_i2) = read_u32(code, i);
                        i = new_i2;
                        i += 1; // mem_idx (always 0x00)
                    }
                    // data.drop: data_idx
                    9 => {
                        let (_, new_i2) = read_u32(code, i);
                        i = new_i2;
                    }
                    // memory.copy: src_mem + dst_mem
                    10 => {
                        i += 2; // two memory indices
                    }
                    // memory.fill: mem_idx
                    11 => {
                        i += 1;
                    }
                    // table.init: elem_idx + table_idx
                    12 => {
                        let (_, new_i2) = read_u32(code, i);
                        let (_, new_i3) = read_u32(code, new_i2);
                        i = new_i3;
                    }
                    // elem.drop: elem_idx
                    13 => {
                        let (_, new_i2) = read_u32(code, i);
                        i = new_i2;
                    }
                    // table.copy: dst_table + src_table
                    14 => {
                        let (_, new_i2) = read_u32(code, i);
                        let (_, new_i3) = read_u32(code, new_i2);
                        i = new_i3;
                    }
                    // table.grow, table.size, table.fill: table_idx
                    15 | 16 | 17 => {
                        let (_, new_i2) = read_u32(code, i);
                        i = new_i2;
                    }
                    // Saturating truncation (0-7): no immediates
                    _ => {}
                }
            }
            // FB prefix (GC ops) - skip for now
            0xFB => {
                let (_, new_i) = read_u32(code, i);
                i = new_i;
                // Most FB ops have additional immediates but we don't need
                // precise parsing for hot local analysis - worst case we
                // misparse a few bytes but still get correct local weights.
            }
            // Everything else: no immediates (single-byte ops)
            _ => {}
        }
    }

    // Find the index with maximum weight
    let (max_idx, max_weight) = weights
        .iter()
        .enumerate()
        .max_by_key(|(_, w)| *w)
        .unwrap();

    if *max_weight == 0 {
        return None;
    }

    Some(max_idx as u32)
}

/// Compute loop weight: 10^min(depth, 6)
#[inline]
fn loop_weight(depth: u32) -> u64 {
    const WEIGHTS: [u64; 7] = [1, 10, 100, 1_000, 10_000, 100_000, 1_000_000];
    WEIGHTS[depth.min(6) as usize]
}

/// Skip a block type (single byte 0x40, or valtype, or s33 type index)
#[inline]
fn skip_block_type(code: &[u8], i: usize) -> usize {
    if i >= code.len() {
        return i;
    }
    let b = code[i];
    if b == 0x40 || (b >= 0x6C && b <= 0x7F) {
        // Empty block type or single value type
        i + 1
    } else {
        // s33 type index (LEB128)
        let (_, new_i) = read_i32(code, i);
        new_i
    }
}
