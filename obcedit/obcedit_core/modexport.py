# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Exporting a mod, and auditing one by reading it back.

A mod (docs/MODS_AND_OBCEDIT.md section 3) is a folder, or the same folder as
a ZIP named `.obcmod`, holding:

    mod.toml              the manifest (campaign.py writes it)
    MAPHEAD.BC            the mod's maps, Operation Body Count's own format
    MAPTEMP.BC            (archive.py writes both)
    README.txt            optional
    walls/ sprites/ sounds/ music/
                          format 2: the project's replacement art, sounds and
                          music (docs/MOD_ASSETS_AND_ARENAS.md section 2),
                          read from its ResourceStore

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
from .campaign import (
    MOD_FORMAT,
    MOD_FORMAT_ASSETS,
    Campaign,
    LevelEntry,
    music_file,
    render_manifest,
    validate,
)
from .errors import Diagnostic, ObcEditError, Severity, export_error
from .paths import OutputGuard, atomic_write
from .resources import FOLDERS, MUSIC, SOUND, SPRITE, WALL, ResourceError, parse_mod_path

MANIFEST_NAME = "mod.toml"
README_NAME = "README.txt"
KNOWN_FILES = (MANIFEST_NAME, MAPHEAD_NAME, MAPTEMP_NAME, README_NAME)
MOD_SUFFIX = ".obcmod"
#: OBCR's limits for an archive it is offered (docs/MOD_ASSETS_AND_ARENAS.md 2.3).
MAX_ENTRY_BYTES = 16 << 20
MAX_TOTAL_BYTES = 128 << 20
MAX_ENTRIES = 2000
ASSET_FOLDERS = tuple(FOLDERS.values())


@dataclass
class ModBuild:
    """A mod in memory: its files, and how its maps were numbered."""

    files: dict[str, bytes]
    campaign: Campaign
    #: Project map uuid -> slot in the mod's archive.
    slots: dict[str, int] = field(default_factory=dict)
    format: int = MOD_FORMAT

    @property
    def digest(self) -> str:
        sha = hashlib.sha256()
        for name in sorted(self.files):
            sha.update(name.encode("ascii") + b"\0" + self.files[name])
        return sha.hexdigest()


def asset_files(project, campaign: Campaign, store) -> dict[str, bytes]:
    """The mod's asset folders: every replacement, and the music it plays.

    A music file nothing plays is left out (OBCR would only warn about it).
    Raises ExportError naming a resource whose file is missing or changed.
    """
    resources = getattr(project, "resources", ())
    if not resources:
        return {}
    if store is None:
        raise export_error("OBC-RES-002", "this project has replacement art, sounds or music, "
                           "but no folder to read them from")
    played = {name.lower() for name in campaign.music_files()}
    files: dict[str, bytes] = {}
    for resource in resources:
        if resource.kind == MUSIC and str(resource.target).lower() not in played:
            continue
        try:
            files[resource.mod_path] = store.read(resource)
        except ResourceError as error:
            raise export_error(error.diagnostic.code, error.diagnostic.message,
                               error.diagnostic.where) from error
    return files


def _check_limits(files: dict[str, bytes]) -> None:
    if len(files) > MAX_ENTRIES:
        raise export_error("OBC-MOD-012", f"the mod has {len(files)} files; OBCR reads at most "
                           f"{MAX_ENTRIES}")
    total = 0
    for name, data in files.items():
        if len(data) > MAX_ENTRY_BYTES:
            raise export_error("OBC-MOD-012", f"{name} is {len(data)} bytes; the limit is "
                               f"{MAX_ENTRY_BYTES}", name)
        total += len(data)
    if total > MAX_TOTAL_BYTES:
        raise export_error("OBC-MOD-012", f"the mod is {total} bytes; OBCR reads at most "
                           f"{MAX_TOTAL_BYTES}")


