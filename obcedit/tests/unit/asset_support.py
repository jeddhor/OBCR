# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Synthetic art, sounds and music for the format-2 tests.

Everything here is made up in the test: PNGs in every colour type, WAVs,
the first page of an Ogg Vorbis stream, a QOA header, an IMF song. No game
data, no palette from the game.
"""

from __future__ import annotations

import io
import os
import struct
import sys
import wave
import zlib
from pathlib import Path

_ADAM7 = ((0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
          (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2))


def _chunk(kind: bytes, body: bytes) -> bytes:
    return (struct.pack(">I", len(body)) + kind + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF))


def _pack_row(samples, depth: int) -> bytes:
    if depth == 8:
        return bytes(samples)
    if depth == 16:
        return b"".join(struct.pack(">H", v) for v in samples)
    per = 8 // depth
    out = bytearray((len(samples) + per - 1) // per)
    for i, value in enumerate(samples):
        out[i // per] |= value << (8 - depth * (i % per + 1))
    return bytes(out)


def png(width: int, height: int, pixel, *, mode: int = 6, depth: int = 8,
        palette: bytes = b"", trns: bytes = b"", interlace: bool = False,
        filter_type: int = 0) -> bytes:
    """A PNG whose sample tuple at (x, y) is `pixel(x, y)`."""
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[mode]
    passes = _ADAM7 if interlace else ((0, 0, 1, 1),)
    raw = bytearray()
    for x0, y0, dx, dy in passes:
        xs = list(range(x0, width, dx))
        ys = list(range(y0, height, dy))
        if not xs or not ys:
            continue
        for y in ys:
            samples = []
            for x in xs:
                value = pixel(x, y)
                samples.extend(value if isinstance(value, tuple) else (value,))
            assert len(samples) == len(xs) * channels
            line = _pack_row(samples, depth)
            if filter_type == 1:  # Sub, to exercise the unfilter
                bpp = max(1, channels * depth // 8)
                line = bytes((line[i] - (line[i - bpp] if i >= bpp else 0)) & 0xFF
                             for i in range(len(line)))
            raw.append(filter_type)
            raw += line
    body = struct.pack(">IIBBBBB", width, height, depth, mode, 0, 0, 1 if interlace else 0)
    out = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", body)
    if palette:
        out += _chunk(b"PLTE", palette)
    if trns:
        out += _chunk(b"tRNS", trns)
    return out + _chunk(b"IDAT", zlib.compress(bytes(raw))) + _chunk(b"IEND", b"")


def art(color=(200, 40, 40), *, edge: int = 64, hole: bool = False) -> bytes:
    """A 64x64 RGBA image: one colour, with a transparent square when `hole`."""
    def pixel(x, y):
        if hole and 16 <= x < 48 and 16 <= y < 48:
            return (0, 0, 0, 0)
        return (*color, 255)
    return png(edge, edge, pixel)


def wav(seconds: float = 0.25, *, rate: int = 11025, channels: int = 1, width: int = 2) -> bytes:
    buffer = io.BytesIO()
    with wave.open(buffer, "wb") as out:
        out.setnchannels(channels)
        out.setsampwidth(width)
        out.setframerate(rate)
        out.writeframes(b"\x00" * int(seconds * rate) * channels * width)
    return buffer.getvalue()


def float_wav(seconds: float = 0.1, rate: int = 22050) -> bytes:
    frames = int(seconds * rate)
    data = struct.pack(f"<{frames}f", *([0.0] * frames))
    fmt = struct.pack("<HHIIHH", 3, 1, rate, rate * 4, 4, 32)
    body = b"WAVE" + b"fmt " + struct.pack("<I", len(fmt)) + fmt \
        + b"data" + struct.pack("<I", len(data)) + data
    return b"RIFF" + struct.pack("<I", len(body)) + body


def ogg(channels: int = 2, rate: int = 44100, *, codec: bytes = b"\x01vorbis") -> bytes:
    packet = codec + struct.pack("<IBIiiiBB", 0, channels, rate, 0, 128000, 0, 0xB8, 1)
    header = (b"OggS" + bytes((0, 2)) + b"\0" * 8 + struct.pack("<III", 1, 0, 0)
              + bytes((1, len(packet))))
    return header + packet + b"OggS" + b"\0" * 64


def qoa(channels: int = 2, rate: int = 44100, samples: int = 44100) -> bytes:
    return (b"qoaf" + struct.pack(">I", samples) + bytes((channels,))
            + rate.to_bytes(3, "big") + struct.pack(">HH", 5120, 8) + b"\0" * 64)


def imf(commands: int = 8) -> bytes:
    body = b"".join(struct.pack("<BBH", 0x20, 0x01, 70) for _ in range(commands))
    return struct.pack("<H", len(body)) + body


def fake_obcr(directory: Path, body: str) -> Path:
    """A stand-in `obcr` running `body` (Python, with `argv`)."""
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / "fake_obcr.py"
    script.write_text("import os, sys\nargv = sys.argv[1:]\n" + body + "\n", encoding="utf-8")
    python = getattr(sys, "_base_executable", "") or sys.executable
    if os.name == "nt":
        launcher = directory / "fake_obcr.cmd"
        launcher.write_text(f'@"{python}" "{script}" %*\r\n@exit /b %errorlevel%\r\n',
                            encoding="ascii")
    else:
        launcher = directory / "fake_obcr"
        launcher.write_text(f'#!/bin/sh\nexec "{python}" "{script}" "$@"\n', encoding="utf-8")
        launcher.chmod(0o755)
    return launcher


#: A fake OBCR whose --convert-audio writes a small QOA file.
CONVERTS = r'''
if "--convert-audio" in argv:
    source, target = argv[argv.index("--convert-audio") + 1:argv.index("--convert-audio") + 3]
    data = b"qoaf" + (22050).to_bytes(4, "big") + bytes((1,)) + (22050).to_bytes(3, "big") + b"\0" * 64
    open(target, "wb").write(data)
    sys.exit(0)
sys.exit(2)
'''

#: An OBCR from before format 2: clap-style refusal of the flag.
TOO_OLD = r'''
sys.stderr.write("error: unexpected argument '--convert-audio' found\n")
sys.exit(2)
'''
