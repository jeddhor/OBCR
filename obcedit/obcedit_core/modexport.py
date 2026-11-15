# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Exporting a mod, and auditing one by reading it back.

A mod (docs/MODS_AND_OBCEDIT.md section 3) is a folder, or the same folder as
a ZIP named `.obcmod`, holding:

    mod.toml              the manifest (campaign.py writes it)
    MAPHEAD.BC            the mod's maps, Operation Body Count's own format
    MAPTEMP.BC            (archive.py writes both)
    README.txt            optional

`build_mod` turns a project and its campaign into those files in memory;
`write_mod` puts them on disk through the path guard, so an export can never
land on the user's game data; `audit_mod` answers "what is in this mod" by
reading the files, never by remembering what was written, with the same
limits OBCR applies to a mod it is offered (ZIP entries, sizes, manifest keys,
maps). OBCR's own `--check-mod` remains the final gate (engine_runner.py).
"""

from __future__ import annotations

import hashlib
import io
import stat
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

from . import campaign as campaign_module
from .archive import MAPHEAD_NAME, MAPTEMP_NAME, MapRecord, encode_archive, parse_archive
from .campaign import Campaign, LevelEntry, render_manifest, validate
from .errors import Diagnostic, Severity, export_error
from .paths import OutputGuard, atomic_write

MANIFEST_NAME = "mod.toml"
README_NAME = "README.txt"
KNOWN_FILES = (MANIFEST_NAME, MAPHEAD_NAME, MAPTEMP_NAME, README_NAME)
MOD_SUFFIX = ".obcmod"
#: OBCR's limits for an archive it is offered (docs/MODS_AND_OBCEDIT.md 3.3).
MAX_ENTRY_BYTES = 16 << 20
MAX_TOTAL_BYTES = 64 << 20


@dataclass
class ModBuild:
    """A mod in memory: its files, and how its maps were numbered."""

    files: dict[str, bytes]
    campaign: Campaign
    #: Project map uuid -> slot in the mod's archive.
    slots: dict[str, int] = field(default_factory=dict)

    @property
    def digest(self) -> str:
        sha = hashlib.sha256()
        for name in sorted(self.files):
            sha.update(name.encode("ascii") + b"\0" + self.files[name])
        return sha.hexdigest()


def build_mod(project, campaign: Campaign | None = None, *, sharing: bool = True) -> ModBuild:
    """The mod files for `project` (its campaign, or every map in order).

    `sharing=False` allows maps imported from the original game (a private
    build for playtesting); a mod meant to be given to others refuses them.
    Raises ExportError with the first problem when the campaign is not valid.
    """
    campaign = campaign or (Campaign.from_json(project.campaign) if project.campaign
                            else Campaign.default_for(project))
    problems = [p for p in validate(campaign, project.maps, sharing=sharing)
                if p.severity is Severity.ERROR]
    if problems:
        first = problems[0]
        raise export_error(first.code, first.message, first.where)
    by_uuid = {document.uuid: document for document in project.maps}
    slots = {uuid: slot for slot, uuid in enumerate(campaign.map_order())}
    files: dict[str, bytes] = {MANIFEST_NAME: render_manifest(campaign).encode("ascii")}
    if slots:
        records = [MapRecord(slot, by_uuid[uuid].native_name, by_uuid[uuid].planes)
                   for uuid, slot in slots.items()]
        files[MAPHEAD_NAME], files[MAPTEMP_NAME] = encode_archive(records)
    if campaign.readme.strip():
        files[README_NAME] = campaign.readme.replace("\r\n", "\n").encode("utf-8")
    return ModBuild(files, campaign, slots)


def preview_mod(project, map_uuid: str, *, like: int | None = None) -> ModBuild:
    """A one-floor private mod of one map, for playtests and snapshots.

    Uses that map's level settings from the project's campaign when it has
    some, so the playtest looks like the finished mod.
    """
    document = next((d for d in project.maps if d.uuid == map_uuid), None)
    if document is None:
        raise export_error("OBC-MOD-006", f"no map {map_uuid} in the project")
    campaign = Campaign.from_json(project.campaign) if project.campaign else Campaign()
    level = next((lv for lv in campaign.levels if lv.map_uuid == map_uuid), None)
    level = level or LevelEntry(map_uuid=map_uuid)
    if like is not None:
        from dataclasses import replace

        level = replace(level, like=like)
    preview = Campaign(mod_id="obcedit-preview", title="OBCEdit playtest", version="1.0",
                       levels=(level,))
    return build_mod(project, preview, sharing=False)


def write_mod(build: ModBuild, destination: Path | str, *,
              guard: OutputGuard | None = None) -> Path:
    """Write the mod as a folder, or as a ZIP when `destination` ends in `.obcmod`."""
    destination = Path(destination)
    if destination.suffix.lower() == MOD_SUFFIX:
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name in sorted(build.files):
                info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                archive.writestr(info, build.files[name])
        return atomic_write(destination, buffer.getvalue(), guard=guard)
    destination.mkdir(parents=True, exist_ok=True)
    for name in KNOWN_FILES:
        stale = destination / name
        if name not in build.files and stale.is_file():
            stale.unlink()
    for name, data in sorted(build.files.items()):
        atomic_write(destination / name, data, guard=guard)
    return destination


# ---------------------------------------------------------------------------
# Audit
# ---------------------------------------------------------------------------


@dataclass
class ModAudit:
    """What a mod holds, and what is wrong with it."""

    path: Path
    files: dict[str, int] = field(default_factory=dict)
    manifest: dict = field(default_factory=dict)
    campaign: Campaign | None = None
    maps: dict[int, str] = field(default_factory=dict)
    problems: list[Diagnostic] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not any(p.severity is Severity.ERROR for p in self.problems)

    def add(self, code: str, severity: Severity, message: str, where: str = "") -> None:
        self.problems.append(Diagnostic(code, severity, message, where))


def _safe_name(name: str) -> bool:
    if not name or name.startswith(("/", "\\")) or ":" in name:
        return False
    parts = name.replace("\\", "/").split("/")
    return all(part not in ("", ".", "..") for part in parts)


def _read_files(path: Path, audit: ModAudit) -> dict[str, bytes]:
    """The mod's files by upper-case name, applying OBCR's archive limits."""
    out: dict[str, bytes] = {}
    if path.is_dir():
        for entry in sorted(path.iterdir()):
            if entry.is_file():
                size = entry.stat().st_size
                audit.files[entry.name] = size
                if entry.name.upper() in (n.upper() for n in KNOWN_FILES):
                    if size > MAX_ENTRY_BYTES:
                        audit.add("OBC-MOD-012", Severity.ERROR,
                                  f"{entry.name} is {size} bytes; the limit is {MAX_ENTRY_BYTES}")
                        continue
                    out[entry.name.upper()] = entry.read_bytes()
        return out
    try:
        archive = zipfile.ZipFile(path)
    except (zipfile.BadZipFile, OSError) as error:
        audit.add("OBC-MOD-012", Severity.ERROR, f"not a readable .obcmod (ZIP): {error}")
        return out
    total = 0
    with archive:
        for info in archive.infolist():
            name = info.filename
            audit.files[name] = info.file_size
            if not _safe_name(name):
                audit.add("OBC-MOD-012", Severity.ERROR,
                          f"entry {name!r} points outside the mod; OBCR refuses the archive")
                continue
            if info.flag_bits & 0x1:
                audit.add("OBC-MOD-012", Severity.ERROR, f"{name} is encrypted")
                continue
            if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                audit.add("OBC-MOD-012", Severity.ERROR,
                          f"{name} uses compression method {info.compress_type}; OBCR reads "
                          "stored and deflated entries only")
                continue
            if info.file_size > MAX_ENTRY_BYTES:
                audit.add("OBC-MOD-012", Severity.ERROR,
                          f"{name} is {info.file_size} bytes; the limit is {MAX_ENTRY_BYTES}")
                continue
            total += info.file_size
            if total > MAX_TOTAL_BYTES:
                audit.add("OBC-MOD-012", Severity.ERROR,
                          f"the mod unpacks to more than {MAX_TOTAL_BYTES} bytes")
                break
            if "/" in name.replace("\\", "/") or info.is_dir():
                continue
            if name.upper() in (n.upper() for n in KNOWN_FILES):
                out[name.upper()] = archive.read(info)
    return out


def audit_mod(path: Path | str, *, catalog=None) -> ModAudit:
    """Read a mod folder or `.obcmod` back and report everything about it."""
    from .document import MapDocument
    from .validation import validate_map

    path = Path(path)
    audit = ModAudit(path)
    if not path.exists():
        audit.add("OBC-MOD-012", Severity.ERROR, f"{path} does not exist")
        return audit
    files = _read_files(path, audit)
    for name in audit.files:
        if name.upper() not in (n.upper() for n in KNOWN_FILES):
            audit.add("OBC-MOD-013", Severity.INFORMATION, f"{name} is not part of a mod; "
                      "OBCR ignores it")
    text = files.get(MANIFEST_NAME.upper())
    if text is None:
        audit.add("OBC-MOD-010", Severity.ERROR, "the mod has no mod.toml")
        return audit
    try:
        decoded = text.decode("utf-8")
    except UnicodeDecodeError:
        audit.add("OBC-MOD-010", Severity.ERROR, "mod.toml is not UTF-8 text")
        return audit
    manifest, problems = campaign_module.parse_manifest(decoded)
    audit.manifest = manifest
    audit.problems.extend(problems)
    if not manifest:
        return audit
    audit.campaign = campaign_module.campaign_from_manifest(manifest)
    stock_free = all(level.stock is not None for level in audit.campaign.levels)

    archive = None
    head, temp = files.get(MAPHEAD_NAME.upper()), files.get(MAPTEMP_NAME.upper())
    if head is None or temp is None:
        if not stock_free:
            audit.add("OBC-MOD-014", Severity.ERROR,
                      "the mod has map floors but no MAPHEAD.BC/MAPTEMP.BC")
    else:
        try:
            archive = parse_archive(head, temp)
            audit.maps = {record.number: record.name.text for record in archive}
        except Exception as error:  # codec errors carry diagnostics; report them all alike
            audit.add("OBC-MOD-014", Severity.ERROR, f"the maps do not decode: {error}")

    # The manifest's values, checked with the editor's own rules (map uuids
    # here are "slot:N"; check them against the archive instead).
    level_rules = [p for p in validate(audit.campaign, (), sharing=False)
                   if p.code != "OBC-MOD-006"]
    audit.problems.extend(level_rules)
    for index, level in enumerate(audit.campaign.levels):
        if level.map_uuid:
            slot = int(level.map_uuid.split(":", 1)[1])
            if slot not in audit.maps:
                audit.add("OBC-MOD-006", Severity.ERROR,
                          f"floor {index + 1} uses map {slot}, which the archive does not hold",
                          f"floor {index + 1}")
    if archive is not None:
        used = {int(l.map_uuid.split(":", 1)[1]) for l in audit.campaign.levels if l.map_uuid}
        for record in archive:
            if record.number not in used:
                continue
            document = MapDocument.from_record(record)
            for problem in validate_map(document, catalog):
                if problem.severity is Severity.ERROR:
                    audit.problems.append(Diagnostic(
                        problem.code, problem.severity,
                        f"map {record.number} ({record.name.text}): {problem.message}",
                        problem.where, problem.cell))
    return audit


__all__ = ["KNOWN_FILES", "MOD_SUFFIX", "ModAudit", "ModBuild", "audit_mod", "build_mod",
           "preview_mod", "write_mod"]
