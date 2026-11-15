# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Checking replacement sounds and music before they go into a project.

Section 2.3 of docs/MOD_ASSETS_AND_ARENAS.md sets what OBCR accepts:

* a sound is a WAV: PCM 8 or 16-bit or 32-bit float, mono or stereo,
  4,000-48,000 Hz, at most 30 seconds and 4 MiB;
* music is Ogg Vorbis, QOA (mono or stereo, up to 48 kHz) or IMF (AdLib), at
  most 16 MiB.

The editor reads headers only. OBCR decodes every file in full when it checks
the mod (`--check-mod`), which is the final word; these checks are there so
that a file OBCR would refuse is refused at import, with a reason, rather than
at export. A WAV chosen as *music* is converted to QOA by OBCR itself
(`obcr --convert-audio`, engine_runner.convert_audio).
"""

from __future__ import annotations

import struct
from dataclasses import dataclass

from .errors import Diagnostic, ObcEditError, Severity
from .resources import MAX_MUSIC_BYTES, MAX_SOUND_BYTES, MAX_SOUND_SECONDS

MIN_RATE = 4000
MAX_RATE = 48000
_PCM, _FLOAT, _EXTENSIBLE = 1, 3, 0xFFFE
#: The GUID tail shared by the KSDATAFORMAT_SUBTYPE_* GUIDs.
_GUID_TAIL = b"\x00\x00\x00\x00\x10\x00\x80\x00\x00\xaa\x00\x38\x9b\x71"


class AudioError(ObcEditError):
    """A sound or music file OBCR would refuse."""


def _error(message: str, where: str = "", code: str = "OBC-AUDIO-001") -> AudioError:
    return AudioError(Diagnostic(code, Severity.ERROR, message, where))


@dataclass(frozen=True)
class AudioInfo:
    """What a header says."""

    format: str          # "wav" | "ogg" | "qoa" | "imf"
    channels: int = 0
    rate: int = 0
    bits: int = 0
    seconds: float = 0.0

    def describe(self) -> str:
        if self.format == "imf":
            return f"IMF (AdLib), {self.seconds:.1f} s" if self.seconds else "IMF (AdLib)"
        layout = {1: "mono", 2: "stereo"}.get(self.channels, f"{self.channels} channels")
        length = f", {self.seconds:.1f} s" if self.seconds else ""
        bits = f" {self.bits}-bit" if self.bits else ""
        return f"{self.format.upper()}{bits} {layout} {self.rate} Hz{length}"


def check_wav(blob: bytes, name: str = "the sound", *, max_bytes: int = MAX_SOUND_BYTES,
              max_seconds: float | None = MAX_SOUND_SECONDS) -> AudioInfo:
    """A WAV OBCR plays as a replacement sound (or converts to music)."""
    if len(blob) > max_bytes:
        raise _error(f"{name} is {len(blob)} bytes; the limit is {max_bytes >> 20} MiB",
                     name, "OBC-AUDIO-002")
    if len(blob) < 12 or blob[:4] != b"RIFF" or blob[8:12] != b"WAVE":
        raise _error(f"{name} is not a WAV file (RIFF/WAVE)", name)
    at = 12
    fmt = None
    data_size = None
    while at + 8 <= len(blob):
        kind, size = struct.unpack_from("<4sI", blob, at)
        body = blob[at + 8:at + 8 + size]
        if kind == b"fmt ":
            if len(body) < 16:
                raise _error(f"{name} has a malformed format chunk", name)
            fmt = body
        elif kind == b"data":
            data_size = min(size, len(blob) - at - 8)
            break
        at += 8 + size + (size & 1)
    if fmt is None or data_size is None:
        raise _error(f"{name} has no {'format' if fmt is None else 'data'} chunk", name)
    tag, channels, rate, _, align, bits = struct.unpack_from("<HHIIHH", fmt)
    if tag == _EXTENSIBLE:
        if len(fmt) < 40 or fmt[26:40] != _GUID_TAIL:
            raise _error(f"{name} uses an extensible format OBCR does not read", name)
        tag = struct.unpack_from("<H", fmt, 24)[0]
    if tag == _PCM and bits not in (8, 16):
        raise _error(f"{name} is {bits}-bit PCM; OBCR reads 8 and 16-bit PCM or 32-bit float",
                     name)
    if tag == _FLOAT and bits != 32:
        raise _error(f"{name} is {bits}-bit float; OBCR reads 32-bit float", name)
    if tag not in (_PCM, _FLOAT):
        raise _error(f"{name} is compressed (WAV format {tag}); save it as PCM", name)
    if channels not in (1, 2):
        raise _error(f"{name} has {channels} channels; OBCR reads mono and stereo", name)
    if not MIN_RATE <= rate <= MAX_RATE:
        raise _error(f"{name} is {rate} Hz; OBCR reads {MIN_RATE}-{MAX_RATE} Hz", name)
    frame = channels * bits // 8
    if align != frame:
        raise _error(f"{name} has an inconsistent block alignment", name)
    seconds = data_size / (frame * rate)
    if max_seconds is not None and seconds > max_seconds:
        raise _error(f"{name} is {seconds:.1f} seconds; a sound may be at most "
                     f"{max_seconds} seconds", name, "OBC-AUDIO-002")
    return AudioInfo("wav", channels, rate, bits, seconds)


def check_ogg(blob: bytes, name: str = "the music") -> AudioInfo:
    """The first page of an Ogg Vorbis stream: the identification header."""
    _size(blob, name)
    if len(blob) < 58 or blob[:4] != b"OggS" or blob[4] != 0:
        raise _error(f"{name} is not an Ogg file", name)
    if not blob[5] & 0x02:
        raise _error(f"{name} does not begin an Ogg stream", name)
    segments = blob[26]
    header_end = 27 + segments
    if header_end > len(blob):
        raise _error(f"{name} is truncated", name)
    packet = blob[header_end:header_end + sum(blob[27:header_end])]
    if len(packet) < 30 or packet[:7] != b"\x01vorbis":
        raise _error(f"{name} is Ogg but not Vorbis (Opus and FLAC-in-Ogg are not read); "
                     "export it as Ogg Vorbis", name)
    version, channels, rate = struct.unpack_from("<IBI", packet, 7)
    if version != 0 or channels == 0 or rate == 0:
        raise _error(f"{name} has an invalid Vorbis header", name)
    if channels > 2:
        raise _error(f"{name} has {channels} channels; OBCR plays mono and stereo", name)
    if rate > MAX_RATE:
        raise _error(f"{name} is {rate} Hz; OBCR plays up to {MAX_RATE} Hz", name)
    return AudioInfo("ogg", channels, rate)


def check_qoa(blob: bytes, name: str = "the music") -> AudioInfo:
    """A QOA file: `qoaf`, the sample count, then frames (qoaformat.org)."""
    _size(blob, name)
    if len(blob) < 16 or blob[:4] != b"qoaf":
        raise _error(f"{name} is not a QOA file", name)
    samples = struct.unpack_from(">I", blob, 4)[0]
    channels = blob[8]
    rate = int.from_bytes(blob[9:12], "big")
    if samples == 0:
        raise _error(f"{name} is a streaming QOA without a length; OBCR reads static files",
                     name)
    if channels not in (1, 2):
        raise _error(f"{name} has {channels} channels; OBCR plays mono and stereo QOA", name)
    if not 0 < rate <= MAX_RATE:
        raise _error(f"{name} is {rate} Hz; OBCR plays QOA up to {MAX_RATE} Hz", name)
    return AudioInfo("qoa", channels, rate, 16, samples / rate)


def check_imf(blob: bytes, name: str = "the music") -> AudioInfo:
    """An IMF song as the game stores one: a 16-bit length, then 4-byte commands.

    The commands are (register, value, 16-bit delay) at 700 Hz -- the same
    layout as the original's songs (docs/re/AUDIO.md).
    """
    _size(blob, name)
    if len(blob) < 6:
        raise _error(f"{name} is too short to be an IMF song", name)
    length = struct.unpack_from("<H", blob)[0]
    if length == 0 or length % 4 or length > len(blob) - 2:
        raise _error(f"{name} is not an IMF song with a length field: it declares {length} "
                     f"bytes of commands in a {len(blob)}-byte file", name)
    seconds = 0.0
    if length <= 1 << 20:  # a running time is a nicety; not worth seconds of work
        delay = sum(struct.unpack_from("<H", blob, 2 + i + 2)[0] for i in range(0, length, 4))
        seconds = delay / 700
    return AudioInfo("imf", seconds=seconds)


def check_music(blob: bytes, extension: str, name: str = "the music") -> AudioInfo:
    extension = extension.lower().lstrip(".")
    if extension == "ogg":
        return check_ogg(blob, name)
    if extension == "qoa":
        return check_qoa(blob, name)
    if extension == "imf":
        return check_imf(blob, name)
    raise _error(f"{name}: music is Ogg Vorbis (.ogg), QOA (.qoa) or IMF (.imf); "
                 "a WAV is converted to QOA on import", name)


def _size(blob: bytes, name: str) -> None:
    if len(blob) > MAX_MUSIC_BYTES:
        raise _error(f"{name} is {len(blob)} bytes; a music file may be at most "
                     f"{MAX_MUSIC_BYTES >> 20} MiB", name, "OBC-AUDIO-002")


__all__ = ["AudioError", "AudioInfo", "check_imf", "check_music", "check_ogg", "check_qoa",
           "check_wav"]
