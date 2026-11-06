"""Matches COUNT.EXE functions to named WOLF3D.EXE functions by instruction shape.

Reads the per-function disassembly exported by ExportAll.java for both programs
(.local/ghidra/export and .local/ghidra/export-wolf) and writes candidate names
to re/symbols/count_exe.csv with confidence LIKELY (AGENT_GUIDE §5.3).

Usage: python -I tools/ghidra/match_functions.py [--min-ratio 0.8]
"""

import argparse
import csv
import difflib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NUM = re.compile(r"0x[0-9a-fA-F]+|\b\d+\b")


def load(export_dir):
    """Returns {address: (name, linear, [normalized instructions])}."""
    funcs = {}
    for path in sorted((export_dir / "asm").glob("*.asm")):
        lines = path.read_text(errors="replace").splitlines()
        if not lines:
            continue
        header = lines[0].split()
        name, addr = header[1], header[2]
        linear = header[3].replace("img+", "")
        body = []
        for line in lines[1:]:
            parts = line.split(None, 2)
            if len(parts) == 3:
                body.append(NUM.sub("#", parts[2].strip().lower()))
        funcs[addr] = (name, linear, body)
    return funcs


def best_match(body, wolf, min_ratio):
    n = len(body)
    scored = []
    for addr, (name, _, wbody) in wolf.items():
        if name.startswith("FUN_") or not (0.7 * n <= len(wbody) <= 1.4 * n):
            continue
        sm = difflib.SequenceMatcher(None, body, wbody, autojunk=False)
        if sm.real_quick_ratio() < min_ratio or sm.quick_ratio() < min_ratio:
            continue
        scored.append((sm.ratio(), name, addr))
    scored.sort(reverse=True)
    if not scored or scored[0][0] < min_ratio:
        return None
    if len(scored) > 1 and scored[1][0] > scored[0][0] - 0.03 and scored[1][1] != scored[0][1]:
        return (scored[0][0], scored[0][1], scored[0][2], f"ambiguous with {scored[1][1]} {scored[1][0]:.2f}")
    return (scored[0][0], scored[0][1], scored[0][2], "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--min-ratio", type=float, default=0.8)
    ap.add_argument("--min-len", type=int, default=8)
    args = ap.parse_args()
    obc = load(ROOT / ".local/ghidra/export")
    wolf = load(ROOT / ".local/ghidra/export-wolf")
    rows = []
    for addr, (name, linear, body) in sorted(obc.items(), key=lambda kv: kv[1][1]):
        if len(body) < args.min_len:
            continue
        m = best_match(body, wolf, args.min_ratio)
        if m:
            ratio, wname, waddr, note = m
            rows.append({
                "img_linear": linear, "ghidra_addr": addr, "kind": "func", "name": wname,
                "wolf3d_name": wname, "confidence": "LIKELY", "source": f"wolf3d-match {ratio:.2f}",
                "notes": note,
            })
    out = ROOT / "re/symbols/count_exe.csv"
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["img_linear", "ghidra_addr", "kind", "name", "wolf3d_name",
                                          "confidence", "source", "notes"])
        w.writeheader()
        w.writerows(rows)
    print(f"{len(rows)} of {len(obc)} COUNT.EXE functions matched; wrote {out}")


if __name__ == "__main__":
    main()
