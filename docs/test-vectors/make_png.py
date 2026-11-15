#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Writes png/*.png and png.json: how a mod image's pixels are read.

docs/MOD_ASSETS_AND_ARENAS.md section 2.4. OBCR (obc_formats::modassets::
decode_png) and OBCEdit (obcedit_core.artimport) both decode every file here
and compare with the expected RGBA, computed below from the PNG rules alone:

- samples of 1, 2 or 4 bits are scaled to 0-255 exactly (v * 255 / (2^d - 1));
- 16-bit samples keep their high byte;
- tRNS makes palette entries, or one exact grey or RGB value, transparent
  (alpha 0) or partly so (palette alpha);
- images without alpha are opaque.

The images are generated patterns; nothing from the game.

    python docs/test-vectors/make_png.py
"""

import hashlib
import json
import struct
import zlib
from pathlib import Path

SIZE = 64


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def pack_row(samples, depth):
    if depth >= 8:
        width = depth // 8
        return b"".join(s.to_bytes(width, "big") for s in samples)
    out, acc, bits = bytearray(), 0, 0
    for s in samples:
        acc = (acc << depth) | s
        bits += depth
        if bits == 8:
            out.append(acc)
            acc, bits = 0, 0
    if bits:
        out.append(acc << (8 - bits))
    return bytes(out)


def png(mode, depth, rows, extra=b""):
    """rows: per row, a flat list of samples (all channels interleaved)."""
    raw = b"".join(b"\0" + pack_row(r, depth) for r in rows)
    ihdr = struct.pack(">IIBBBBB", SIZE, SIZE, depth, mode, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + extra
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def scale(v, depth):
    return v * 255 // ((1 << depth) - 1) if depth < 8 else (v >> 8 if depth == 16 else v)


def cases():
    out = {}
    # 1, 2, 4 and 8-bit grey: a ramp.
    for d in (1, 2, 4, 8, 16):
        top = (1 << d) - 1
        rows = [[(x * 7 + y * 3) % (top + 1) for x in range(SIZE)] for y in range(SIZE)]
        rgba = [[(g := scale(v, d)), g, g, 255] for r in rows for v in r]
        out[f"grey{d}"] = (png(0, d, rows), rgba)
    # Grey 8-bit with tRNS: value 5 transparent.
    rows = [[(x + y) % 16 for x in range(SIZE)] for y in range(SIZE)]
    rgba = [[v, v, v, 0 if v == 5 else 255] for r in rows for v in r]
    out["grey8_trns"] = (png(0, 8, rows, chunk(b"tRNS", struct.pack(">H", 5))), rgba)
    # RGB 8 and 16-bit, RGB with tRNS.
    rows = [sum(([x * 4, y * 4, (x ^ y) * 4] for x in range(SIZE)), []) for y in range(SIZE)]
    rgba = [[r[i], r[i + 1], r[i + 2], 255] for r in rows for i in range(0, len(r), 3)]
    out["rgb8"] = (png(2, 8, rows), rgba)
    rows16 = [[v * 257 + 3 for v in r] for r in rows]
    out["rgb16"] = (png(2, 16, rows16), rgba)
    key = (8, 4, 12)  # x = 2, y = 1
    trns = chunk(b"tRNS", struct.pack(">HHH", *key))
    rgba_t = [[*p[:3], 0 if tuple(p[:3]) == key else 255] for p in rgba]
    out["rgb8_trns"] = (png(2, 8, rows, trns), rgba_t)
    # Grey + alpha, RGBA 8 and 16-bit.
    rows = [sum(([x * 4, (y * 4) % 256] for x in range(SIZE)), []) for y in range(SIZE)]
    rgba = [[r[i], r[i], r[i], r[i + 1]] for r in rows for i in range(0, len(r), 2)]
    out["greyalpha8"] = (png(4, 8, rows), rgba)
    rows = [sum(([x * 4, y * 4, 128, (x * y) % 256] for x in range(SIZE)), []) for y in range(SIZE)]
    rgba = [r[i:i + 4] for r in rows for i in range(0, len(r), 4)]
    out["rgba8"] = (png(6, 8, rows), rgba)
    rows16 = [[v * 257 for v in r] for r in rows]
    out["rgba16"] = (png(6, 16, rows16), rgba)
    # Palette, 8-bit and 4-bit, with partial tRNS.
    pal = [((i * 37) % 256, (i * 91) % 256, (i * 53) % 256) for i in range(16)]
    plte = chunk(b"PLTE", bytes(c for p in pal for c in p))
    alphas = [0, 64, 128] + [255] * 3  # entries 0-5; the rest opaque
    trns = chunk(b"tRNS", bytes(alphas))
    for d in (4, 8):
        rows = [[(x // 4 + y) % 16 for x in range(SIZE)] for y in range(SIZE)]
        rgba = [[*pal[v], alphas[v] if v < len(alphas) else 255] for r in rows for v in r]
        out[f"palette{d}"] = (png(3, d, rows, plte + trns), rgba)
    return out


def main():
    here = Path(__file__).parent
    folder = here / "png"
    folder.mkdir(exist_ok=True)
    doc = {"rule": __doc__.split("\n\n")[1].strip(), "images": {}}
    for name, (data, rgba) in cases().items():
        assert len(rgba) == SIZE * SIZE
        (folder / f"{name}.png").write_bytes(data)
        flat = bytes(c for p in rgba for c in p)
        doc["images"][name] = {
            "file": f"png/{name}.png",
            "rgba_sha256": hashlib.sha256(flat).hexdigest(),
            "samples": {f"{x},{y}": rgba[y * SIZE + x] for x, y in ((0, 0), (2, 1), (5, 0), (63, 63), (17, 40))},
        }
    (here / "png.json").write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8", newline="\n")
    print("wrote", len(doc["images"]), "images")


if __name__ == "__main__":
    main()
