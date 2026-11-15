# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Bringing an author's file into a project: check it, store it, name it.

One function per question the Resources panel asks, all free of Qt so they are
tested anywhere:

* `import_art` -- a PNG replacing a wall page or sprite (checked 64x64 and
  readable; stored exactly as the author made it);
* `import_sound` -- a WAV replacing a digitized sound;
* `import_music` -- an Ogg Vorbis, QOA or IMF file stored as is, or a WAV
  converted to QOA by OBCR (`obcr --convert-audio`).

Each returns the `Resource` record; the caller turns it into an undoable
command (`commands.set_resource`). Nothing here touches the game's data.
"""

from __future__ import annotations

import re
import tempfile
from pathlib import Path

from . import artimport, audioimport
from .engine_runner import convert_audio
from .errors import Diagnostic, ObcEditError, Severity
from .resources import (
    MAX_MUSIC_BYTES,
    MUSIC,
    SOUND,
    SPRITE,
    WALL,
    Resource,
    ResourceStore,
    check_target,
)


class ImportError_(ObcEditError):
    """The file cannot be imported; the message says why."""


def _error(message: str, where: str = "") -> ImportError_:
    return ImportError_(Diagnostic("OBC-RES-004", Severity.ERROR, message, where))


def _read(path: Path, limit: int) -> bytes:
    try:
        size = path.stat().st_size
    except OSError as error:
        raise _error(f"cannot read {path.name}: {error.strerror or error}", path.name) from error
    if size > limit:
        raise _error(f"{path.name} is {size} bytes; the limit is {limit} bytes", path.name)
    return path.read_bytes()


def import_art(store: ResourceStore, kind: str, target: int, path: Path | str) -> Resource:
    """A 64x64 PNG replacing wall page or sprite `target`."""
    if kind not in (WALL, SPRITE):
        raise _error(f"{kind} is not art")
    check_target(kind, target)
    path = Path(path)
    blob = _read(path, artimport.MAX_IMAGE_BYTES)
    artimport.check_art(blob, path.name)
    return store.put(blob, kind, target, path.name)


def import_sound(store: ResourceStore, target: int, path: Path | str) -> Resource:
    """A WAV replacing digitized sound `target`."""
    check_target(SOUND, target)
    path = Path(path)
    blob = _read(path, audioimport.MAX_SOUND_BYTES)
    audioimport.check_wav(blob, path.name)
    return store.put(blob, SOUND, target, path.name)


def music_name_for(path: Path | str, taken=(), *, extension: str | None = None) -> str:
    """A section-2.1 music name from a file name, not clashing with `taken`."""
    path = Path(path)
    extension = (extension or path.suffix.lstrip(".")).lower()
    stem = re.sub(r"[^A-Za-z0-9_-]+", "-", path.stem).strip("-") or "music"
    taken = {name.lower() for name in taken}
    base = stem[:32]
    candidate = f"{base}.{extension}"
    number = 2
    while candidate.lower() in taken:
        suffix = f"-{number}"
        candidate = f"{base[:32 - len(suffix)]}{suffix}.{extension}"
        number += 1
    return candidate


def import_music(store: ResourceStore, path: Path | str, *, name: str | None = None,
                 taken=(), engine: Path | str | None = None) -> Resource:
    """Music: .ogg/.qoa/.imf stored as given; .wav converted to QOA by OBCR."""
    path = Path(path)
    extension = path.suffix.lower().lstrip(".")
    if extension == "wav":
        blob = _read(path, 512 << 20)
        audioimport.check_wav(blob, path.name, max_bytes=512 << 20, max_seconds=None)
        with tempfile.TemporaryDirectory(prefix="obcedit-music-") as work:
            output = Path(work) / "converted.qoa"
            done = convert_audio(engine, path, output)
            if not done.ok:
                raise _error(done.message, path.name)
            blob = output.read_bytes()
        extension = "qoa"
    elif extension in ("ogg", "qoa", "imf"):
        blob = _read(path, MAX_MUSIC_BYTES)
    else:
        raise _error(f"{path.name}: music is Ogg Vorbis, QOA, IMF, or a WAV to convert",
                     path.name)
    if len(blob) > MAX_MUSIC_BYTES:
        raise _error(f"{path.name} is {len(blob)} bytes as {extension.upper()}; a music file "
                     f"may be at most {MAX_MUSIC_BYTES >> 20} MiB", path.name)
    audioimport.check_music(blob, extension, path.name)
    name = name or music_name_for(path, taken, extension=extension)
    if not name.lower().endswith("." + extension):
        name = f"{name.rsplit('.', 1)[0]}.{extension}"
    return store.put(blob, MUSIC, name, path.name)


__all__ = ["ImportError_", "import_art", "import_music", "import_sound", "music_name_for"]
