# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""A mod's campaign: the bounded `mod.toml` schema, checked and written.

A mod is an ordered list of floors (docs/MODS_AND_OBCEDIT.md section 3). Each
floor is either one of the project's maps or an original floor by slot
(`stock`), with the behaviour profile of an original floor (`like`) and a few
overrides. OBCR refuses unknown keys and out-of-range values, so this module
checks every field against the same limits before anything is written, and
writes the TOML itself (the standard library reads TOML but does not write it).

The schema, stored in a project as JSON and written to `mod.toml`:

    format = 1 | 2
    [mod]   id, title, author, version, description, release, obcr
    [[level]]  map | stock, name, like, music, dark, sewer, colors,
               sequence, message, exit
    [[arena]]  map, name, music, colors, dark, sewer          (format 2)

Format 2 (docs/MOD_ASSETS_AND_ARENAS.md section 2.2) adds network arenas, a
`music` that names one of the mod's music files, and the asset folders. A mod
that uses none of that is still written as format 1, so OBCRs before format 2
keep playing it.

Maps are numbered in the mod's archive in campaign order: the first map level
is slot 0, the next distinct map slot 1, and so on; arenas' maps follow.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field, replace

from .errors import Diagnostic, Severity

MOD_FORMAT = 1
#: Arenas, music files and asset folders (section 2).
MOD_FORMAT_ASSETS = 2
FORMATS = (MOD_FORMAT, MOD_FORMAT_ASSETS)
MAX_LEVELS = 100
MAX_ARENAS = 10
#: Network floors have their own colour entries, 50-59.
MAX_ARENA_COLORS = 59
ARENA_COLOR_BASE = 50
MAX_ID = 32
MAX_TITLE = 32
MAX_AUTHOR = 64
MAX_VERSION = 16
MAX_DESCRIPTION = 400
MAX_LEVEL_NAME = 16
MAX_MESSAGE = 40
STOCK_FLOORS = 40
MAX_COLORS = 50
RELEASES = ("any", "OBC-A", "OBC-CD")
SEQUENCES = ("auto", "F2", "F3", "F4", "F5", "F6", "F7", "none")
EXITS = ("normal", "none")
#: The default behaviour profile: a floor of UN Building 1.
DEFAULT_LIKE = 6

_ID = re.compile(r"\A[a-z0-9-]{1,32}\Z")
_PRINTABLE = re.compile(r"\A[\x20-\x7e]*\Z")
_VERSION = re.compile(r"\A\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?\Z")


def toml_string(text: str) -> str:
    """A TOML basic string. Callers have already refused non-printable text."""
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


@dataclass(frozen=True)
class LevelEntry:
    """One floor of the campaign."""

    #: A project map's uuid; empty for a stock floor.
    map_uuid: str = ""
    #: An original floor (slot 0-39), or None.
    stock: int | None = None
    name: str = ""
    like: int | None = None
    #: 0-9, "random", or None for the profile's.
    music: int | str | None = None
    dark: bool | None = None
    sewer: bool | None = None
    colors: int | None = None
    sequence: str | None = None
    message: str | None = None
    exit: str | None = None

    def to_json(self) -> dict:
        out = {}
        for key in ("map_uuid", "stock", "name", "like", "music", "dark", "sewer", "colors",
                    "sequence", "message", "exit"):
            value = getattr(self, key)
            if value not in (None, ""):
                out[key] = value
        return out

    @classmethod
    def from_json(cls, raw: dict) -> "LevelEntry":
        known = {k: raw[k] for k in raw if k in cls.__dataclass_fields__}
        return cls(**known)


@dataclass(frozen=True)
class ArenaEntry:
    """One network floor (format 2): always one of the project's own maps."""

    map_uuid: str = ""
    name: str = ""
    #: 0-9, "random", a music file's name, or None (random).
    music: int | str | None = None
    #: 0-59; None means 50 + its place in the list.
    colors: int | None = None
    dark: bool | None = None
    sewer: bool | None = None

    def to_json(self) -> dict:
        out = {}
        for key in ("map_uuid", "name", "music", "colors", "dark", "sewer"):
            value = getattr(self, key)
            if value not in (None, ""):
                out[key] = value
        return out

    @classmethod
    def from_json(cls, raw: dict) -> "ArenaEntry":
        known = {k: raw[k] for k in raw if k in cls.__dataclass_fields__}
        return cls(**known)


