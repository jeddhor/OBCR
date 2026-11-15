# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The semantic catalog: raw map words to things a person can recognize.

A map cell is a number. Plane-1 `116` is a soldier standing facing east,
plane-0 `130` a door, `1` a wall painted with page 0. The catalog is what lets
the palette show those as pictures with names, and what lets the validator and
the tools ask what a word *is*.

It is not written here. `resources/catalog.json` is generated from OBCR's own
code (`cargo run -p obc-tool -- catalog`): every plane-1 code is spawned into a
scratch world by the engine's real `SetupGameLevel`, so a name, class, sprite
and difficulty tier here is what the game does with that word. This module
only groups that file into palette entries:

* one entry per wall, door, floor or static value;
* one entry per actor *kind and tier*, carrying its four facings as
  `directions` (the raw word for each), so a soldier is one palette item with
  a facing control rather than four items;
* player starts as one entry with four facings; patrol arrows as one with
  eight.

`schema`/`version` are checked: a catalog from a newer OBCR is refused rather
than half-understood.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

#: The `version` this module understands (obc-tool's SCHEMA_VERSION).
CATALOG_SCHEMA = 1
SCHEMA_NAME = "obcedit-catalog"

#: Palette tabs, in display order.
CATEGORIES = ("walls", "specials", "floors", "objects", "enemies", "starts", "markers", "raw")

#: Plane-0 wall values map to GFXTILES pages one lower: wall 1 is page 0.
WALL_PAGE_OFFSET = 1

#: Plane-1 statics: `statinfo[code - 23]`.
STATIC_WORD_BASE = 23

#: Compass names for the 8-way patrol arrows (90 + index).
ARROW_ORDER = ("east", "northeast", "north", "northwest", "west", "southwest", "south", "southeast")


class CatalogError(ValueError):
    """The catalog file is missing, malformed or from a newer OBCR."""


@dataclass(frozen=True)
class CatalogEntry:
    """One selectable thing, with everything needed to draw and place it."""

    key: str
    category: str
    subcategory: str
    rank: int
    name: str
    plane: int
    value: int
    values: tuple[int, ...]

    description: str = ""
    aliases: tuple[str, ...] = ()

    #: The actor class name (enemies, allies), or "".
    actor: str = ""
    #: Sprite number (index into GFXTILES' sprite list) for thumbnails.
    sprite: int | None = None
    #: Wall page for plane-0 entries drawn as a texture.
    page: int | None = None
    #: The raw catalog record of `value`, for anything not lifted into fields.
    data: dict = field(default_factory=dict, compare=False, hash=False)

    #: Facing name to the exact raw value that produces it.
    directions: tuple[tuple[str, int], ...] = ()
    #: "stand" / "patrol" for actors.
    variant: str = ""
    #: Lowest game type that spawns it (0 = all, 1 = types 2 and 3, 2 = type 3).
    minskill: int = 0

    placement: str = "floor"  # floor | wall | any
    blocking: bool | None = None
    #: False for values that only appear as animation frames or are unused.
    safe_for_new_maps: bool = True

    @property
    def resolved(self) -> bool:
        return bool(self.values)

    @property
    def texture(self) -> str:
        return f"page {self.page}" if self.page is not None else ""

    @property
    def kind(self) -> str:
        return str(self.data.get("kind", ""))

    def value_for(self, facing: str) -> int:
        """The raw word for `facing`, or `value` when this entry has no facings."""
        for name, value in self.directions:
            if name == facing:
                return value
        return self.value

    def facing_of(self, value: int) -> str:
        for name, raw in self.directions:
            if raw == value:
                return name
        return ""

    def matches(self, query: str) -> bool:
        """Search by friendly name, raw value, class, page or alias."""
        needle = query.strip().lower()
        if not needle:
            return True
        if needle.isdigit() and int(needle) in self.values:
            return True
        haystack = [self.name, self.actor, self.texture, self.key, self.subcategory]
        haystack.extend(self.aliases)
        return any(needle in item.lower() for item in haystack if item)


