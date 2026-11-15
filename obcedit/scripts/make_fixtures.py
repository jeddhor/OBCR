#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Generate the synthetic binaries OBCEdit's tests are allowed to use.

Every byte these produce is computed here, from constants written here. None of
it is derived from Operation Body Count -- not a wall index, not a palette
entry, not a map name -- because the editor's binary boundaries have to be
tested in CI on machines with no right to the commercial data, and a fixture
cut from the original archive could never be committed.

Being synthetic is made checkable:

  * plane words are drawn from a band (0xE000+) the game's maps never use, so a
    real word cannot be mistaken for one of ours;
  * every map is named "SYNTH..." and every archive carries a generator tag;
  * `verify` re-runs the generators and compares digests, so a fixture edited
    by hand -- or replaced with real data -- stops the gate.

There is no randomness at all, seeded or otherwise.

Usage:
  make_fixtures.py write DIR     write the fixture set
  make_fixtures.py digests       print name and sha256 of each, without writing
  make_fixtures.py verify DIR    re-generate and compare against what is there
"""

from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path

# The native contract, restated here on purpose: duplicated rather than
# imported so a fixture cannot drift silently with the code under test.
SIGNATURE = b"TED5v1.0"
MAP_MARKER = b"!ID!"
RLEW_TAG = 0xABCD
NAME_FIELD = 16
SLOTS = 100
HEADER = 38
SYNTH_BASE = 0xE000


def rlew_compress(words: list[int]) -> bytes:
    """A legal RLEW encoding with runs of three or more.

    Deliberately not the production writer's (which uses four, matching the
    original): reading these is a real test of the decoder rather than a round
    trip through one shared assumption.
    """
    out = bytearray()
    i = 0
    while i < len(words):
        value = words[i]
        run = 1
        while i + run < len(words) and words[i + run] == value:
            run += 1
        if run >= 3 or value == RLEW_TAG:
            out += struct.pack("<HHH", RLEW_TAG, run, value)
            i += run
        else:
            out += struct.pack("<H", value)
            i += 1
    return bytes(out)


def plane_stream(words: list[int]) -> bytes:
    return struct.pack("<H", len(words) * 2) + rlew_compress(words)


def synth_plane(width: int, height: int, salt: int) -> list[int]:
    """Runs and literals, all in the synthetic band."""
    words = []
    for y in range(height):
        for x in range(width):
            if (x + y + salt) % 5 < 2:
                words.append(SYNTH_BASE + salt)
            else:
                words.append(SYNTH_BASE + ((x * 7 + y * 3 + salt) & 0xFF))
    return words


def build_archive(maps: list[tuple[int, str, int, int]], *, terminate: bool = True,
                  tail_garbage: bytes = b"") -> tuple[bytes, bytes]:
    """(MAPHEAD, MAPTEMP) for `(slot, name, width, height)` maps."""
    offsets = [-1] * SLOTS
    temp = bytearray(SIGNATURE)
    for slot, name, width, height in maps:
        streams = [plane_stream(synth_plane(width, height, slot * 3 + p)) for p in range(3)]
        header = len(temp)
        offsets[slot] = header
        starts, position = [], header + HEADER
        for stream in streams:
            starts.append(position)
            position += len(stream)
        raw_name = name.encode("ascii").ljust(NAME_FIELD - len(tail_garbage), b"\0") + tail_garbage
        temp += struct.pack("<3i3HHH", *starts, *(len(s) for s in streams), width, height)
        temp += raw_name[:NAME_FIELD]
        for stream in streams:
            temp += stream
        if terminate:
            temp += MAP_MARKER
    head = struct.pack("<H", RLEW_TAG) + struct.pack(f"<{SLOTS}i", *offsets)
    return head, bytes(temp)


def build_wall_page(salt: int = 0) -> bytes:
    return bytes((x * 3 + y + salt) & 0xFF for x in range(64) for y in range(64))


def build_sprite_page(left: int = 20, right: int = 43) -> bytes:
    """A column-post sprite: one post per column, rows 16..47, pixels 0x40+."""
    columns = right - left + 1
    table = 4 + columns * 2
    pixels_at = table
    pixel_bytes = bytes((0x40 + i) & 0xFF for i in range(64))
    commands_at = pixels_at + len(pixel_bytes)
    page = bytearray(struct.pack("<HH", left, right))
    command_offsets = []
    commands = bytearray()
    for column in range(columns):
        command_offsets.append(commands_at + len(commands))
        start, end = 16, 48
        source = pixels_at - start + (column % 8)
        commands += struct.pack("<HhH", end * 2, source, start * 2) + struct.pack("<H", 0)
    page += struct.pack(f"<{columns}H", *command_offsets)
    page += pixel_bytes
    page += commands
    return bytes(page)


def build_page_file(walls: int = 2, sprites: int = 2) -> bytes:
    """A VSWAP page file with `walls` wall pages and `sprites` sprites, no sounds."""
    pages = [build_wall_page(i) for i in range(walls)] + [build_sprite_page() for _ in range(sprites)]
    count = len(pages)
    directory = 6 + count * 6
    offsets, position = [], directory
    for page in pages:
        offsets.append(position)
        position += len(page)
    head = struct.pack("<HHH", count, walls, count)
    head += struct.pack(f"<{count}I", *offsets) + struct.pack(f"<{count}H", *(len(p) for p in pages))
    return head + b"".join(pages)


def build_palette_executable(name: str = "COUNT.EXE") -> bytes:
    """A file of the size of `name` with a 6-bit ramp palette at its offset."""
    size, offset = {"COUNT.EXE": (235440, 0x2DDB0), "BCCD.EXE": (237056, 0x2E450)}[name]
    blob = bytearray(size)
    blob[offset:offset + 768] = bytes((i // 3) & 0x3F for i in range(768))
    return bytes(blob)


def malformed() -> dict[str, bytes]:
    """Archives the reader must refuse, each for one reason."""
    good_head, good_temp = build_archive([(0, "SYNTH00", 8, 8)])
    out = {}
    out["bad-short-maphead.MAPHEAD.BC"] = good_head[:100]
    out["bad-short-maphead.MAPTEMP.BC"] = good_temp
    out["bad-truncated.MAPHEAD.BC"] = good_head
    out["bad-truncated.MAPTEMP.BC"] = good_temp[:60]
    out["bad-empty.MAPHEAD.BC"] = struct.pack("<H", RLEW_TAG) + struct.pack(f"<{SLOTS}i", *([-1] * SLOTS))
    out["bad-empty.MAPTEMP.BC"] = SIGNATURE
    zero_size = bytearray(good_temp)
    struct.pack_into("<HH", zero_size, len(SIGNATURE) + 18, 0, 8)
    out["bad-zero-width.MAPHEAD.BC"] = good_head
    out["bad-zero-width.MAPTEMP.BC"] = bytes(zero_size)
    return out


def fixture_set() -> dict[str, bytes]:
    head, temp = build_archive([(0, "SYNTH00", 8, 8), (1, "SYNTH01", 16, 16),
                                (5, "SYNTH05", 64, 64)])
    out = {"synthetic.MAPHEAD.BC": head, "synthetic.MAPTEMP.BC": temp,
           "synthetic.GFXTILES.BC": build_page_file(),
           "synthetic.COUNT.EXE": build_palette_executable()}
    tail_head, tail_temp = build_archive([(0, "SYNTH", 4, 4)], tail_garbage=b"\x01T\x00")
    out["tailname.MAPHEAD.BC"] = tail_head
    out["tailname.MAPTEMP.BC"] = tail_temp
    out.update(malformed())
    return out


def digests() -> dict[str, str]:
    return {name: hashlib.sha256(blob).hexdigest() for name, blob in sorted(fixture_set().items())}


def main() -> int:
    if len(sys.argv) >= 2 and sys.argv[1] == "digests":
        for name, digest in digests().items():
            print(f"{digest}  {name}")
        return 0
    if len(sys.argv) >= 3 and sys.argv[1] in ("write", "verify"):
        root = Path(sys.argv[2])
        if sys.argv[1] == "write":
            root.mkdir(parents=True, exist_ok=True)
            for name, blob in fixture_set().items():
                (root / name).write_bytes(blob)
            print(f"wrote {len(fixture_set())} fixtures to {root}")
            return 0
        bad = [name for name, blob in fixture_set().items()
               if not (root / name).is_file() or (root / name).read_bytes() != blob]
        for name in bad:
            print(f"MISMATCH {name}")
        return 1 if bad else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