def music_file(value) -> str | None:
    """The music file a `music` value names, or None for a song number or random."""
    if isinstance(value, str) and value != "random":
        return value
    return None


@dataclass(frozen=True)
class Campaign:
    """The mod's identity and its floors, in order."""

    mod_id: str = ""
    title: str = ""
    author: str = ""
    version: str = "1.0"
    description: str = ""
    release: str = "any"
    obcr: str = ""
    levels: tuple[LevelEntry, ...] = ()
    readme: str = ""
    arenas: tuple[ArenaEntry, ...] = ()

    def to_json(self) -> dict:
        out = {
            "id": self.mod_id, "title": self.title, "author": self.author,
            "version": self.version, "description": self.description,
            "release": self.release, "obcr": self.obcr, "readme": self.readme,
            "levels": [level.to_json() for level in self.levels],
        }
        if self.arenas:
            out["arenas"] = [arena.to_json() for arena in self.arenas]
        return out

    @classmethod
    def from_json(cls, raw: dict) -> "Campaign":
        raw = raw or {}
        return cls(
            mod_id=str(raw.get("id", "")), title=str(raw.get("title", "")),
            author=str(raw.get("author", "")), version=str(raw.get("version", "1.0")),
            description=str(raw.get("description", "")),
            release=str(raw.get("release", "any")), obcr=str(raw.get("obcr", "")),
            readme=str(raw.get("readme", "")),
            levels=tuple(LevelEntry.from_json(level) for level in raw.get("levels", ())
                         if isinstance(level, dict)),
            arenas=tuple(ArenaEntry.from_json(arena) for arena in raw.get("arenas", ())
                         if isinstance(arena, dict)),
        )

    @classmethod
    def default_for(cls, project) -> "Campaign":
        """Every map of the project, in order, as building floors."""
        slug = re.sub(r"[^a-z0-9-]+", "-", project.name.lower()).strip("-")[:MAX_ID] or "my-mod"
        return cls(
            mod_id=slug, title=project.name[:MAX_TITLE] or "My mod",
            author=project.author[:MAX_AUTHOR],
            levels=tuple(LevelEntry(map_uuid=d.uuid, name=d.name[:MAX_LEVEL_NAME])
                         for d in project.maps),
        )

    def with_levels(self, levels) -> "Campaign":
        return replace(self, levels=tuple(levels))

    def with_arenas(self, arenas) -> "Campaign":
        return replace(self, arenas=tuple(arenas))

    def map_order(self) -> list[str]:
        """Distinct map uuids in campaign order (floors, then arenas): their
        slots in the mod archive."""
        order: list[str] = []
        for entry in self.levels + self.arenas:
            if entry.map_uuid and entry.map_uuid not in order:
                order.append(entry.map_uuid)
        return order

    def arena_maps(self) -> set[str]:
        """The project maps used as network arenas (validated as arenas)."""
        return {arena.map_uuid for arena in self.arenas if arena.map_uuid}

    def music_files(self) -> list[str]:
        """Music file names the floors and arenas play, in first-use order."""
        names: list[str] = []
        for entry in self.levels + self.arenas:
            name = music_file(entry.music)
            if name and name.lower() not in (n.lower() for n in names):
                names.append(name)
        return names

    def needs_format_2(self) -> bool:
        """Whether the manifest uses anything format 1 does not have."""
        return bool(self.arenas) or bool(self.music_files())


def _problem(code: str, severity: Severity, message: str, where: str = "") -> Diagnostic:
    return Diagnostic(code, severity, message, where)


def _text(problems, value: str, limit: int, what: str, where: str, *, required=False):
    if required and not value.strip():
        problems.append(_problem("OBC-MOD-002", Severity.ERROR, f"{what} is empty", where))
    if len(value) > limit:
        problems.append(_problem("OBC-MOD-002", Severity.ERROR,
                                 f"{what} is {len(value)} characters; the limit is {limit}", where))
    if not _PRINTABLE.match(value):
        problems.append(_problem("OBC-MOD-002", Severity.ERROR,
                                 f"{what} may only use printable ASCII (the game's fonts)", where))


