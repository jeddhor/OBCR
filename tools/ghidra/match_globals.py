"""Transfers Wolf3D global-variable names to COUNT.EXE DGROUP offsets.

For every function pair in re/symbols/count_exe.csv, aligns the two
instruction sequences and pairs up direct DS memory operands ([0xNNNN]).
Votes decide each OBC offset's Wolf3D counterpart, which is then named from
WOLF3D.MAP's DGROUP publics (name+delta for offsets inside a variable).
Writes re/symbols/count_exe_data.csv (confidence LIKELY).

Usage: python -I tools/ghidra/match_globals.py
"""

import bisect
import csv
import difflib
import re
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WOLF_MAP = ROOT.parent / "wolf3d/WOLFSRC/WOLF3D.MAP"
WOLF_DGROUP = "3A39"
MEM = re.compile(r"\[(?:[A-Z]{2} \+ )?0x([0-9a-f]{3,4})\]")
NUM = re.compile(r"0x[0-9a-fA-F]+|\b\d+\b")


def load_raw(export_dir):
    funcs = {}
    for path in (export_dir / "asm").glob("*.asm"):
        lines = path.read_text(errors="replace").splitlines()
        if not lines:
            continue
        addr = lines[0].split()[2]
        raw = [l.split(None, 2)[2].strip() for l in lines[1:] if len(l.split(None, 2)) == 3]
        funcs[addr] = raw
    return funcs


def wolf_publics():
    pubs = []
    started = False
    for line in WOLF_MAP.read_text(errors="replace").splitlines():
        if "Publics by Value" in line:
            started = True
            continue
        m = re.match(r"\s*([0-9A-F]{4}):([0-9A-F]{4})\s+(?:idle\s+)?(\S+)\s*$", line)
        if started and m and m.group(1) == WOLF_DGROUP:
            pubs.append((int(m.group(2), 16), m.group(3).lstrip("_")))
    pubs.sort()
    return pubs


def main():
    obc = load_raw(ROOT / ".local/ghidra/export")
    wolf = load_raw(ROOT / ".local/ghidra/export-wolf")
    wolf_by_name = {}
    for path in (ROOT / ".local/ghidra/export-wolf/asm").glob("*.asm"):
        head = path.read_text(errors="replace").split("\n", 1)[0].split()
        wolf_by_name.setdefault(head[1], head[2])
    votes = defaultdict(Counter)
    for row in csv.DictReader((ROOT / "re/symbols/count_exe.csv").open()):
        if row["notes"]:
            continue
        a, b = obc.get(row["ghidra_addr"]), wolf.get(wolf_by_name.get(row["wolf3d_name"], ""))
        if not a or not b:
            continue
        na = [NUM.sub("#", x.lower()) for x in a]
        nb = [NUM.sub("#", x.lower()) for x in b]
        sm = difflib.SequenceMatcher(None, na, nb, autojunk=False)
        for tag, i1, i2, j1, j2 in sm.get_opcodes():
            if tag not in ("equal", "replace") or i2 - i1 != j2 - j1:
                continue
            for k in range(i2 - i1):
                ma, mb = MEM.findall(a[i1 + k]), MEM.findall(b[j1 + k])
                if len(ma) == len(mb):
                    for x, y in zip(ma, mb):
                        votes[int(x, 16)][int(y, 16)] += 1
    pubs = wolf_publics()
    offs = [p[0] for p in pubs]
    rows = []
    for obc_off, counter in sorted(votes.items()):
        (wolf_off, n), total = counter.most_common(1)[0], sum(counter.values())
        i = bisect.bisect_right(offs, wolf_off) - 1
        if i < 0 or n * 2 < total:
            continue
        base, name = pubs[i]
        delta = wolf_off - base
        if delta > 0x200:
            continue
        rows.append({
            "ds_off": f"0x{obc_off:04X}", "name": name if delta == 0 else f"{name}+0x{delta:X}",
            "wolf_ds_off": f"0x{wolf_off:04X}", "votes": f"{n}/{total}", "confidence": "LIKELY",
        })
    out = ROOT / "re/symbols/count_exe_data.csv"
    with out.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["ds_off", "name", "wolf_ds_off", "votes", "confidence"])
        w.writeheader()
        w.writerows(rows)
    print(f"{len(rows)} DGROUP offsets named; wrote {out}")


if __name__ == "__main__":
    main()
