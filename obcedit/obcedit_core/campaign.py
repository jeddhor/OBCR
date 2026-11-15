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

    format = 1
    [mod]   id, title, author, version, description, release, obcr
    [[level]]  map | stock, name, like, music, dark, sewer, colors,
               sequence, message, exit

Maps are numbered in the mod's archive in campaign order: the first map level
is slot 0, the next distinct map slot 1, and so on.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field, replace

from .errors import Diagnostic, Severity

MOD_FORMAT = 1
MAX_LEVELS = 100
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

    def to_json(self) -> dict:
        return {
            "id": self.mod_id, "title": self.title, "author": self.author,
            "version": self.version, "description": self.description,
            "release": self.release, "obcr": self.obcr, "readme": self.readme,
            "levels": [level.to_json() for level in self.levels],
        }

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

    def map_order(self) -> list[str]:
        """Distinct map uuids in campaign order: their slots in the mod archive."""
        order: list[str] = []
        for level in self.levels:
            if level.map_uuid and level.map_uuid not in order:
                order.append(level.map_uuid)
        return order


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


def validate(campaign: Campaign, documents=(), *, sharing: bool = True) -> list[Diagnostic]:
    """Every reason this campaign cannot become a mod OBCR accepts."""
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
        if level.map_uuid and level.map_uuid not in by_uuid:
            problems.append(_problem("OBC-MOD-006", Severity.ERROR,
                                     "this floor's map is not in the project", where))
        document = by_uuid.get(level.map_uuid)
        if document is not None and sharing and document.source is not None \
                and document.source.identified:
            problems.append(_problem(
                "OBC-MOD-009", Severity.ERROR,
                f"'{document.name}' was imported from the original game; its words are "
                "Operation Body Count's, not yours, so it cannot go in a mod meant for "
                "sharing. Reference the original floor with 'stock' instead.", where))
        for key, value, limit in (("stock", level.stock, STOCK_FLOORS - 1),
                                  ("like", level.like, STOCK_FLOORS - 1),
                                  ("colors", level.colors, MAX_COLORS)):
            if value is not None and not (isinstance(value, int) and 0 <= value <= limit):
                problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                         f"{key} must be 0..{limit}", where))
        if level.music is not None and not (
            level.music == "random" or (isinstance(level.music, int) and 0 <= level.music <= 9)
        ):
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     "music must be 0..9 or \"random\"", where))
        if level.sequence is not None and level.sequence not in SEQUENCES:
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     f"sequence must be one of {', '.join(SEQUENCES)}", where))
        if level.exit is not None and level.exit not in EXITS:
            problems.append(_problem("OBC-MOD-007", Severity.ERROR,
                                     "exit must be \"normal\" or \"none\"", where))
        _text(problems, level.name, MAX_LEVEL_NAME, "the floor name", where)
        if level.message is not None:
            _text(problems, level.message, MAX_MESSAGE, "the message", where)
    if len(campaign.map_order()) > 100:
        problems.append(_problem("OBC-MOD-004", Severity.ERROR,
                                 "more than 100 distinct maps; MAPHEAD has 100 slots"))
    return problems


def render_manifest(campaign: Campaign, *, generator: str = "OBCEdit") -> str:
    """`mod.toml` text. Validate first: this writes what it is given."""
    slots = {uuid: slot for slot, uuid in enumerate(campaign.map_order())}
    lines = [f"# Written by {generator}. docs/MODS_AND_OBCEDIT.md section 3 has the format.",
             f"format = {MOD_FORMAT}", "", "[mod]",
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
            lines.append(f"music = {toml_string(level.music) if isinstance(level.music, str) else level.music}")
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
    return "\n".join(lines) + "\n"


def parse_manifest(text: str) -> tuple[dict, list[Diagnostic]]:
    """Read a `mod.toml` back (for audits), with the problems OBCR would report."""
    import tomllib

    problems: list[Diagnostic] = []
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        return {}, [_problem("OBC-MOD-010", Severity.ERROR, f"mod.toml does not parse: {error}")]
    allowed_top = {"format", "mod", "level"}
    allowed_mod = {"id", "title", "author", "version", "description", "release", "obcr"}
    allowed_level = {"map", "stock", "name", "like", "music", "dark", "sewer", "colors",
                     "sequence", "message", "exit"}
    for key in data:
        if key not in allowed_top:
            problems.append(_problem("OBC-MOD-011", Severity.ERROR, f"unknown key '{key}'"))
    if data.get("format") != MOD_FORMAT:
        problems.append(_problem("OBC-MOD-011", Severity.ERROR,
                                 f"format is {data.get('format')!r}; OBCR reads format 1"))
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
    return Campaign(
        mod_id=mod.get("id", ""), title=mod.get("title", ""), author=mod.get("author", ""),
        version=mod.get("version", ""), description=mod.get("description", ""),
        release=mod.get("release", "any"), obcr=mod.get("obcr", ""), levels=tuple(levels),
    )


__all__ = [
    "Campaign", "DEFAULT_LIKE", "EXITS", "LevelEntry", "MOD_FORMAT", "RELEASES", "SEQUENCES",
    "campaign_from_manifest", "parse_manifest", "render_manifest", "toml_string", "validate",
]
