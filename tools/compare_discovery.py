#!/usr/bin/env python3
"""
Compare static vs dynamic fusion discovery TOML files.

Normalizes hot-local-aware opcode names (e.g. local_get_l0 -> local_get)
so that patterns from both discovery modes can be compared on equal footing.
"""

import re
import sys
from pathlib import Path


# ---------------------------------------------------------------------------
# Normalization
# ---------------------------------------------------------------------------

# Regex to strip _l0/_l1/_l2 suffixes from hot-local opcodes
HOT_LOCAL_RE = re.compile(r'^(local_get|local_set|local_tee)_l[012]$')


def normalize_op(op: str) -> str:
    m = HOT_LOCAL_RE.match(op)
    return m.group(1) if m else op


def normalize_pattern(pattern):
    return tuple(normalize_op(op) for op in pattern)


# ---------------------------------------------------------------------------
# Loading — lightweight regex parser (no tomllib dependency)
# ---------------------------------------------------------------------------

# Matches: pattern = ["op1", "op2", ...]
PATTERN_RE = re.compile(r'^pattern\s*=\s*\[([^\]]+)\]', re.MULTILINE)
OP_RE = re.compile(r'^op\s*=\s*"([^"]+)"', re.MULTILINE)


def load_toml(path):
    """Extract list of {op, pattern} dicts from a fused-handlers TOML file."""
    text = open(path, 'r').read()

    # Split on [[fused]] headers
    sections = re.split(r'^\[\[fused\]\]\s*$', text, flags=re.MULTILINE)
    entries = []
    for section in sections:
        pat_m = PATTERN_RE.search(section)
        op_m = OP_RE.search(section)
        if pat_m:
            ops = [s.strip().strip('"') for s in pat_m.group(1).split(',')]
            entry = {'pattern': ops}
            if op_m:
                entry['op'] = op_m.group(1)
            entries.append(entry)
    return entries


# ---------------------------------------------------------------------------
# Analysis
# ---------------------------------------------------------------------------

