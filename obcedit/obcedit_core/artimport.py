# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Importing replacement art: any PNG to RGBA, checked, and the palette rule.

A mod carries the author's own PNG (docs/MOD_ASSETS_AND_ARENAS.md section 2.1)
and OBCR converts it to the game's palette when it loads it. The editor shows
what the game will draw by applying the same written rule (section 2.4):

    alpha < 128              -> 255, transparent (sprites only: a wall pixel
                                is always opaque)
    otherwise                -> the entry 0-254 with the least squared RGB
                                distance, the lowest index on a tie

`quantize` is that rule, and `docs/test-vectors/quantize.json` (a synthetic
palette, never the game's) is what both this and OBCR's `obc-render::quantize`
are tested against.

The reader is the standard library's zlib plus a page of PNG: every bit depth
(1, 2, 4, 8, 16) and colour type (grey, RGB, palette, grey+alpha, RGBA),
`tRNS` transparency, and Adam7 interlacing. An image is untrusted input, so the
file size, the declared dimensions and the decompressed size are all bounded
before anything is allocated, and every chunk's CRC is checked.
"""

from __future__ import annotations

import struct
import zlib
from dataclasses import dataclass

from .errors import Diagnostic, ObcEditError, Severity
from .resources import MAX_IMAGE_BYTES

EDGE = 64
TRANSPARENT = 255
#: Refuse any declared size past this before decoding (an art file is 64x64;
#: the bound only has to stop a hostile header from asking for gigabytes).
MAX_DIMENSION = 4096

_SIGNATURE = b"\x89PNG\r\n\x1a\n"
#: (x0, y0, dx, dy) for the seven Adam7 passes.
_ADAM7 = ((0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
          (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2))
_CHANNELS = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
_DEPTHS = {0: (1, 2, 4, 8, 16), 2: (8, 16), 3: (1, 2, 4, 8), 4: (8, 16), 6: (8, 16)}


class ArtError(ObcEditError):
    """An image that cannot be a replacement. The message says what to do."""


def _error(message: str, where: str = "", code: str = "OBC-IMG-001") -> ArtError:
    return ArtError(Diagnostic(code, Severity.ERROR, message, where))


@dataclass(frozen=True)
class Image:
    """Decoded pixels: row-major RGBA, four bytes a pixel."""

    width: int
    height: int
    rgba: bytes


def read_png(blob: bytes, name: str = "the image") -> Image:
    """Decode any standard PNG to RGBA, or raise ArtError saying why not."""
    if len(blob) > MAX_IMAGE_BYTES:
        raise _error(f"{name} is {len(blob)} bytes; a replacement image may be at most "
                     f"{MAX_IMAGE_BYTES // 1024} KiB", name, "OBC-IMG-003")
    if blob[:8] != _SIGNATURE:
        raise _error(f"{name} is not a PNG file", name)
    header = None
    palette = b""
    trns = b""
    data = bytearray()
    at = 8
    ended = False
    while at + 12 <= len(blob):
        length, kind = struct.unpack_from(">I4s", blob, at)
        if length > len(blob) - at - 12:
            raise _error(f"{name} ends part way through a {kind!r} chunk", name)
        body = blob[at + 8:at + 8 + length]
        crc = struct.unpack_from(">I", blob, at + 8 + length)[0]
        if zlib.crc32(kind + body) & 0xFFFFFFFF != crc:
            raise _error(f"{name} is damaged (a {kind.decode('latin-1')} chunk fails its "
                         "checksum)", name)
        if kind == b"IHDR":
            if length != 13:
                raise _error(f"{name} has a malformed header", name)
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            data.extend(body)
        elif kind == b"IEND":
            ended = True
            break
        at += 12 + length
    if header is None:
        raise _error(f"{name} has no PNG header", name)
    if not ended:
        raise _error(f"{name} is truncated (no end marker)", name)
    width, height, depth, mode, compression, filtering, interlace = header
    if mode not in _CHANNELS or depth not in _DEPTHS[mode]:
        raise _error(f"{name} uses PNG colour type {mode} at {depth} bits, which is not a "
                     "valid PNG combination", name)
    if compression or filtering or interlace not in (0, 1):
        raise _error(f"{name} uses a PNG compression, filter or interlace method that "
                     "does not exist", name)
    if not (0 < width <= MAX_DIMENSION and 0 < height <= MAX_DIMENSION):
        raise _error(f"{name} declares {width}x{height} pixels", name)
    if mode == 3 and (not palette or len(palette) % 3):
        raise _error(f"{name} is paletted but carries no valid palette", name)

    channels = _CHANNELS[mode]
    bits = channels * depth
    passes = _ADAM7 if interlace else ((0, 0, 1, 1),)
    sizes = []
    expected = 0
    for x0, y0, dx, dy in passes:
        pw = (width - x0 + dx - 1) // dx if width > x0 else 0
        ph = (height - y0 + dy - 1) // dy if height > y0 else 0
        stride = (pw * bits + 7) // 8
        sizes.append((pw, ph, stride))
        if pw and ph:
            expected += (stride + 1) * ph
    inflater = zlib.decompressobj()
    try:
        raw = inflater.decompress(bytes(data), expected + 1)
    except zlib.error as error:
        raise _error(f"{name} has damaged image data: {error}", name) from error
    if len(raw) < expected:
        raise _error(f"{name} ends part way through the image", name)

    rgba = bytearray(width * height * 4)
    at = 0
    bpp = max(1, bits // 8)
    for (x0, y0, dx, dy), (pw, ph, stride) in zip(passes, sizes):
        if not pw or not ph:
            continue
        previous = bytearray(stride)
        for row in range(ph):
            method = raw[at]
            line = bytearray(raw[at + 1:at + 1 + stride])
            at += stride + 1
            _unfilter(method, line, previous, bpp, name)
            previous = line
            samples = _samples(line, pw * channels, depth)
            y = y0 + row * dy
            for column in range(pw):
                x = x0 + column * dx
                pixel = samples[column * channels:(column + 1) * channels]
                rgba[(y * width + x) * 4:(y * width + x) * 4 + 4] = _to_rgba(
                    pixel, mode, depth, palette, trns)
    return Image(width, height, bytes(rgba))


def _unfilter(method: int, line: bytearray, previous: bytearray, bpp: int, name: str) -> None:
    if method == 0:
        return
    if method > 4:
        raise _error(f"{name} uses an unknown PNG row filter ({method})", name)
    for i in range(len(line)):
        a = line[i - bpp] if i >= bpp else 0
        b = previous[i]
        c = previous[i - bpp] if i >= bpp else 0
        if method == 1:
            line[i] = (line[i] + a) & 0xFF
        elif method == 2:
            line[i] = (line[i] + b) & 0xFF
        elif method == 3:
            line[i] = (line[i] + (a + b) // 2) & 0xFF
        else:
            p = a + b - c
            pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
            line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF


def _samples(line: bytes, count: int, depth: int) -> list[int]:
    if depth == 8:
        return list(line[:count])
    if depth == 16:
        return [(line[i * 2] << 8) | line[i * 2 + 1] for i in range(count)]
    per = 8 // depth
    mask = (1 << depth) - 1
    out = []
    for i in range(count):
        byte = line[i // per]
        shift = 8 - depth * (i % per + 1)
        out.append((byte >> shift) & mask)
    return out


def _scale(value: int, depth: int) -> int:
    """A sample to 8 bits, as the PNG specification's reference scaling."""
    if depth == 8:
        return value
    if depth == 16:
        return value >> 8
    return value * 255 // ((1 << depth) - 1)


def _to_rgba(pixel, mode: int, depth: int, palette: bytes, trns: bytes) -> bytes:
    if mode == 3:
        index = pixel[0]
        if index * 3 + 3 > len(palette):
            return b"\0\0\0\0"
        alpha = trns[index] if index < len(trns) else 255
        return palette[index * 3:index * 3 + 3] + bytes((alpha,))
    if mode == 0:
        grey = _scale(pixel[0], depth)
        alpha = 0 if len(trns) >= 2 and pixel[0] == struct.unpack(">H", trns[:2])[0] else 255
        return bytes((grey, grey, grey, alpha))
    if mode == 4:
        grey = _scale(pixel[0], depth)
        return bytes((grey, grey, grey, _scale(pixel[1], depth)))
    if mode == 2:
        alpha = 255
        if len(trns) >= 6 and tuple(pixel) == struct.unpack(">HHH", trns[:6]):
            alpha = 0
        return bytes((_scale(pixel[0], depth), _scale(pixel[1], depth),
                      _scale(pixel[2], depth), alpha))
    return bytes(_scale(v, depth) for v in pixel)


# ---------------------------------------------------------------------------
# Replacements
# ---------------------------------------------------------------------------


def check_art(blob: bytes, name: str = "the image") -> Image:
    """A replacement wall page or sprite: a readable 64x64 PNG."""
    image = read_png(blob, name)
    if (image.width, image.height) != (EDGE, EDGE):
        raise _error(f"{name} is {image.width}x{image.height}; wall pages and sprites are "
                     f"{EDGE}x{EDGE}", name, "OBC-IMG-002")
    return image


def _palette_triples(palette) -> list[tuple[int, int, int]]:
    """Accept a flat 768-list (assets.load_palette) or 256 triples."""
    flat = list(palette)
    if flat and isinstance(flat[0], (tuple, list)):
        return [tuple(entry) for entry in flat]
    return [tuple(flat[i:i + 3]) for i in range(0, len(flat), 3)]


class Quantizer:
    """The section-2.4 rule over one palette, with a cache of answers."""

    def __init__(self, palette) -> None:
        self.entries = _palette_triples(palette)[:255]
        self._cache: dict[int, int] = {}

    def nearest(self, red: int, green: int, blue: int) -> int:
        key = (red << 16) | (green << 8) | blue
        found = self._cache.get(key)
        if found is not None:
            return found
        best, best_distance = 0, None
        for index, (r, g, b) in enumerate(self.entries):
            distance = (r - red) ** 2 + (g - green) ** 2 + (b - blue) ** 2
            if best_distance is None or distance < best_distance:
                best, best_distance = index, distance
                if not distance:
                    break
        self._cache[key] = best
        return best

    def index(self, red: int, green: int, blue: int, alpha: int, *, sprite: bool) -> int:
        if sprite and alpha < 128:
            return TRANSPARENT
        return self.nearest(red, green, blue)

    def quantize(self, rgba: bytes, *, sprite: bool) -> bytes:
        out = bytearray(len(rgba) // 4)
        for i in range(len(out)):
            r, g, b, a = rgba[i * 4:i * 4 + 4]
            out[i] = self.index(r, g, b, a, sprite=sprite)
        return bytes(out)


def quantize(rgba: bytes, palette, *, sprite: bool) -> bytes:
    """Palette indices for row-major RGBA pixels, by the section-2.4 rule."""
    return Quantizer(palette).quantize(rgba, sprite=sprite)


def preview(image: Image, palette, *, sprite: bool, quantizer: Quantizer | None = None) -> bytes:
    """What the game will draw, in the thumbnail's format.

    Walls: row-major RGB (three bytes a pixel). Sprites: RGBA with index 255
    fully transparent -- the same buffers `assets.wall_rgb` and
    `assets.sprite_rgba` produce for the original artwork.
    """
    quantizer = quantizer or Quantizer(palette)
    flat = [c for entry in _palette_triples(palette) for c in entry]
    indices = quantizer.quantize(image.rgba, sprite=sprite)
    out = bytearray(len(indices) * (4 if sprite else 3))
    for i, index in enumerate(indices):
        if sprite:
            if index == TRANSPARENT:
                continue
            out[i * 4:i * 4 + 4] = bytes((flat[index * 3], flat[index * 3 + 1],
                                          flat[index * 3 + 2], 255))
        else:
            out[i * 3:i * 3 + 3] = bytes(flat[index * 3:index * 3 + 3])
    return bytes(out)


def raw_preview(image: Image, *, sprite: bool) -> bytes:
    """The author's pixels unconverted (no game palette open): RGB or RGBA."""
    if sprite:
        return bytes(b if (i % 4) != 3 else (255 if b >= 128 else 0)
                     for i, b in enumerate(image.rgba))
    out = bytearray(image.width * image.height * 3)
    for i in range(image.width * image.height):
        out[i * 3:i * 3 + 3] = image.rgba[i * 4:i * 4 + 3]
    return bytes(out)


__all__ = ["ArtError", "EDGE", "Image", "Quantizer", "TRANSPARENT", "check_art", "preview",
           "quantize", "raw_preview", "read_png"]
