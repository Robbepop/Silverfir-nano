//! Finalizer: compact, patch, and build final instructions.
//!
//! This is the ONLY place where encoding happens.
//! TempInst stores logical PatternData values; encoding to imm0/imm1/imm2 happens here.

use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;

use super::stack::StackTracker;
use super::temp_inst::TempInst;
use crate::opcodes::{Opcode, WasmOpcode};
use crate::vm::interp::fast::encoding::{finalize_pattern_data, PatternData};
use crate::vm::interp::fast::handlers::full_set::*;
use crate::vm::interp::fast::instruction::Instruction;

/// Marker base for operand stack slots during compilation.
/// Final slot = operand_base + (value - OPERAND_BASE)
const OPERAND_BASE: usize = 16384;

/// Finalize temp instructions into final code.
///
/// Returns compiled instructions. br_table data is stored inline
/// in the instruction stream as data pseudo-instructions.
pub fn finalize(
    mut temps: Vec<TempInst>,
    stack: &mut StackTracker,
) -> Box<[Instruction]> {
    // Append terminal instructions
    append_terminals(&mut temps);

    // Route RETURN and UNREACHABLE to terminal
    route_terminals(&mut temps);

    // Default alt to terminal (for trap paths)
    default_alt_to_term(&mut temps);

    // Expand br_tables by inserting inline data pseudo-instructions
    let temps = expand_br_tables(temps);

    // Compute which instructions to keep (remove structural no-ops)
    let keep = compute_keep_mask(&temps);

    // Build old->new index mapping
    let index_map = build_index_map(&keep);

    // Compact temps and patch indices (including br_table inline data)
    let compacted = compact_and_patch(temps, &keep, &index_map);

    // Build final Instruction array (encoding happens here!)
    let operand_base = stack.operand_base();
    build_instructions(compacted, operand_base)
}

/// Append terminal (op_term) instruction and arena sentinel.
fn append_terminals(temps: &mut Vec<TempInst>) {
    let term_idx = temps.len();
    temps.push(TempInst::new(
        op_term,
        PatternData::Raw { imm0: 0, imm1: 0, imm2: 0 },
        WasmOpcode::OP(Opcode::END),
    ));

    // Link last real instruction to terminal
    if term_idx > 0 {
        temps[term_idx - 1].fallthrough_idx = Some(term_idx);
    }

    // Arena sentinel: ensures pc_next of any instruction (including the terminal
    // above) is a valid read. Required for next-handler preloading — every
    // dispatch path reads pc_next(np)->handler to prepare the nh parameter.
    temps.push(TempInst::new(
        op_term,
        PatternData::Raw { imm0: 0, imm1: 0, imm2: 0 },
        WasmOpcode::OP(Opcode::END),
    ));
}

/// Route RETURN and UNREACHABLE to the single terminal instruction.
fn route_terminals(temps: &mut Vec<TempInst>) {
    let term_idx = temps.len() - 1;

    for t in temps.iter_mut() {
        match t.wasm_op {
            WasmOpcode::OP(Opcode::RETURN) | WasmOpcode::OP(Opcode::UNREACHABLE) => {
                t.fallthrough_idx = None;
                t.alt_idx = Some(term_idx);
            }
            _ => {}
        }
    }
}

/// Default alt to terminal for instructions without alt.
fn default_alt_to_term(temps: &mut Vec<TempInst>) {
    let term_idx = temps.len() - 1;
    for t in temps.iter_mut() {
        if t.alt_idx.is_none() {
            t.alt_idx = Some(term_idx);
        }
    }
}