def _check_music(problems, value, where, music_names) -> None:
    from .resources import is_music_name

    if value is None or value == "random":
        return
    if isinstance(value, int) and not isinstance(value, bool) and 0 <= value <= 9:
        return
    if isinstance(value, str) and is_music_name(value):
        if music_names is not None and value.lower() not in music_names:
            problems.append(_problem("OBC-MOD-015", Severity.ERROR,
                                     f"music {value!r} is not one of the project's music files "
                                     "(Resources > Music)", where))
        return
    problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                             "music must be 0..9, \"random\" or a music file's name", where))


def _check_own_map(problems, map_uuid, by_uuid, where, sharing) -> None:
    if map_uuid and map_uuid not in by_uuid:
        problems.append(_problem("OBC-MOD-006", Severity.ERROR,
                                 "this floor's map is not in the project", where))
    document = by_uuid.get(map_uuid)
    if document is not None and sharing and document.source is not None \
            and document.source.identified:
        problems.append(_problem(
            "OBC-MOD-009", Severity.ERROR,
            f"'{document.name}' was imported from the original game; its words are "
            "Operation Body Count's, not yours, so it cannot go in a mod meant for "
            "sharing. Reference the original floor with 'stock' instead.", where))


def validate(campaign: Campaign, documents=(), *, sharing: bool = True,
             music_names=None) -> list[Diagnostic]:
    """Every reason this campaign cannot become a mod OBCR accepts.

    `music_names` are the project's music files; None skips the check that a
    named file exists (an audit checks the mod's own folder instead).
    """
    if music_names is not None:
        music_names = {name.lower() for name in music_names}
    problems: list[Diagnostic] = []
    if not _ID.match(campaign.mod_id):
        problems.append(_problem("OBC-MOD-001", Severity.ERROR,
                                 "the mod id must be 1-32 characters of a-z, 0-9 and '-'", "id"))
    _text(problems, campaign.title, MAX_TITLE, "the title", "title", required=True)
    _text(problems, campaign.author, MAX_AUTHOR, "the author", "author")
    _text(problems, campaign.version, MAX_VERSION, "the version", "version", required=True)
    _text(problems, campaign.description, MAX_DESCRIPTION, "the description", "description")
    if campaign.release not in RELEASES:
        problems.append(_problem("OBC-MOD-003", Severity.ERROR,
                                 f"release must be one of {', '.join(RELEASES)}", "release"))
    if campaign.obcr and not _VERSION.match(campaign.obcr):
        problems.append(_problem("OBC-MOD-003", Severity.ERROR,
                                 f"{campaign.obcr!r} is not an OBCR version like 1.0.0-beta.3",
                                 "obcr"))
    if not campaign.levels:
        problems.append(_problem("OBC-MOD-004", Severity.ERROR, "the campaign has no floors"))
    if len(campaign.levels) > MAX_LEVELS:
        problems.append(_problem("OBC-MOD-004", Severity.ERROR,
                                 f"{len(campaign.levels)} floors; the limit is {MAX_LEVELS}"))
    by_uuid = {d.uuid: d for d in documents}
    for index, level in enumerate(campaign.levels):
        where = f"floor {index + 1}"
        if bool(level.map_uuid) == (level.stock is not None):
            problems.append(_problem("OBC-MOD-005", Severity.ERROR,
                                     "a floor is either one of your maps or an original floor",
                                     where))
        _check_own_map(problems, level.map_uuid, by_uuid, where, sharing)
        for key, value, limit in (("stock", level.stock, STOCK_FLOORS - 1),
                                  ("like", level.like, STOCK_FLOORS - 1),
                                  ("colors", level.colors, MAX_COLORS)):
            if value is not None and not (isinstance(value, int) and 0 <= value <= limit):
                problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                         f"{key} must be 0..{limit}", where))
        _check_music(problems, level.music, where, music_names)
        if level.sequence is not None and level.sequence not in SEQUENCES:
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     f"sequence must be one of {', '.join(SEQUENCES)}", where))
        if level.exit is not None and level.exit not in EXITS:
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     "exit must be \"normal\" or \"none\"", where))
        _text(problems, level.name, MAX_LEVEL_NAME, "the floor name", where)
        if level.message is not None:
            _text(problems, level.message, MAX_MESSAGE, "the message", where)
    if len(campaign.arenas) > MAX_ARENAS:
        problems.append(_problem("OBC-MOD-017", Severity.ERROR,
                                 f"{len(campaign.arenas)} arenas; a mod has at most {MAX_ARENAS}"))
    for index, arena in enumerate(campaign.arenas):
        where = f"arena {index + 1}"
        if not arena.map_uuid:
            problems.append(_problem("OBC-MOD-017", Severity.ERROR,
                                     "an arena is one of your own maps", where))
        _check_own_map(problems, arena.map_uuid, by_uuid, where, sharing)
        if arena.colors is not None and not (
                isinstance(arena.colors, int) and not isinstance(arena.colors, bool)
                and 0 <= arena.colors <= MAX_ARENA_COLORS):
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     f"colors must be 0..{MAX_ARENA_COLORS}", where))
        for key in ("dark", "sewer"):
            value = getattr(arena, key)
            if value is not None and not isinstance(value, bool):
                problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                         f"{key} must be true or false", where))
        _check_music(problems, arena.music, where, music_names)
        _text(problems, arena.name, MAX_LEVEL_NAME, "the arena name", where)
    if len(campaign.map_order()) > 100:
        problems.append(_problem("OBC-MOD-004", Severity.ERROR,
                                 "more than 100 distinct maps; MAPHEAD has 100 slots"))
    return problems