@dataclass
class Catalog:
    """Every entry, plus the generated file's limits and floor sets."""

    entries: tuple[CatalogEntry, ...] = ()
    unresolved: tuple[str, ...] = ()
    schema: int = CATALOG_SCHEMA
    limits: dict = field(default_factory=dict)
    floor_sets: tuple[dict, ...] = ()
    #: Every raw record by (plane, value), as generated.
    records: dict = field(default_factory=dict, repr=False)

    _by_value: dict | None = field(default=None, repr=False, compare=False)
    _by_key: dict | None = field(default=None, repr=False, compare=False)

    def __len__(self) -> int:
        return len(self.entries)

    def __iter__(self):
        return iter(self.entries)

    def _index(self) -> dict:
        if self._by_value is None:
            index: dict[tuple[int, int], CatalogEntry] = {}
            for entry in self.entries:
                for value in entry.values:
                    index.setdefault((entry.plane, value), entry)
            self._by_value = index
        return self._by_value

    def by_key(self, key: str) -> CatalogEntry | None:
        if self._by_key is None:
            self._by_key = {entry.key: entry for entry in self.entries}
        return self._by_key.get(key)

    def for_value(self, plane: int, value: int) -> CatalogEntry | None:
        """The entry that owns a raw value on a plane, if any does."""
        return self._index().get((plane, value))

    def record(self, plane: int, value: int) -> dict:
        """The generated record for one raw value ({} when unknown)."""
        return self.records.get((plane, value), {})

    def kind(self, plane: int, value: int) -> str:
        return str(self.record(plane, value).get("kind", ""))

    def in_category(self, category: str) -> list[CatalogEntry]:
        return sorted(
            (e for e in self.entries if e.category == category),
            key=lambda e: (e.rank, e.name),
        )

    def search(self, query: str, *, category: str = "") -> list[CatalogEntry]:
        pool = self.in_category(category) if category else list(self.entries)
        return [entry for entry in pool if entry.matches(query)]

    # -- convenience for rules and validation -------------------------------

    def limit(self, name: str, default: int = 0) -> int:
        return int(self.limits.get(name, default))

    def is_solid(self, value: int) -> bool:
        """Plane 0: solid for the engine's first pass (< AREATILE)."""
        return value < self.limit("areatile", 140)

    def wall_page(self, value: int) -> int | None:
        page = self.record(0, value).get("page")
        return int(page) if page is not None else None

    def floor_set(self, index: int) -> dict:
        for item in self.floor_sets:
            if item.get("set") == index:
                return item
        return {}

    def rotation_groups(self) -> list[tuple[int, ...]]:
        """Raw words that are one thing facing different ways.

        Each group is ordered by compass heading, quarter turns apart for
        4-way groups (north, east, south, west) and eighth turns for the
        8-way patrol arrows (east, northeast, ...). Rotation tools step
        through a group; nothing hardcodes the codes.
        """
        groups = []
        for entry in self.entries:
            if len(entry.directions) < 2:
                continue
            facing = dict(entry.directions)
            if len(facing) == 8:
                groups.append(tuple(facing[name] for name in ARROW_ORDER))
            elif {"north", "east", "south", "west"} <= facing.keys():
                groups.append(tuple(facing[n] for n in ("north", "east", "south", "west")))
            elif len(facing) == 2 and {"east", "north"} <= facing.keys():
                # The brute only comes facing east or north (142/143, 160/161).
                groups.append((facing["north"], facing["east"]))
        return groups


# ---------------------------------------------------------------------------
# Building entries from the generated file
# ---------------------------------------------------------------------------