/// Expand br_tables by inserting inline data pseudo-instructions.
fn expand_br_tables(temps: Vec<TempInst>) -> Vec<TempInst> {
    // First pass: calculate expansion
    let mut expansion_at: Vec<usize> = vec![0; temps.len()];
    let mut total_expansion = 0;

    for (i, t) in temps.iter().enumerate() {
        expansion_at[i] = total_expansion;
        if matches!(t.wasm_op, WasmOpcode::OP(Opcode::BR_TABLE)) {
            if let Some(ref entries) = t.br_table_entries {
                let data_slot_count = (entries.len() + 1) / 2;
                total_expansion += data_slot_count;
            }
        }
    }

    if total_expansion == 0 {
        return temps;
    }

    // Build old->new index mapping
    let old_to_new: Vec<usize> = expansion_at
        .iter()
        .enumerate()
        .map(|(i, &exp)| i + exp)
        .collect();

    // Update all index references
    let mut temps = temps;
    for t in temps.iter_mut() {
        if let Some(ref mut alt) = t.alt_idx {
            if *alt < old_to_new.len() {
                *alt = old_to_new[*alt];
            }
        }
        if let Some(ref mut ft) = t.fallthrough_idx {
            if *ft < old_to_new.len() {
                *ft = old_to_new[*ft];
            }
        }
        if let Some(ref mut entries) = t.br_table_entries {
            for e in entries.iter_mut() {
                if let Some(ref mut tgt) = e.target_idx {
                    if *tgt < old_to_new.len() {
                        *tgt = old_to_new[*tgt];
                    }
                }
            }
        }
    }

    // Build expanded Vec with data slots
    let mut result = Vec::with_capacity(temps.len() + total_expansion);

    for t in temps {
        let is_br_table = matches!(t.wasm_op, WasmOpcode::OP(Opcode::BR_TABLE));
        let data_slot_count = if is_br_table {
            t.br_table_entries.as_ref().map(|e| (e.len() + 1) / 2).unwrap_or(0)
        } else {
            0
        };

        result.push(t);

        // Insert data pseudo-instructions after br_table
        for _ in 0..data_slot_count {
            result.push(TempInst::new(
                op_data,
                PatternData::Raw { imm0: 0, imm1: 0, imm2: 0 },
                WasmOpcode::OP(Opcode::NOP),
            ));
        }
    }

    result
}

/// Compute which instructions to keep (remove structural no-ops).
/// SP-based model: DROP is NOT a no-op, it must run to decrement sp.
fn compute_keep_mask(temps: &[TempInst]) -> Vec<bool> {
    temps
        .iter()
        .map(|t| {
            let h = t.handler as usize;
            !(h == op_nop as usize
                || h == op_block as usize
                || h == op_loop as usize
                || h == op_end as usize)
            // Note: op_drop is kept in SP-based model (needs to decrement sp)
        })
        .collect()
}

/// Build old->new index mapping.
fn build_index_map(keep: &[bool]) -> Vec<Option<usize>> {
    let mut map = vec![None; keep.len()];
    let mut new_idx = 0;
    for (old_idx, &k) in keep.iter().enumerate() {
        if k {
            map[old_idx] = Some(new_idx);
            new_idx += 1;
        }
    }
    map
}

