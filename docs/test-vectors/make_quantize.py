#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Writes quantize.json: test vectors for the mod palette rule.

docs/MOD_ASSETS_AND_ARENAS.md section 2.4. OBCR (obc-render::quantize) and
OBCEdit (obcedit_core.artimport) both test against the file this writes.

The palette is synthetic, made up here, so no game palette is ever in the
repository. It has deliberate duplicates, so the lowest-index tie rule is
exercised, and an entry 255 that must never be chosen.

    python docs/test-vectors/make_quantize.py
"""

import json
from pathlib import Path


def palette():
    pal = [((i * 37) % 256, (i * 91 + 13) % 256, (i * 53 + 101) % 256) for i in range(256)]
    pal[200] = pal[17]          # a duplicate: 17 must win
    pal[201] = pal[17]
    pal[254] = (0, 0, 0)
    pal[3] = (0, 0, 0)          # black twice: 3 must win over 254
    pal[255] = (255, 255, 255)  # never chosen, even for white
    return pal


def nearest(pal, r, g, b):
    best, best_d = 0, None
    for i in range(255):
        pr, pg, pb = pal[i]
        d = (pr - r) ** 2 + (pg - g) ** 2 + (pb - b) ** 2
        if best_d is None or d < best_d:
            best, best_d = i, d
    return best


def cases(pal):
    rgba = [
        (0, 0, 0, 255), (255, 255, 255, 255), (255, 0, 0, 255), (0, 255, 0, 255),
        (0, 0, 255, 255), (128, 128, 128, 255), (*pal[17], 255), (*pal[100], 255),
        (*pal[254], 255), (1, 2, 3, 255), (10, 200, 30, 127), (10, 200, 30, 128),
        (99, 99, 99, 0), (250, 5, 128, 64), (17, 34, 51, 255),
    ]
    # A deterministic spread over the colour cube.
    x = 12345
    for _ in range(85):
        x = (x * 1103515245 + 12345) & 0x7FFFFFFF
        rgba.append(((x >> 4) & 255, (x >> 12) & 255, (x >> 20) & 255, 255 if x & 1 else (x >> 8) & 255))
    out = []
    for r, g, b, a in rgba:
        idx = nearest(pal, r, g, b)
        out.append({"rgba": [r, g, b, a], "wall": idx, "sprite": 255 if a < 128 else idx})
    return out


def main():
    pal = palette()
    doc = {
        "rule": "alpha < 128 -> 255 (sprites only; walls ignore alpha); else the entry 0-254 "
                "with the least squared RGB distance, lowest index on a tie",
        "palette": [list(c) for c in pal],
        "cases": cases(pal),
    }
    path = Path(__file__).with_name("quantize.json")
    path.write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8", newline="\n")
    print("wrote", path)


if __name__ == "__main__":
    main()
