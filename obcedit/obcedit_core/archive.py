# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Operation Body Count's map archive: `MAPHEAD.BC` plus `MAPTEMP.BC`.

Wolfenstein 3D's split layout, RLEW only (no Carmack layer). The authority is
the engine's reader, `crates/obc-formats/src/maps.rs`; the layout below was
also checked by parsing all 61 maps of both releases (docs/re/DATA_FORMATS.md
section 3).

`MAPHEAD.BC`, 402 bytes::

    0   u16       RLEW tag (0xABCD)
    2   i32[100]  offset of each map's header in MAPTEMP.BC; -1 = empty slot

`MAPTEMP.BC`::

    0   char[8]   "TED5v1.0"
    then, for every used slot (in slot order, except that the original
    stores slot 56 last; a re-encode keeps each map's original place):
        header (38 bytes):
          0   i32[3]   absolute offsets of planes 0, 1, 2
          12  u16[3]   stored length of each plane
          18  u16      width   (always 64)
          20  u16      height  (always 64)
          22  char[16] name, NUL-terminated, with whatever followed it kept
        plane 0, plane 1, plane 2   (u16 expanded byte length, then RLEW words)
        "!ID!"

The canonical writer produces exactly that, and re-encoding the original
archive of either release is byte-identical (`verify_archive`, run against the
user's own files; never stored). The engine itself only follows offsets, so
the reader accepts any layout it would, and notes what the writer would not
produce.

Slots are **0-based**, as the engine's `mapon`: slot 0 is SEWER LEVEL 1.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass, field, replace
from pathlib import Path

from .errors import DiagnosticLog, native_error
from .names import NAME_FIELD_BYTES, NativeName
from .planes import PLANE_COUNT, MapPlanes, validate_dimensions
from .rlew import RLEW_TAG, decode_plane, encode_plane

SIGNATURE = b"TED5v1.0"
MAP_MARKER = b"!ID!"
HEADER_BYTES = 38
#: `MAPHEAD` has room for exactly this many slots.
MAX_MAPS = 100
MAPHEAD_BYTES = 2 + 4 * MAX_MAPS

MAPHEAD_NAME = "MAPHEAD.BC"
MAPTEMP_NAME = "MAPTEMP.BC"

_I32_MAX = 0x7FFFFFFF


@dataclass(frozen=True)
class RecordSource:
    """Where a record's bytes were, exactly as the files stated them."""

    header_offset: int
    plane_offsets: tuple[int, int, int]
    plane_lengths: tuple[int, int, int]


@dataclass(frozen=True)
class MapRecord:
    """One map: its slot, its 16 raw name bytes, and its three planes."""

    number: int  # the 0-based slot (the engine's mapon)
    name: NativeName
    planes: MapPlanes
    source: RecordSource | None = None

    @property
    def slot(self) -> int:
        return self.number

    @property
    def label(self) -> str:
        """`slot 00`: how the editor names a slot (the floor is slot + 1)."""
        return f"slot {self.number:02d}"

    # The name EC7Edit's GUI shows in its map list.
    lump_name = label

    @property
    def width(self) -> int:
        return self.planes.width

    @property
    def height(self) -> int:
        return self.planes.height


@dataclass
class Archive:
    """A parsed archive plus everything noticed while parsing it."""

    records: tuple[MapRecord, ...]
    diagnostics: DiagnosticLog = field(default_factory=DiagnosticLog)
    rlew_tag: int = RLEW_TAG

    def __len__(self) -> int:
        return len(self.records)

    def __iter__(self):
        return iter(self.records)

    def __getitem__(self, index: int) -> MapRecord:
        return self.records[index]

    def by_number(self, number: int) -> MapRecord:
        for record in self.records:
            if record.number == number:
                return record
        raise native_error(
            "OBC-NATIVE-001",
            f"archive has no map in slot {number} (it holds {len(self.records)} maps)",
        )

    by_slot = by_number


def _u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def parse_archive(
    maphead: bytes, maptemp: bytes, *, log: DiagnosticLog | None = None
) -> Archive:
    """Parse the two files. Raises on anything the engine could not load."""
    diagnostics = log if log is not None else DiagnosticLog()
    if len(maphead) < MAPHEAD_BYTES:
        raise native_error(
            "OBC-NATIVE-001",
            f"MAPHEAD is {len(maphead)} bytes; it needs {MAPHEAD_BYTES} "
            "(the RLEW tag and 100 offsets)",
        )
    if len(maphead) > MAPHEAD_BYTES:
        diagnostics.information(
            "OBC-NATIVE-008",
            f"MAPHEAD has {len(maphead) - MAPHEAD_BYTES} bytes after its 100 offsets; "
            "the engine ignores them and the canonical writer emits none",
        )
    tag = _u16(maphead, 0)
    if tag != RLEW_TAG:
        diagnostics.information(
            "OBC-NATIVE-009", f"RLEW tag is 0x{tag:04X}, not the usual 0x{RLEW_TAG:04X}"
        )
    offsets = struct.unpack_from(f"<{MAX_MAPS}i", maphead, 2)
    if maptemp[: len(SIGNATURE)] != SIGNATURE:
        diagnostics.warning(
            "OBC-NATIVE-010",
            "MAPTEMP does not start with 'TED5v1.0'; the engine does not check, "
            "the canonical writer always adds it",
        )

    records = []
    for slot, offset in enumerate(offsets):
        if offset <= 0:
            continue
        where = f"slot {slot}"
        if offset + HEADER_BYTES > len(maptemp):
            raise native_error(
                "OBC-NATIVE-001",
                f"header at 0x{offset:x} runs past the end of MAPTEMP ({len(maptemp)} bytes)",
                where,
            )
        starts = struct.unpack_from("<3i", maptemp, offset)
        lengths = struct.unpack_from("<3H", maptemp, offset + 12)
        width, height = struct.unpack_from("<HH", maptemp, offset + 18)
        validate_dimensions(width, height, where=where)
        raw_name = maptemp[offset + 22 : offset + 22 + NAME_FIELD_BYTES]
        name = NativeName.from_raw(raw_name)
        name.report(diagnostics, where)
        planes = []
        for plane in range(PLANE_COUNT):
            start, length = starts[plane], lengths[plane]
            if start < 0 or start + length > len(maptemp):
                raise native_error(
                    "OBC-NATIVE-001",
                    f"plane {plane} at 0x{start:x}+{length} lies outside MAPTEMP",
                    where,
                )
            planes.append(
                decode_plane(
                    maptemp[start : start + length],
                    width * height,
                    tag=tag,
                    where=f"{where} plane {plane}",
                    log=diagnostics,
                )
            )
        records.append(
            MapRecord(
                slot,
                name,
                MapPlanes(width, height, tuple(planes)),  # type: ignore[arg-type]
                RecordSource(offset, tuple(starts), tuple(lengths)),  # type: ignore[arg-type]
            )
        )
    if not records:
        raise native_error("OBC-NATIVE-001", "MAPHEAD lists no maps")
    return Archive(tuple(records), diagnostics, tag)


def encode_archive(records, *, tag: int = RLEW_TAG) -> tuple[bytes, bytes]:
    """Write a canonical `(MAPHEAD.BC, MAPTEMP.BC)` pair. Deterministic."""
    # Maps read from a file keep their place in it (the original stores slot
    # 56 last); new ones follow, by slot. That keeps a re-encode identical.
    records = sorted(
        records,
        key=lambda r: (0, r.source.header_offset, r.number) if r.source else (1, 0, r.number),
    )
    if not 1 <= len(records) <= MAX_MAPS:
        raise native_error("OBC-NATIVE-001", f"an archive holds 1..{MAX_MAPS} maps, not {len(records)}")
    seen: set[int] = set()
    offsets = [-1] * MAX_MAPS
    out = bytearray(SIGNATURE)
    for record in records:
        slot = record.number
        where = f"slot {slot}"
        if not 0 <= slot < MAX_MAPS:
            raise native_error("OBC-NATIVE-001", f"slot {slot} is outside 0..{MAX_MAPS - 1}", where)
        if slot in seen:
            raise native_error("OBC-NATIVE-001", f"two maps in slot {slot}", where)
        seen.add(slot)
        planes = record.planes
        validate_dimensions(planes.width, planes.height, where=where)
        raw_name = record.name.raw
        if len(raw_name) != NAME_FIELD_BYTES:
            raise native_error("OBC-NATIVE-004", f"name field is {len(raw_name)} bytes", where)
        streams = [
            encode_plane(planes.planes[p], tag=tag, where=f"{where} plane {p}")
            for p in range(PLANE_COUNT)
        ]
        header = len(out)
        offsets[slot] = header
        starts = []
        position = header + HEADER_BYTES
        for stream in streams:
            starts.append(position)
            position += len(stream)
        if position > _I32_MAX:
            raise native_error("OBC-NATIVE-001", "archive exceeds the 32-bit offset space", where)
        out.extend(struct.pack("<3i", *starts))
        out.extend(struct.pack("<3H", *(len(s) for s in streams)))
        out.extend(struct.pack("<HH", planes.width, planes.height))
        out.extend(raw_name)
        for stream in streams:
            out.extend(stream)
        out.extend(MAP_MARKER)
    maphead = struct.pack("<H", tag) + struct.pack(f"<{MAX_MAPS}i", *offsets)
    return maphead, bytes(out)


def _find(directory: Path, name: str) -> Path:
    """`name` in `directory`, in any letter case."""
    direct = directory / name
    if direct.is_file():
        return direct
    lowered = name.lower()
    for entry in directory.iterdir() if directory.is_dir() else ():
        if entry.name.lower() == lowered and entry.is_file():
            return entry
    raise native_error("OBC-SOURCE-001", f"{name} not found in {directory}")


def archive_paths(path: Path | str) -> tuple[Path, Path]:
    """`(MAPHEAD, MAPTEMP)` for a folder, or for either file of the pair."""
    path = Path(path)
    directory = path if path.is_dir() else path.parent
    return _find(directory, MAPHEAD_NAME), _find(directory, MAPTEMP_NAME)


def read_archive(path: Path | str, *, log: DiagnosticLog | None = None) -> Archive:
    """Parse an archive from disk (a folder, or either of its two files). Read-only."""
    head, temp = archive_paths(path)
    return parse_archive(head.read_bytes(), temp.read_bytes(), log=log)


def replace_records(archive: Archive, replacements) -> tuple[bytes, bytes]:
    """Re-encode an archive with some slots replaced and the rest untouched.

    `replacements` maps a slot to the MapRecord to put there. A slot the
    archive does not hold is an error rather than an addition.
    """
    numbers = {record.number for record in archive.records}
    unknown = sorted(set(replacements) - numbers)
    if unknown:
        raise native_error("OBC-NATIVE-001", f"this archive has no map in slot {unknown[0]}")
    out = []
    for record in archive.records:
        wanted = replacements.get(record.number)
        out.append(
            replace(wanted, number=record.number, source=record.source) if wanted else record
        )
    return encode_archive(out, tag=archive.rlew_tag)


@dataclass(frozen=True)
class VerifyResult:
    """What `verify_archive` found."""

    maps: int
    maphead_identical: bool
    maptemp_identical: bool
    first_difference: int | None

    @property
    def identical(self) -> bool:
        return self.maphead_identical and self.maptemp_identical


def verify_archive(path: Path | str) -> VerifyResult:
    """Decode the archive at `path`, re-encode it canonically, and compare bytes."""
    head_path, temp_path = archive_paths(path)
    head, temp = head_path.read_bytes(), temp_path.read_bytes()
    archive = parse_archive(head, temp)
    new_head, new_temp = encode_archive(archive.records, tag=archive.rlew_tag)
    first = None
    if new_temp != temp:
        first = next(
            (i for i, (a, b) in enumerate(zip(new_temp, temp)) if a != b),
            min(len(new_temp), len(temp)),
        )
    return VerifyResult(len(archive), new_head == head[: len(new_head)] and len(head) == len(new_head),
                        new_temp == temp, first)