/// Compact temps and patch all indices.
fn compact_and_patch(
    temps: Vec<TempInst>,
    keep: &[bool],
    index_map: &[Option<usize>],
) -> Vec<TempInst> {
    let mut compacted = Vec::with_capacity(temps.len());
    let mut temps_iter = temps.into_iter().enumerate().peekable();

    while let Some((old_idx, t)) = temps_iter.next() {
        if !keep[old_idx] {
            continue;
        }

        let mut t = t;

        // Patch alt_idx
        if let Some(mut alt) = t.alt_idx {
            while alt < index_map.len() && index_map[alt].is_none() {
                alt += 1;
            }
            t.alt_idx = index_map.get(alt).copied().flatten();
        }

        // Handle br_table: fill inline data slots
        if matches!(t.wasm_op, WasmOpcode::OP(Opcode::BR_TABLE)) {
            if let Some(entries) = t.br_table_entries.take() {
                let br_table_new_idx = compacted.len();
                let entry_count = entries.len();
                let data_slot_count = (entry_count + 1) / 2;

                // Update BrTable pattern data with counts (preserve height and operand_base_offset)
                if let PatternData::BrTable { height, operand_base_offset, .. } = t.data {
                    t.data = PatternData::BrTable {
                        entry_count: entry_count as u64,
                        data_slot_count: data_slot_count as u64,
                        height,
                        operand_base_offset,
                    };
                }

                compacted.push(t);

                // Build packed data for each slot
                let mut data_slots: Vec<(u64, u64, u64)> = vec![(0, 0, 0); data_slot_count];

                for (entry_idx, entry) in entries.iter().enumerate() {
                    if let Some(mut tgt_old) = entry.target_idx {
                        while tgt_old < index_map.len() && index_map[tgt_old].is_none() {
                            tgt_old += 1;
                        }
                        if let Some(tgt_new) = index_map.get(tgt_old).copied().flatten() {
                            let rel = (tgt_new as i32) - (br_table_new_idx as i32);
                            let stack_drop = entry.stack_offset as u32;
                            let arity = entry.arity as u32;

                            let slot_idx = entry_idx / 2;
                            let entry_in_slot = entry_idx % 2;

                            if entry_in_slot == 0 {
                                data_slots[slot_idx].0 = rel as i32 as u64;
                                data_slots[slot_idx].1 = ((stack_drop << 16) | arity) as u64;
                            } else {
                                let packed = ((rel as u64) << 32)
                                    | ((stack_drop as u64) << 16)
                                    | (arity as u64);
                                data_slots[slot_idx].2 = packed;
                            }
                        }
                    }
                }

                // Fill data pseudo-instructions
                for (imm0, imm1, imm2) in data_slots {
                    if let Some((data_old_idx, mut data_t)) = temps_iter.next() {
                        debug_assert!(keep[data_old_idx]);
                        debug_assert_eq!(data_t.handler as usize, op_data as usize);
                        data_t.data = PatternData::Raw { imm0, imm1, imm2 };
                        compacted.push(data_t);
                    }
                }

                continue;
            }
        }

        compacted.push(t);
    }

    compacted
}

/// Convert an operand slot placeholder to an absolute frame offset.
/// In SP-based model, only OPERAND_BASE slots need fixup.
#[inline]
fn fixup_slot(slot: u16, operand_base: usize) -> u16 {
    let slot = slot as usize;
    if slot >= OPERAND_BASE {
        (operand_base + (slot - OPERAND_BASE)) as u16
    } else {
        slot as u16
    }
}

/// Build final Instruction array.
///
/// This is where ENCODING happens - TempInst's PatternData is converted to imm0/imm1/imm2.
fn build_instructions(
    temps: Vec<TempInst>,
    operand_base: usize,
) -> Box<[Instruction]> {
    if temps.is_empty() {
        return Box::new([]);
    }

    // Create the slot fixup closure
    let fix_slot = |slot: u16| -> u16 {
        fixup_slot(slot, operand_base)
    };

    // First pass: encode all instructions (target_ptr = 0 for now)
    let instructions: Vec<Instruction> = temps
        .iter()
        .map(|t| {
            let (imm0, imm1, imm2) = finalize_pattern_data(&t.data, 0, &fix_slot);
            Instruction::new(t.handler, imm0, imm1, imm2)
        })
        .collect();

    // Convert to Box (stable heap allocation)
    let mut code_box: Box<[Instruction]> = instructions.into_boxed_slice();

    // Second pass: patch branch target pointers
    let base = code_box.as_mut_ptr();
    for (i, t) in temps.iter().enumerate() {
        if let Some(alt_idx) = t.alt_idx {
            let needs_target = t.has_target;

            if needs_target {
                unsafe {
                    let target_ptr = base.add(alt_idx) as u64;
                    // Re-encode with actual target pointer
                    let (imm0, imm1, imm2) = finalize_pattern_data(&t.data, target_ptr, &fix_slot);
                    (*base.add(i)).imm0 = imm0;
                    (*base.add(i)).imm1 = imm1;
                    (*base.add(i)).imm2 = imm2;
                }
            }
        }
    }

    code_box
}