_SPECIAL_KINDS = {
    "door_look": ("Door-look walls", 10),
    "glass": ("Glass", 20),
    "glass_broken": ("Glass", 21),
    "fan": ("Fans", 30),
    "damaged": ("Walls", 40),
    "panel": ("Panels", 50),
    "rock_opening": ("Openings", 60),
    "generator": ("Generator switches", 70),
    "exit": ("Exits", 0),
}

_UNPLACEABLE = {"door_look_frame", "glass_frame", "fan_frame", "invalid", "unused"}


def _plane0_entries(records: list[dict]) -> list[CatalogEntry]:
    out = []
    for rec in records:
        value = int(rec["value"])
        kind = rec.get("kind", "")
        common = dict(name=rec.get("name", f"Plane 0 value {value}"), plane=0, value=value,
                      values=(value,), data=rec, page=rec.get("page"),
                      description=rec.get("note", ""))
        if kind == "wall":
            out.append(CatalogEntry(f"wall-{value}", "walls", "Walls", value, placement="any",
                                    blocking=True, **common))
        elif kind in _SPECIAL_KINDS:
            sub, rank = _SPECIAL_KINDS[kind]
            out.append(CatalogEntry(f"special-{value}", "specials", sub, rank * 1000 + value,
                                    placement="any", blocking=True, **common))
        elif kind == "door":
            out.append(CatalogEntry(f"door-{value}", "specials", "Doors", 5000 + value,
                                    placement="any", blocking=True, **common))
        elif kind == "ambush":
            out.append(CatalogEntry("ambush", "markers", "Ambush", 0, placement="floor",
                                    blocking=False, **common))
        elif kind == "floor":
            area = int(rec.get("area", 0))
            fset = rec.get("floor_set")
            sub = "Floors" if fset is not None else "Floors (no texture set)"
            out.append(CatalogEntry(f"floor-{value}", "floors", sub, value, placement="any",
                                    blocking=False, aliases=(f"area {area}",), **common))
        else:
            out.append(CatalogEntry(f"raw0-{value}", "raw", kind or "raw", value,
                                    placement="any", safe_for_new_maps=kind not in _UNPLACEABLE,
                                    **common))
    return out


def _actor_entries(records: list[dict]) -> list[CatalogEntry]:
    """Enemies and allies, one entry per (name without facing, tier)."""
    groups: dict[tuple, list[dict]] = {}
    for rec in records:
        tier = int(rec.get("difficulty", {}).get("tier", 0))
        base = rec["name"].rsplit(" ", 1)[0] if rec.get("facing") else rec["name"]
        groups.setdefault((rec["kind"], base, tier, rec.get("class")), []).append(rec)
    out = []
    for (kind, base, tier, cls), recs in groups.items():
        recs.sort(key=lambda r: r["value"])
        first = recs[0]
        tier_label = ("", " (game types 2-3)", " (game type 3 only)")[min(tier, 2)]
        directions = tuple((r["facing"], int(r["value"])) for r in recs if r.get("facing"))
        category = "starts" if kind == "ally" else "enemies"
        name = base + tier_label
        key = f"{kind}-{first['value']}"
        out.append(
            CatalogEntry(
                key, category, "Allies" if kind == "ally" else first.get("class_name", "Enemies"),
                int(first.get("class", 0)) * 1000 + tier * 100 + int(first["value"]) % 100,
                name, 1, int(first["value"]), tuple(int(r["value"]) for r in recs),
                actor=first.get("class_name", ""), sprite=first.get("sprite"), data=first,
                directions=directions,
                variant="patrol" if first.get("patrol") else "stand",
                minskill=tier, placement="floor", blocking=True,
                description="Ends the game when killed." if first.get("ends_game") else "",
            )
        )
    return out