def build_mod(project, campaign: Campaign | None = None, *, sharing: bool = True,
              store=None) -> ModBuild:
    """The mod files for `project` (its campaign, or every map in order).

    `sharing=False` allows maps imported from the original game (a private
    build for playtesting); a mod meant to be given to others refuses them.
    `store` is the project's ResourceStore, needed when it has resources.
    Raises ExportError with the first problem when the campaign is not valid.
    The manifest is format 2 only when the mod uses something format 1 lacks.
    """
    campaign = campaign or (Campaign.from_json(project.campaign) if project.campaign
                            else Campaign.default_for(project))
    music_names = {str(r.target) for r in getattr(project, "resources", ()) if r.kind == MUSIC}
    problems = [p for p in validate(campaign, project.maps, sharing=sharing,
                                    music_names=music_names)
                if p.severity is Severity.ERROR]
    if problems:
        first = problems[0]
        raise export_error(first.code, first.message, first.where)
    by_uuid = {document.uuid: document for document in project.maps}
    slots = {uuid: slot for slot, uuid in enumerate(campaign.map_order())}
    assets = asset_files(project, campaign, store)
    format = MOD_FORMAT_ASSETS if (assets or campaign.needs_format_2()) else MOD_FORMAT
    files: dict[str, bytes] = {
        MANIFEST_NAME: render_manifest(campaign, format=format).encode("ascii")}
    if slots:
        records = [MapRecord(slot, by_uuid[uuid].native_name, by_uuid[uuid].planes)
                   for uuid, slot in slots.items()]
        files[MAPHEAD_NAME], files[MAPTEMP_NAME] = encode_archive(records)
    if campaign.readme.strip():
        files[README_NAME] = campaign.readme.replace("\r\n", "\n").encode("utf-8")
    files.update(assets)
    _check_limits(files)
    return ModBuild(files, campaign, slots, format)


def preview_mod(project, map_uuid: str, *, like: int | None = None, store=None) -> ModBuild:
    """A one-floor private mod of one map, for playtests and snapshots.

    Uses that map's level settings from the project's campaign when it has
    some, so the playtest looks like the finished mod -- including its
    replacement art, sounds and music (`store`, the project's ResourceStore).
    A map used only as an arena plays as a floor with the arena's music.
    """
    document = next((d for d in project.maps if d.uuid == map_uuid), None)
    if document is None:
        raise export_error("OBC-MOD-006", f"no map {map_uuid} in the project")
    campaign = Campaign.from_json(project.campaign) if project.campaign else Campaign()
    level = next((lv for lv in campaign.levels if lv.map_uuid == map_uuid), None)
    if level is None:
        arena = next((a for a in campaign.arenas if a.map_uuid == map_uuid), None)
        level = LevelEntry(map_uuid=map_uuid,
                           music=arena.music if arena is not None else None)
    if like is not None:
        from dataclasses import replace

        level = replace(level, like=like)
    preview = Campaign(mod_id="obcedit-preview", title="OBCEdit playtest", version="1.0",
                       levels=(level,))
    return build_mod(project, preview, sharing=False, store=store)


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
    # Replacements a previous export wrote and this one no longer has: only
    # files spelled like assets, in the asset folders, are ever removed.
    wanted = {name.lower() for name in build.files}
    for folder in ASSET_FOLDERS:
        directory = destination / folder
        if not directory.is_dir():
            continue
        for entry in directory.iterdir():
            relative = f"{folder}/{entry.name}"
            if not entry.is_file() or relative.lower() in wanted:
                continue
            try:
                if parse_mod_path(relative) is not None:
                    entry.unlink()
            except ResourceError:
                pass
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
    #: Asset file name -> (kind, target), for format-2 mods.
    assets: dict[str, tuple] = field(default_factory=dict)

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


def _is_asset_folder(name: str) -> bool:
    return name.lower() in ASSET_FOLDERS