def _music_value(value) -> str:
    return toml_string(value) if isinstance(value, str) else str(value)


def render_manifest(campaign: Campaign, *, generator: str = "OBCEdit",
                    format: int | None = None) -> str:
    """`mod.toml` text. Validate first: this writes what it is given.

    `format` defaults to 1, or 2 when the campaign itself needs it (arenas,
    music files); a caller whose mod carries asset folders passes 2.
    """
    if format is None:
        format = MOD_FORMAT_ASSETS if campaign.needs_format_2() else MOD_FORMAT
    slots = {uuid: slot for slot, uuid in enumerate(campaign.map_order())}
    reference = ("docs/MOD_ASSETS_AND_ARENAS.md section 2" if format >= MOD_FORMAT_ASSETS
                 else "docs/MODS_AND_OBCEDIT.md section 3")
    lines = [f"# Written by {generator}. {reference} has the format.",
             f"format = {format}", "", "[mod]",
             f"id = {toml_string(campaign.mod_id)}", f"title = {toml_string(campaign.title)}"]
    if campaign.author:
        lines.append(f"author = {toml_string(campaign.author)}")
    lines.append(f"version = {toml_string(campaign.version)}")
    if campaign.description:
        lines.append(f"description = {toml_string(campaign.description)}")
    lines.append(f"release = {toml_string(campaign.release)}")
    if campaign.obcr:
        lines.append(f"obcr = {toml_string(campaign.obcr)}")
    for level in campaign.levels:
        lines += ["", "[[level]]"]
        if level.map_uuid:
            lines.append(f"map = {slots[level.map_uuid]}")
        else:
            lines.append(f"stock = {level.stock}")
        if level.name:
            lines.append(f"name = {toml_string(level.name)}")
        if level.like is not None:
            lines.append(f"like = {level.like}")
        if level.music is not None:
            lines.append(f"music = {_music_value(level.music)}")
        for key in ("dark", "sewer"):
            value = getattr(level, key)
            if value is not None:
                lines.append(f"{key} = {'true' if value else 'false'}")
        if level.colors is not None:
            lines.append(f"colors = {level.colors}")
        for key in ("sequence", "message", "exit"):
            value = getattr(level, key)
            if value is not None:
                lines.append(f"{key} = {toml_string(value)}")
    for arena in campaign.arenas:
        lines += ["", "[[arena]]", f"map = {slots[arena.map_uuid]}"]
        if arena.name:
            lines.append(f"name = {toml_string(arena.name)}")
        if arena.music is not None:
            lines.append(f"music = {_music_value(arena.music)}")
        if arena.colors is not None:
            lines.append(f"colors = {arena.colors}")
        for key in ("dark", "sewer"):
            value = getattr(arena, key)
            if value is not None:
                lines.append(f"{key} = {'true' if value else 'false'}")
    return "\n".join(lines) + "\n"