def analyze(static_path: str, dynamic_path: str):
    static_entries = load_toml(static_path)
    dynamic_entries = load_toml(dynamic_path)

    # --- 1. Total patterns ---
    print("=" * 72)
    print("FUSION DISCOVERY OVERLAP ANALYSIS")
    print("=" * 72)
    print(f"\nStatic file:  {static_path}")
    print(f"Dynamic file: {dynamic_path}")
    print(f"\n1) Total patterns:")
    print(f"   Static:  {len(static_entries)}")
    print(f"   Dynamic: {len(dynamic_entries)}")

    # Build normalized pattern sets (and keep best savings per pattern)
    # savings = pattern length - 1 (dispatches saved)
    static_norm: dict[tuple, int] = {}
    for e in static_entries:
        pat = normalize_pattern(e['pattern'])
        savings = len(e['pattern']) - 1
        if pat not in static_norm or savings > static_norm[pat]:
            static_norm[pat] = savings

    dynamic_norm: dict[tuple, int] = {}
    dynamic_raw_count = 0
    dynamic_dup_count = 0
    for e in dynamic_entries:
        pat = normalize_pattern(e['pattern'])
        savings = len(e['pattern']) - 1
        dynamic_raw_count += 1
        if pat in dynamic_norm:
            dynamic_dup_count += 1
            if savings > dynamic_norm[pat]:
                dynamic_norm[pat] = savings
        else:
            dynamic_norm[pat] = savings

    # Also count static duplicates after normalization (should be 0)
    static_raw_count = len(static_entries)
    static_dup_count = static_raw_count - len(static_norm)

    # --- 2. Unique patterns after normalization ---
    print(f"\n2) Unique normalized patterns:")
    print(f"   Static:  {len(static_norm)}  (duplicates removed: {static_dup_count})")
    print(f"   Dynamic: {len(dynamic_norm)}  (duplicates removed: {dynamic_dup_count})")

    # --- 3. Overlap ---
    static_set = set(static_norm.keys())
    dynamic_set = set(dynamic_norm.keys())
    overlap = static_set & dynamic_set

    print(f"\n3) Overlap (patterns in BOTH files): {len(overlap)}")

    # --- 4. Static-only ---
    static_only = static_set - dynamic_set
    print(f"\n4) Static-only patterns: {len(static_only)}")

    # --- 5. Dynamic-only ---
    dynamic_only = dynamic_set - static_set
    print(f"\n5) Dynamic-only patterns: {len(dynamic_only)}")

    # --- 6. Overlap rates ---
    static_overlap_pct = 100.0 * len(overlap) / len(static_norm) if static_norm else 0
    dynamic_overlap_pct = 100.0 * len(overlap) / len(dynamic_norm) if dynamic_norm else 0
    print(f"\n6) Overlap rates:")
    print(f"   As % of static unique:  {static_overlap_pct:.1f}%  ({len(overlap)}/{len(static_norm)})")
    print(f"   As % of dynamic unique: {dynamic_overlap_pct:.1f}%  ({len(overlap)}/{len(dynamic_norm)})")

    # --- 7. Top-20 dynamic-only by savings (pattern length) ---
    print(f"\n7) Top-20 highest-savings DYNAMIC-ONLY patterns (missed by static):")
    print(f"   {'Rank':<5} {'Savings':<9} {'Pattern'}")
    print(f"   {'----':<5} {'-------':<9} {'-------'}")
    dynamic_only_ranked = sorted(
        [(pat, dynamic_norm[pat]) for pat in dynamic_only],
        key=lambda x: (-x[1], x[0])
    )
    for i, (pat, sav) in enumerate(dynamic_only_ranked[:20], 1):
        pat_str = " -> ".join(pat)
        print(f"   {i:<5} {sav:<9} {pat_str}")

    if len(dynamic_only_ranked) > 20:
        print(f"   ... and {len(dynamic_only_ranked) - 20} more")

    # --- 8. Top-20 static-only by savings (pattern length) ---
    print(f"\n8) Top-20 highest-savings STATIC-ONLY patterns (missed by dynamic):")
    print(f"   {'Rank':<5} {'Savings':<9} {'Pattern'}")
    print(f"   {'----':<5} {'-------':<9} {'-------'}")
    static_only_ranked = sorted(
        [(pat, static_norm[pat]) for pat in static_only],
        key=lambda x: (-x[1], x[0])
    )
    for i, (pat, sav) in enumerate(static_only_ranked[:20], 1):
        pat_str = " -> ".join(pat)
        print(f"   {i:<5} {sav:<9} {pat_str}")

    if len(static_only_ranked) > 20:
        print(f"   ... and {len(static_only_ranked) - 20} more")

    # --- Summary ---
    print(f"\n{'=' * 72}")
    print("SUMMARY")
    print(f"{'=' * 72}")
    print(f"  Static unique:       {len(static_norm)}")
    print(f"  Dynamic unique:      {len(dynamic_norm)}")
    print(f"  Overlap:             {len(overlap)}")
    print(f"  Static-only:         {len(static_only)}")
    print(f"  Dynamic-only:        {len(dynamic_only)}")
    print(f"  Combined unique:     {len(static_set | dynamic_set)}")
    print(f"  Jaccard similarity:  {100.0 * len(overlap) / len(static_set | dynamic_set):.1f}%")

    # Pattern length distribution comparison
    print(f"\n  Pattern length distribution (normalized):")
    print(f"  {'Length':<8} {'Static':<10} {'Dynamic':<10} {'Overlap':<10} {'S-only':<10} {'D-only':<10}")
    all_lengths = set()
    for pat in static_set | dynamic_set:
        all_lengths.add(len(pat))
    for length in sorted(all_lengths):
        s_count = sum(1 for p in static_set if len(p) == length)
        d_count = sum(1 for p in dynamic_set if len(p) == length)
        o_count = sum(1 for p in overlap if len(p) == length)
        so_count = sum(1 for p in static_only if len(p) == length)
        do_count = sum(1 for p in dynamic_only if len(p) == length)
        print(f"  {length:<8} {s_count:<10} {d_count:<10} {o_count:<10} {so_count:<10} {do_count:<10}")


if __name__ == '__main__':
    base = Path(__file__).resolve().parent.parent
    static_path = str(base / 'handlers_fused_static_coremark.toml')
    dynamic_path = str(base / 'handlers_fused_dynamic_coremark.toml')

    # Allow overriding via command-line args
    if len(sys.argv) >= 3:
        static_path = sys.argv[1]
        dynamic_path = sys.argv[2]

    analyze(static_path, dynamic_path)