def _read_files(path: Path, audit: ModAudit,
                assets: dict[str, bytes] | None = None) -> dict[str, bytes]:
    """The mod's files by upper-case name, applying OBCR's archive limits.

    Files in the asset folders go to `assets`, by their name in the mod.
    """
    out: dict[str, bytes] = {}
    assets = {} if assets is None else assets
    if path.is_dir():
        total = 0
        for entry in sorted(path.iterdir()):
            if entry.is_dir() and _is_asset_folder(entry.name):
                for child in sorted(entry.iterdir()):
                    relative = f"{entry.name}/{child.name}"
                    if not child.is_file():
                        audit.add("OBC-MOD-016", Severity.ERROR,
                                  f"{relative} is a folder; the asset folders hold files only")
                        continue
                    size = child.stat().st_size
                    audit.files[relative] = size
                    if size > MAX_ENTRY_BYTES:
                        audit.add("OBC-MOD-012", Severity.ERROR,
                                  f"{relative} is {size} bytes; the limit is {MAX_ENTRY_BYTES}")
                        continue
                    total += size
                    assets[relative] = child.read_bytes()
            elif entry.is_dir():
                # Not read, only reported, as OBCR does (bounded: a mod with a
                # huge unrelated tree is reported, not walked forever).
                for count, child in enumerate(sorted(entry.rglob("*"))):
                    if count >= MAX_ENTRIES:
                        break
                    if child.is_file():
                        audit.files[child.relative_to(path).as_posix()] = child.stat().st_size
            elif entry.is_file():
                size = entry.stat().st_size
                audit.files[entry.name] = size
                if entry.name.upper() in (n.upper() for n in KNOWN_FILES):
                    if size > MAX_ENTRY_BYTES:
                        audit.add("OBC-MOD-012", Severity.ERROR,
                                  f"{entry.name} is {size} bytes; the limit is {MAX_ENTRY_BYTES}")
                        continue
                    total += size
                    out[entry.name.upper()] = entry.read_bytes()
        if total > MAX_TOTAL_BYTES:
            audit.add("OBC-MOD-012", Severity.ERROR,
                      f"the mod holds {total} bytes; OBCR reads at most {MAX_TOTAL_BYTES}")
        if len(audit.files) > MAX_ENTRIES:
            audit.add("OBC-MOD-012", Severity.ERROR,
                      f"the mod has {len(audit.files)} files; OBCR reads at most {MAX_ENTRIES}")
        return out
    try:
        archive = zipfile.ZipFile(path)
    except (zipfile.BadZipFile, OSError) as error:
        audit.add("OBC-MOD-012", Severity.ERROR, f"not a readable .obcmod (ZIP): {error}")
        return out
    total = 0
    with archive:
        if len(archive.infolist()) > MAX_ENTRIES:
            audit.add("OBC-MOD-012", Severity.ERROR,
                      f"the archive has {len(archive.infolist())} entries; OBCR reads at most "
                      f"{MAX_ENTRIES}")
            return out
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
            if info.is_dir():
                continue
            normal = name.replace("\\", "/")
            if "/" in normal:
                if _is_asset_folder(normal.split("/", 1)[0]):
                    assets[normal] = archive.read(info)
                continue
            if name.upper() in (n.upper() for n in KNOWN_FILES):
                out[name.upper()] = archive.read(info)
    return out


def _audit_assets(audit: ModAudit, assets: dict[str, bytes], version) -> set[str]:
    """Check each asset file as OBCR will; returns the music names present."""
    from . import artimport, audioimport

    if assets and version != MOD_FORMAT_ASSETS:
        audit.add("OBC-MOD-016", Severity.WARNING,
                  f"the mod has {len(assets)} file(s) in asset folders, but its manifest is "
                  "format 1, which does not use them; write format = 2")
        return set()
    seen: dict[tuple, str] = {}
    music: set[str] = set()
    for name, data in sorted(assets.items()):
        try:
            parsed = parse_mod_path(name)
        except ResourceError as error:
            audit.problems.append(error.diagnostic)
            continue
        if parsed is None:
            continue
        kind, target = parsed
        key = (kind, str(target).lower())
        if key in seen:
            audit.add("OBC-MOD-016", Severity.ERROR,
                      f"{name} and {seen[key]} both replace {kind} {target}", name)
            continue
        seen[key] = name
        audit.assets[name] = parsed
        try:
            if kind in (WALL, SPRITE):
                artimport.check_art(data, name)
            elif kind == SOUND:
                audioimport.check_wav(data, name)
            else:
                audioimport.check_music(data, name.rsplit(".", 1)[-1], name)
                music.add(str(target).lower())
        except ObcEditError as error:
            audit.problems.append(error.diagnostic)
    return music


