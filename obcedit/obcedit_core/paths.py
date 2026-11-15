# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Source protection and atomic output.

Operation Body Count's data files are the user's own property and this project has no
right to alter them, so the rule is stronger than "try not to": every write
goes through a guard that has to be told which roots are off limits, and the
source's digest is checked again afterward. A tool that merely intends to be
read-only is one argument-order mistake away from not being.

Three ways a write can reach a file it must not touch, all covered here:

* the path is inside a protected root;
* the path *is* the source, spelled differently -- `./MAPTEMP.BC`, a symlink,
  `~/games/../games/data`;
* the path is a hard link to the source, which no amount of string
  normalization will reveal. That one needs the inode.
"""

from __future__ import annotations

import hashlib
import os
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .errors import export_error

_READ_CHUNK = 1 << 20


def canonical(path: Path | str) -> Path:
    """One spelling per file: expanded, absolute, symlinks resolved."""
    return Path(path).expanduser().resolve()


def digest_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def digest_file(path: Path | str) -> str:
    """SHA-256 without reading the whole file into memory."""
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(_READ_CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def same_file(left: Path, right: Path) -> bool:
    """True for the same bytes on disk, including through a hard link."""
    try:
        return os.path.samefile(left, right)
    except OSError:
        return canonical(left) == canonical(right)


#: What an Operation Body Count data directory holds: the page file and an
#: executable. Not `*.BC`: a mod folder holds MAPHEAD.BC and MAPTEMP.BC of its
#: own, and refusing to write a mod into a mod folder would be wrong.
GAME_DATA_MARKERS = ("GFXTILES.BC", "gfxtiles.bc", "COUNT.EXE", "count.exe",
                     "BCCD.EXE", "bccd.exe")


def looks_like_game_data(directory: Path | str) -> bool:
    """True when this directory appears to hold the user's copy of the game."""
    directory = Path(directory)
    if not directory.is_dir():
        return False
    return any(next(directory.glob(pattern), None) is not None for pattern in GAME_DATA_MARKERS)


@dataclass(frozen=True)
class SourceIdentity:
    """What was true about a source file when it was imported."""

    path: Path
    size: int
    modified: float
    digest: str
    is_symlink: bool
    resolved: Path
    writable: bool

    @classmethod
    def probe(cls, path: Path | str) -> "SourceIdentity":
        original = Path(path).expanduser()
        resolved = canonical(original)
        status = resolved.stat()
        return cls(
            path=original,
            size=status.st_size,
            modified=status.st_mtime,
            digest=digest_file(resolved),
            is_symlink=original.is_symlink(),
            resolved=resolved,
            writable=os.access(resolved, os.W_OK),
        )

    def verify_unchanged(self) -> None:
        """Stop the line if the source moved under us.

        Called after any operation that referenced the source. If this fires,
        something wrote to data it had no business writing to, and continuing
        would only spread the damage.
        """
        if not self.resolved.exists():
            raise export_error(
                "OBC-SOURCE-001", f"source {self.resolved} no longer exists", str(self.resolved)
            )
        current = digest_file(self.resolved)
        if current != self.digest:
            raise export_error(
                "OBC-SOURCE-001",
                f"source changed during the operation: {self.digest[:12]} -> {current[:12]}",
                str(self.resolved),
            )


@dataclass
class OutputGuard:
    """Everything an export is forbidden to write."""

    protected_roots: tuple[Path, ...] = ()
    protected_files: tuple[Path, ...] = ()

    @classmethod
    def for_source(cls, source: Path | str, *, extra_roots=()) -> "OutputGuard":
        """Protect the source, and its directory when that is game data.

        The source file itself is always protected, however it is spelled. Its
        *directory* is protected only when it looks like a game data
        directory, because that is where the accident actually lives: an export
        that lands beside MAPTEMP.BC is one typo away from being MAPTEMP.BC.

        Protecting every source's parent unconditionally was the first version
        of this, and it was wrong -- it refused to write a project file into
        the user's own working directory just because a scratch archive
        happened to be there too. Callers who want a directory protected for
        another reason pass it in `extra_roots`.
        """
        resolved = canonical(source)
        roots = tuple(canonical(root) for root in extra_roots)
        if looks_like_game_data(resolved.parent):
            roots += (resolved.parent,)
        return cls(protected_roots=roots, protected_files=(resolved,))

    def check(self, output: Path | str) -> Path:
        """Canonicalise an output path, or refuse it. Returns the safe path."""
        target = canonical(output)

        for protected in self.protected_files:
            if target == protected or (target.exists() and same_file(target, protected)):
                raise export_error(
                    "OBC-SOURCE-002",
                    f"output {target} is the protected source {protected}",
                    str(target),
                )

        for root in self.protected_roots:
            if target == root or target.is_relative_to(root):
                raise export_error(
                    "OBC-EXPORT-001",
                    f"output {target} is inside the protected directory {root}",
                    str(target),
                )
        return target


def atomic_write(path: Path | str, data: bytes, *, guard: OutputGuard | None = None) -> Path:
    """Write, then read back and compare before reporting success.

    The replace is atomic, so a crash leaves either the old file or the new one
    and never a half-written map. The readback is the part that matters to a
    user: an export that returns quietly and left something different on disk
    is worse than one that fails.
    """
    target = guard.check(path) if guard else canonical(path)
    target.parent.mkdir(parents=True, exist_ok=True)

    handle, temporary_name = tempfile.mkstemp(
        dir=target.parent, prefix=f".{target.name}.", suffix=".part"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(handle, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, target)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise

    written = target.read_bytes()
    if written != data:
        raise export_error(
            "OBC-EXPORT-002",
            f"readback differs: wrote {len(data)} bytes, read {len(written)}",
            str(target),
        )
    return target