def _plane1_entries(records: list[dict]) -> list[CatalogEntry]:
    out = []
    actors = [r for r in records if r.get("kind") in ("enemy", "ally")]
    out.extend(_actor_entries(actors))
    players = sorted((r for r in records if r.get("kind") == "player"), key=lambda r: r["value"])
    if players:
        out.append(CatalogEntry(
            "player-start", "starts", "Player", 0, "Player start", 1, players[0]["value"],
            tuple(r["value"] for r in players), data=players[0],
            directions=tuple((r["facing"], r["value"]) for r in players), placement="floor",
            description="Exactly one per map.",
        ))
    arrows = sorted((r for r in records if r.get("kind") == "patrol_arrow"), key=lambda r: r["value"])
    if arrows:
        out.append(CatalogEntry(
            "patrol-arrow", "markers", "Patrol routes", 10, "Patrol turn arrow", 1,
            arrows[0]["value"], tuple(r["value"] for r in arrows), data=arrows[0],
            directions=tuple((r["direction"], r["value"]) for r in arrows), placement="floor",
            description="A patrolling actor stepping on it turns to face this way.",
        ))
    for rec in records:
        value = int(rec["value"])
        kind = rec.get("kind", "")
        common = dict(name=rec["name"], plane=1, value=value, values=(value,), data=rec)
        if kind in ("static", "pickup"):
            out.append(CatalogEntry(
                f"{kind}-{value}", "objects", "Pickups" if kind == "pickup" else
                ("Obstacles" if rec.get("blocking") else "Decorations"), value,
                sprite=rec.get("sprite"), blocking=bool(rec.get("blocking")), **common))
        elif kind == "fire":
            if value == min(r["value"] for r in records if r.get("kind") == "fire"):
                fires = tuple(r["value"] for r in records if r.get("kind") == "fire")
                out.append(CatalogEntry("fire", "enemies", "Fire", 99000, "Fire", 1, value, fires,
                                        data=rec, actor="Fire", placement="floor"))
        elif kind == "anim_wall":
            out.append(CatalogEntry(
                f"anim-{value}", "markers", "Wall behaviour", 100 + value, placement="wall",
                safe_for_new_maps=not rec.get("unused_in_maps"), **common))
        elif kind == "push_wall":
            out.append(CatalogEntry("push-wall", "markers", "Wall behaviour", 98,
                                    placement="wall", **common))
        elif kind == "marker":
            out.append(CatalogEntry(f"marker-{value}", "markers", "Wall behaviour", 200 + value,
                                    placement="wall" if value != 125 else "wall", **common))
        elif kind == "empty":
            out.append(CatalogEntry("empty", "markers", "Empty", 0, placement="any", **common))
    return out


def catalog_from_data(data: dict[str, Any]) -> Catalog:
    if data.get("schema") != SCHEMA_NAME:
        raise CatalogError(f"not an OBCEdit catalog (schema {data.get('schema')!r})")
    version = int(data.get("version", 0))
    if version > CATALOG_SCHEMA:
        raise CatalogError(
            f"catalog version {version} is newer than this OBCEdit understands "
            f"({CATALOG_SCHEMA}); use the OBCEdit that came with that OBCR"
        )
    plane0 = list(data.get("plane0", ()))
    plane1 = list(data.get("plane1", ()))
    records = {(0, int(r["value"])): r for r in plane0}
    records.update({(1, int(r["value"])): r for r in plane1})
    entries = _plane0_entries(plane0) + _plane1_entries(plane1)
    return Catalog(
        tuple(entries), (), version, dict(data.get("limits", {})),
        tuple(data.get("floor_sets", ())), records,
    )


def catalog_from_json(text: str) -> Catalog:
    try:
        data = json.loads(text)
    except json.JSONDecodeError as error:
        raise CatalogError(f"catalog is not valid JSON: {error}") from error
    return catalog_from_data(data)


def load_catalog(path: Path | str) -> Catalog:
    return catalog_from_json(Path(path).read_text(encoding="utf-8"))


def default_catalog() -> Catalog:
    """The catalog this build ships (`resources/catalog.json`)."""
    from .bundle import catalog_path

    return load_catalog(catalog_path())