def parse_manifest(text: str) -> tuple[dict, list[Diagnostic]]:
    """Read a `mod.toml` back (for audits), with the problems OBCR would report."""
    import tomllib

    problems: list[Diagnostic] = []
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        return {}, [_problem("OBC-MOD-010", Severity.ERROR, f"mod.toml does not parse: {error}")]
    version = data.get("format")
    allowed_top = {"format", "mod", "level"}
    if version == MOD_FORMAT_ASSETS:
        allowed_top.add("arena")
    allowed_mod = {"id", "title", "author", "version", "description", "release", "obcr"}
    allowed_level = {"map", "stock", "name", "like", "music", "dark", "sewer", "colors",
                     "sequence", "message", "exit"}
    allowed_arena = {"map", "name", "music", "colors", "dark", "sewer"}
    for key in data:
        if key not in allowed_top:
            problems.append(_problem("OBC-MOD-011", Severity.ERROR, f"unknown key '{key}'"))
    if version not in FORMATS:
        problems.append(_problem("OBC-MOD-011", Severity.ERROR,
                                 f"format is {version!r}; OBCR reads formats 1 and 2"))
    for index, arena in enumerate(data.get("arena", []) if version == MOD_FORMAT_ASSETS else []):
        for key in arena:
            if key not in allowed_arena:
                problems.append(_problem("OBC-MOD-011", Severity.ERROR,
                                         f"unknown key '{key}'", f"arena {index + 1}"))
        if "map" not in arena:
            problems.append(_problem("OBC-MOD-017", Severity.ERROR, "an arena needs a map",
                                     f"arena {index + 1}"))
    if version == MOD_FORMAT:
        for index, level in enumerate(data.get("level", [])):
            if music_file(level.get("music")):
                problems.append(_problem(
                    "OBC-MOD-011", Severity.ERROR,
                    "music names a file, which needs format = 2", f"floor {index + 1}"))
    for key in data.get("mod", {}):
        if key not in allowed_mod:
            problems.append(_problem("OBC-MOD-011", Severity.ERROR, f"unknown key 'mod.{key}'"))
    for index, level in enumerate(data.get("level", [])):
        for key in level:
            if key not in allowed_level:
                problems.append(_problem("OBC-MOD-011", Severity.ERROR,
                                         f"unknown key '{key}'", f"floor {index + 1}"))
    return data, problems


def campaign_from_manifest(data: dict) -> Campaign:
    """A Campaign (with stock levels and map slots as 'slot:N' uuids) from parsed TOML."""
    mod = data.get("mod", {})
    levels = []
    for level in data.get("level", []):
        levels.append(LevelEntry(
            map_uuid=f"slot:{level['map']}" if "map" in level else "",
            stock=level.get("stock"), name=level.get("name", ""), like=level.get("like"),
            music=level.get("music"), dark=level.get("dark"), sewer=level.get("sewer"),
            colors=level.get("colors"), sequence=level.get("sequence"),
            message=level.get("message"), exit=level.get("exit"),
        ))
    arenas = [ArenaEntry(
        map_uuid=f"slot:{arena['map']}" if "map" in arena else "",
        name=arena.get("name", ""), music=arena.get("music"), colors=arena.get("colors"),
        dark=arena.get("dark"), sewer=arena.get("sewer"),
    ) for arena in (data.get("arena", []) if data.get("format") == MOD_FORMAT_ASSETS else [])]
    return Campaign(
        mod_id=mod.get("id", ""), title=mod.get("title", ""), author=mod.get("author", ""),
        version=mod.get("version", ""), description=mod.get("description", ""),
        release=mod.get("release", "any"), obcr=mod.get("obcr", ""), levels=tuple(levels),
        arenas=tuple(arenas),
    )


__all__ = [
    "ARENA_COLOR_BASE", "ArenaEntry", "FORMATS", "MAX_ARENAS", "MAX_ARENA_COLORS",
    "MOD_FORMAT_ASSETS", "music_file",
    "Campaign", "DEFAULT_LIKE", "EXITS", "LevelEntry", "MOD_FORMAT", "RELEASES", "SEQUENCES",
    "campaign_from_manifest", "parse_manifest", "render_manifest", "toml_string", "validate",
]
