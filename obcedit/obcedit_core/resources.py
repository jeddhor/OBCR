# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""A project's own art, sounds and music: what replaces what, and where the bytes are.

Mod format 2 (docs/MOD_ASSETS_AND_ARENAS.md section 2) lets a mod replace any
of the game's 136 wall pages and 680 sprites with a 64x64 PNG, any of its 99
digitized sounds with a WAV, and lets a floor play a music file the mod brings.
This module is the project's half of that:

* `Resource` is one record: *kind* (wall, sprite, sound, music), *target*
  (the page, sprite or sound number, or a music file's name), and the SHA-256
  of the author's file. The project file holds records only.
* `ResourceStore` holds the bytes, content-addressed, in a folder beside the
  project (`<name>.obcproject` keeps them in `<name>.assets/`), or in the
  workspace for a project that has not been saved yet.

Content-addressed rather than laid out like the mod: an import, a replace and
an undo are then three records pointing at files that all still exist, and an
undo can never find that the file it needs was overwritten by the import it is
undoing. The mod's own layout (`walls/012.png`) is produced at export
(`mod_path`).

A shared project is untrusted input: a record names a digest and a number, never
a path, and the store's folder comes from where the project file is, not from
anything written in it.
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .errors import Diagnostic, ObcEditError, Severity

WALL, SPRITE, SOUND, MUSIC = "wall", "sprite", "sound", "music"
KINDS = (WALL, SPRITE, SOUND, MUSIC)
#: Wall pages 0-135, sprites 0-679, digitized sounds 0-98 (section 2.1).
WALL_PAGES = 136
SPRITES = 680
SOUNDS = 99
#: The mod's folder for each kind.
FOLDERS = {WALL: "walls", SPRITE: "sprites", SOUND: "sounds", MUSIC: "music"}
MUSIC_EXTENSIONS = ("ogg", "qoa", "imf")
#: Section 2.3.
MAX_IMAGE_BYTES = 256 << 10
MAX_SOUND_BYTES = 4 << 20
MAX_SOUND_SECONDS = 30
MAX_MUSIC_BYTES = 16 << 20
MAX_MUSIC_FILES = 64

_MUSIC_NAME = re.compile(r"\A[A-Za-z0-9_-]{1,32}\.(ogg|qoa|imf)\Z", re.IGNORECASE)
_DIGEST = re.compile(r"\A[0-9a-f]{64}\Z")
_PRINTABLE = re.compile(r"\A[\x20-\x7e]*\Z")
#: Asset paths inside a mod, as section 2.1 spells them (case-insensitive).
_ASSET_PATH = {
    WALL: re.compile(r"\Awalls/(\d{1,3})\.png\Z", re.IGNORECASE),
    SPRITE: re.compile(r"\Asprites/(\d{1,3})\.png\Z", re.IGNORECASE),
    SOUND: re.compile(r"\Asounds/(\d{1,2})\.wav\Z", re.IGNORECASE),
    MUSIC: re.compile(r"\Amusic/([A-Za-z0-9_-]{1,32}\.(?:ogg|qoa|imf))\Z", re.IGNORECASE),
}
_LIMIT = {WALL: WALL_PAGES, SPRITE: SPRITES, SOUND: SOUNDS}


class ResourceError(ObcEditError):
    """A resource record or file does not hold together. Says which one."""


def _error(code: str, message: str, where: str = "") -> ResourceError:
    return ResourceError(Diagnostic(code, Severity.ERROR, message, where))


def is_music_name(name: str) -> bool:
    return bool(_MUSIC_NAME.match(name or ""))


def extension_for(kind: str, target) -> str:
    if kind == WALL or kind == SPRITE:
        return "png"
    if kind == SOUND:
        return "wav"
    return str(target).rsplit(".", 1)[-1].lower()


def check_target(kind: str, target) -> None:
    """Refuse a record whose target is not something the engine numbers."""
    if kind not in KINDS:
        raise _error("OBC-RES-001", f"unknown resource kind {kind!r}")
    if kind == MUSIC:
        if not isinstance(target, str) or not is_music_name(target):
            raise _error("OBC-RES-001",
                         f"{target!r} is not a music file name: 1-32 characters of "
                         "A-Z a-z 0-9 _ - and .ogg, .qoa or .imf")
        return
    limit = _LIMIT[kind]
    if not isinstance(target, int) or isinstance(target, bool) or not 0 <= target < limit:
        raise _error("OBC-RES-001", f"{kind} {target!r} is outside 0..{limit - 1}")


def mod_path(kind: str, target) -> str:
    """Where a replacement lives in a mod: `walls/012.png`, `sounds/07.wav`..."""
    if kind == WALL:
        return f"walls/{target:03d}.png"
    if kind == SPRITE:
        return f"sprites/{target:03d}.png"
    if kind == SOUND:
        return f"sounds/{target:02d}.wav"
    return f"music/{target}"


def parse_mod_path(name: str):
    """`(kind, target)` for a file name inside a mod's asset folders.

    Returns None for a file outside the four folders, and raises for one
    inside them that does not follow section 2.1 -- a mistyped number must not
    be silently ignored.
    """
    normal = name.replace("\\", "/")
    folder = normal.split("/", 1)[0].lower()
    kinds = {value: key for key, value in FOLDERS.items()}
    if "/" not in normal or folder not in kinds:
        return None
    kind = kinds[folder]
    match = _ASSET_PATH[kind].match(normal)
    if not match:
        raise _error("OBC-MOD-016", f"{name} is not a {kind} file the engine reads "
                     f"(section 2.1: {_spelling(kind)})", name)
    if kind == MUSIC:
        return kind, match.group(1)
    number = int(match.group(1))
    if number >= _LIMIT[kind]:
        raise _error("OBC-MOD-016", f"{name}: {kind} {number} is outside 0..{_LIMIT[kind] - 1}",
                     name)
    return kind, number


def _spelling(kind: str) -> str:
    return {WALL: "walls/NNN.png, 0-135", SPRITE: "sprites/NNN.png, 0-679",
            SOUND: "sounds/NN.wav, 0-98", MUSIC: "music/NAME.ogg|qoa|imf"}[kind]


@dataclass(frozen=True)
class Resource:
    """One replacement or music file, by digest."""

    kind: str
    target: int | str
    sha256: str
    #: The file the author imported, for display only (inert text).
    source_name: str = ""
    size: int = 0

    @property
    def key(self) -> tuple[str, str]:
        return self.kind, str(self.target).lower() if self.kind == MUSIC else str(self.target)

    @property
    def extension(self) -> str:
        return extension_for(self.kind, self.target)

    @property
    def blob_name(self) -> str:
        return f"{self.sha256}.{self.extension}"

    @property
    def mod_path(self) -> str:
        return mod_path(self.kind, self.target)

    def to_json(self) -> dict:
        return {"kind": self.kind, "target": self.target, "sha256": self.sha256,
                "source_name": self.source_name, "size": self.size}

    @classmethod
    def from_json(cls, raw, where: str = "resources") -> "Resource":
        if not isinstance(raw, dict):
            raise _error("OBC-SCHEMA-002", "a resource is not an object", where)
        unknown = sorted(set(raw) - {"kind", "target", "sha256", "source_name", "size"})
        if unknown:
            raise _error("OBC-SCHEMA-002", f"unknown properties: {', '.join(unknown)}", where)
        kind, target = raw.get("kind"), raw.get("target")
        check_target(kind, target)
        digest = raw.get("sha256", "")
        if not isinstance(digest, str) or not _DIGEST.match(digest):
            raise _error("OBC-SCHEMA-002", "sha256 must be 64 lower-case hex digits", where)
        source = raw.get("source_name", "")
        if not isinstance(source, str) or len(source) > 255:
            raise _error("OBC-SCHEMA-002", "source_name must be text of at most 255 characters",
                         where)
        size = raw.get("size", 0)
        if not isinstance(size, int) or isinstance(size, bool) or size < 0:
            raise _error("OBC-SCHEMA-002", "size must be a whole number", where)
        return cls(kind, target, digest, source, size)


def sort_key(resource: Resource):
    order = KINDS.index(resource.kind)
    return (order, str(resource.target).lower() if resource.kind == MUSIC
            else f"{resource.target:04d}")


def validate_set(resources) -> list[Diagnostic]:
    """Problems with a project's set of records as a whole."""
    problems = []
    seen = set()
    for resource in resources:
        if resource.key in seen:
            problems.append(Diagnostic("OBC-RES-001", Severity.ERROR,
                                       f"two resources replace {resource.kind} {resource.target}"))
        seen.add(resource.key)
    music = sum(1 for r in resources if r.kind == MUSIC)
    if music > MAX_MUSIC_FILES:
        problems.append(Diagnostic("OBC-RES-001", Severity.ERROR,
                                   f"{music} music files; a mod holds at most {MAX_MUSIC_FILES}"))
    return problems


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class ResourceStore:
    """The bytes behind a project's records, one file per digest.

    Writes are atomic and idempotent (the same bytes are the same file), reads
    check the digest, so a file edited or damaged outside the editor is named
    rather than exported.
    """

    def __init__(self, root: Path | str) -> None:
        self.root = Path(root).expanduser().resolve()

    def __repr__(self) -> str:
        return f"ResourceStore({str(self.root)!r})"

    def path_of(self, resource: Resource) -> Path:
        return self.root / resource.blob_name

    def put(self, data: bytes, kind: str, target, source_name: str = "") -> Resource:
        """Store `data` and return the record that names it."""
        check_target(kind, target)
        resource = Resource(kind, target, digest(data), str(source_name)[:255], len(data))
        path = self.path_of(resource)
        if path.is_file() and path.stat().st_size == len(data):
            return resource
        self.root.mkdir(parents=True, exist_ok=True)
        handle, temporary = tempfile.mkstemp(dir=self.root, prefix=".put.", suffix=".part")
        try:
            with os.fdopen(handle, "wb") as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, path)
        except BaseException:
            Path(temporary).unlink(missing_ok=True)
            raise
        return resource

    def read(self, resource: Resource) -> bytes:
        path = self.path_of(resource)
        where = resource.source_name or resource.mod_path
        try:
            data = path.read_bytes()
        except OSError as error:
            raise _error("OBC-RES-002",
                         f"the file for {resource.kind} {resource.target} ({where}) is missing "
                         f"from {self.root}: {error.strerror or error}", where) from error
        if digest(data) != resource.sha256:
            raise _error("OBC-RES-003",
                         f"the file for {resource.kind} {resource.target} ({where}) changed "
                         "outside OBCEdit; its contents no longer match the project. Import it "
                         "again.", where)
        return data

    def check(self, resources) -> list[Diagnostic]:
        """One diagnostic per record whose file is missing or changed."""
        problems = []
        for resource in resources:
            try:
                self.read(resource)
            except ResourceError as error:
                problems.append(error.diagnostic)
        return problems

    def adopt(self, resources, other: "ResourceStore") -> int:
        """Copy the files `resources` need from `other`. Returns how many moved.

        Used when an unsaved project is saved (its files leave the workspace
        for the folder beside it) and by Save As and Save a Copy.
        """
        if other.root == self.root:
            return 0
        copied = 0
        for resource in resources:
            target = self.path_of(resource)
            if target.is_file():
                continue
            source = other.path_of(resource)
            if not source.is_file():
                continue
            self.root.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            copied += 1
        return copied

    def prune(self, keep) -> int:
        """Delete blob files no record in `keep` names. Only files this store owns."""
        if not self.root.is_dir():
            return 0
        wanted = {resource.blob_name for resource in keep}
        removed = 0
        for path in self.root.iterdir():
            name = path.name
            if path.is_file() and name not in wanted and _DIGEST.match(name.split(".", 1)[0]) \
                    and name.rsplit(".", 1)[-1] in ("png", "wav", *MUSIC_EXTENSIONS):
                path.unlink()
                removed += 1
        return removed

    def discard(self) -> None:
        """Remove the whole store (an unsaved project's, after it was saved)."""
        if self.root.is_dir():
            shutil.rmtree(self.root, ignore_errors=True)


def store_for(project_path: Path | str | None, *, workspace: Path | str,
              project_uuid: str) -> ResourceStore:
    """`<name>.assets/` beside a saved project; the workspace for an unsaved one."""
    if project_path:
        path = Path(project_path)
        return ResourceStore(path.with_name(path.stem + ".assets"))
    return ResourceStore(Path(workspace) / "unsaved" / project_uuid)


__all__ = [
    "FOLDERS", "KINDS", "MAX_IMAGE_BYTES", "MAX_MUSIC_BYTES", "MAX_MUSIC_FILES",
    "MAX_SOUND_BYTES", "MAX_SOUND_SECONDS", "MUSIC", "MUSIC_EXTENSIONS", "Resource",
    "ResourceError", "ResourceStore", "SOUND", "SOUNDS", "SPRITE", "SPRITES", "WALL",
    "WALL_PAGES", "check_target", "digest", "extension_for", "is_music_name", "mod_path",
    "parse_mod_path", "sort_key", "store_for", "validate_set",
]