def audit_mod(path: Path | str, *, catalog=None) -> ModAudit:
    """Read a mod folder or `.obcmod` back and report everything about it."""
    from .document import MapDocument
    from .validation import validate_map

    path = Path(path)
    audit = ModAudit(path)
    if not path.exists():
        audit.add("OBC-MOD-012", Severity.ERROR, f"{path} does not exist")
        return audit
    assets: dict[str, bytes] = {}
    files = _read_files(path, audit, assets)
    for name in audit.files:
        normal = name.replace("\\", "/")
        in_assets = "/" in normal and _is_asset_folder(normal.split("/", 1)[0])
        if name.upper() not in (n.upper() for n in KNOWN_FILES) and not in_assets:
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
    stock_free = (all(level.stock is not None for level in audit.campaign.levels)
                  and not audit.campaign.arenas)
    music_present = _audit_assets(audit, assets, manifest.get("format"))
    played = {name.lower() for name in audit.campaign.music_files()}
    for entry_name, entry in ([(f"floor {i + 1}", lv) for i, lv in
                               enumerate(audit.campaign.levels)]
                              + [(f"arena {i + 1}", ar) for i, ar in
                                 enumerate(audit.campaign.arenas)]):
        name = music_file(entry.music)
        if name and name.lower() not in music_present \
                and manifest.get("format") == MOD_FORMAT_ASSETS:
            audit.add("OBC-MOD-015", Severity.ERROR,
                      f"music {name!r} is not in the mod's music folder", entry_name)
    for name in sorted(music_present - played):
        audit.add("OBC-MOD-018", Severity.WARNING,
                  f"music/{name} is in the mod but no floor or arena plays it")

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
    entries = ([(f"floor {i + 1}", lv) for i, lv in enumerate(audit.campaign.levels)]
               + [(f"arena {i + 1}", ar) for i, ar in enumerate(audit.campaign.arenas)])
    for where, entry in entries:
        if entry.map_uuid:
            slot = int(entry.map_uuid.split(":", 1)[1])
            if slot not in audit.maps:
                audit.add("OBC-MOD-006", Severity.ERROR,
                          f"{where} uses map {slot}, which the archive does not hold", where)
    if archive is not None:
        from .validation import ARENA, SINGLE_PLAYER

        used = {int(l.map_uuid.split(":", 1)[1]) for l in audit.campaign.levels if l.map_uuid}
        arenas = {int(a.map_uuid.split(":", 1)[1]) for a in audit.campaign.arenas
                  if a.map_uuid}
        for record in archive:
            if record.number not in used | arenas:
                continue
            document = MapDocument.from_record(record)
            checks = []
            if record.number in used:
                checks += validate_map(document, catalog, profile=SINGLE_PLAYER)
            if record.number in arenas:
                checks += [p for p in validate_map(document, catalog, profile=ARENA)
                           if p not in checks]
            for problem in checks:
                if problem.severity is Severity.ERROR:
                    audit.problems.append(Diagnostic(
                        problem.code, problem.severity,
                        f"map {record.number} ({record.name.text}): {problem.message}",
                        problem.where, problem.cell))
    return audit


__all__ = ["ASSET_FOLDERS", "KNOWN_FILES", "MAX_ENTRIES", "MOD_SUFFIX", "ModAudit", "ModBuild",
           "asset_files", "audit_mod", "build_mod", "preview_mod", "write_mod"]
