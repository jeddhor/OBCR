# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""The validator: what stops a map loading or playing, and what probably hurts.

Deliberately small, in EC7Edit's spirit: a validator that reports forty
advisory notes on a map somebody just started trains them to ignore it. The
rules are Operation Body Count's (docs/MODS_AND_OBCEDIT.md section 5.7), from
docs/re/LEVEL_CODES.md and the engine's limits in the catalog.

Errors (block export and playtest):

* the map is not 64x64 (the engine refuses anything else);
* the outer boundary is open (the player walks out of the world);
* no player start, or more than one;
* more doors, actors, statics or animated walls than the engine holds;
* plane-0 words the engine cannot make sense of (0, or 162 and up);
* doors on the edge, or without floor on the sides they open to;
* a push wall marker that is not on a wall;
* plane-1 words past the original's spawn table (318 and up);
* things standing inside walls (a warning on maps imported from the game).

Warnings (advisory):

* no exit reachable, or not enough reachable enemies for the half-kill
  quota (reachability.py);
* a push wall that cannot move, an exit with no floor to use it from;
* an ambush marker with no standing enemy on it (it stays a wall);
* a door-look marker on something that is not a door-look wall;
* plane-1 words the game ignores (19-317 with no meaning);
* static 71, which blocks only in the CD release;
* a patroller that walks into a wall with no arrow to turn it (it stops there
  for good), and a patrol arrow no patrol reaches (patrol.py).

Maps a mod uses as network arenas (docs/MOD_ASSETS_AND_ARENAS.md 3.5) are
checked with the ARENA profile, chosen by campaign membership rather than slot:
exactly one player start, no enemies (an error: in a network game they spawn
but cannot be killed), and a warning for floor the start cannot reach (players
reappear on random free cells).

Every diagnostic has a stable `OBC-*` code and a cell, so the GUI can put the
cursor on the problem.
"""

from __future__ import annotations

from dataclasses import dataclass

from . import patrol, rules
from .catalog import Catalog
from .document import MapDocument
from .errors import Diagnostic, Severity
from .planes import coordinates, linear_index
from .reachability import PLAYER_STARTS, analyze, unreachable_floor

EMPTY_OBJECT = rules.EMPTY_OBJECT
MAP_SIZE = 64
#: Engine limits, used when no catalog is given (the catalog's `limits` win).
DEFAULT_LIMITS = {"max_doors": 64, "max_actors": 100, "max_statics": 224, "max_anim_walls": 32}
#: Plane-1 codes that spawn an animated-wall record.
ANIM_CODES = frozenset({86, 87, 88, 103, 106, 107, 124})
#: The spawn table runs 19-317 (jump table 1823:06c3); past it the original jumps wild.
LAST_SPAWN_CODE = 317
DOOR_LOOK_FRAMES = frozenset(v for base in rules.DOOR_LOOK_BASES for v in range(base, base + 4))


def _is_floor(value: int) -> bool:
    return rules.is_floor(value)


def _where(x: int, y: int) -> str:
    return f"cell ({x}, {y})"


@dataclass(frozen=True)
class Profile:
    """What kind of map is being checked."""

    key: str
    name: str
    #: How many player starts are right. None means "do not check".
    starts: int | None = 1
    needs_exit: bool = True
    reports_unreachable: bool = True
    #: False for mod arenas: enemies there cannot be killed.
    enemies: bool = True
    unreachable_severity: Severity = Severity.INFORMATION


SINGLE_PLAYER = Profile("single_player", "Single-player floor")
#: Network arenas (original slots 50-59): exits work for everyone, no quota.
NETWORK = Profile("network", "Network arena", starts=None, needs_exit=False,
                  reports_unreachable=False)
#: A mod's own network arena (format 2): one start as the reference point.
ARENA = Profile("arena", "Network arena (mod)", starts=1, needs_exit=False,
                reports_unreachable=True, enemies=False,
                unreachable_severity=Severity.WARNING)
PROFILES = {profile.key: profile for profile in (SINGLE_PLAYER, NETWORK, ARENA)}
NETWORK_SLOTS = frozenset(range(50, 60))
#: EC7Edit's name for the same idea.
DEATHMATCH = NETWORK


def profile_for_slot(slot: int) -> Profile:
    return NETWORK if slot in NETWORK_SLOTS else SINGLE_PLAYER


def profile_for_map(document: MapDocument, arena_maps=()) -> Profile:
    """A map the campaign uses as an arena is checked as one, whatever its slot."""
    if document.uuid in arena_maps:
        return ARENA
    return profile_for_slot(document.slot)


LOCAL_CODES = frozenset({
    "OBC-BOUNDARY-001", "OBC-CELL-002", "OBC-THING-001", "OBC-THING-002", "OBC-THING-003",
    "OBC-DOOR-001", "OBC-DOOR-002", "OBC-DOOR-003", "OBC-DOOR-004", "OBC-PUSH-001",
    "OBC-PUSH-002", "OBC-AMBUSH-001", "OBC-WALL-002", "OBC-EXIT-002", "OBC-RELEASE-001",
    "OBC-ARENA-001",
})
GLOBAL_CODES = frozenset({
    "OBC-SIZE-001", "OBC-START-001", "OBC-START-002", "OBC-LIMIT-001", "OBC-LIMIT-002",
    "OBC-LIMIT-003", "OBC-LIMIT-004", "OBC-EXIT-001", "OBC-KILL-001", "OBC-UNREACH-001",
    "OBC-PATROL-001", "OBC-PATROL-002",
})


def validate_local(document: MapDocument, catalog: Catalog | None = None, *,
                   profile: Profile = SINGLE_PLAYER,
                   previous: list[Diagnostic] | None = None) -> list[Diagnostic]:
    """The rules whose answer is local, plus the previous result for the rest."""
    fresh = validate_map(document, catalog, profile=profile, only=LOCAL_CODES)
    local = [p for p in fresh if p.code in LOCAL_CODES]
    stale = [p for p in (previous or []) if p.code not in LOCAL_CODES]
    return _sorted(local + stale)


def _sorted(problems):
    return sorted(problems, key=lambda p: (-p.severity.value, p.code, p.cell or (0, 0)))


def _limits(catalog: Catalog | None) -> dict:
    limits = dict(DEFAULT_LIMITS)
    if catalog is not None:
        limits.update({k: v for k, v in catalog.limits.items() if k in limits})
    return limits


def _kind(catalog: Catalog | None, plane: int, value: int) -> str:
    if catalog is None:
        return ""
    return catalog.kind(plane, value)


def validate_map(document: MapDocument, catalog: Catalog | None = None, *,
                 profile: Profile = SINGLE_PLAYER,
                 only: frozenset[str] | None = None) -> list[Diagnostic]:
    """Check one map. Returns diagnostics worst first.

    Maps imported from the game are judged more gently where the original
    itself breaks a rule (the shipped maps have things inside walls).
    """
    problems: list[Diagnostic] = []

    def add(code, severity, message, x=None, y=None, fix="", detail=""):
        if only is not None and code not in only:
            return
        cell = (x, y) if x is not None else None
        problems.append(Diagnostic(code, severity, message,
                                   _where(x, y) if cell else "", cell, fix, detail))

    imported = document.source is not None and bool(document.source.sha256)
    preserved = Severity.WARNING if imported else Severity.ERROR
    width, height = document.width, document.height
    plane0 = document.planes.planes[0]
    plane1 = document.planes.planes[1]
    limits = _limits(catalog)

    if (width, height) != (MAP_SIZE, MAP_SIZE):
        add("OBC-SIZE-001", Severity.ERROR,
            f"the map is {width}x{height}; Operation Body Count only plays 64x64 maps")

    # -- the outer boundary ----------------------------------------------
    for index, value in enumerate(plane0):
        x, y = coordinates(index, width)
        if (x in (0, width - 1) or y in (0, height - 1)) and value >= rules.AREATILE:
            add("OBC-BOUNDARY-001", Severity.ERROR,
                "the map edge is open here; the player could walk out of the world",
                x, y, fix="seal_boundary")

    # -- cells ---------------------------------------------------------
    doors = anims = statics = actors = 0
    starts = []
    bad_words: dict[int, list[tuple[int, int]]] = {}
    for index in range(width * height):
        x, y = coordinates(index, width)
        w0, w1 = plane0[index], plane1[index]
        if w0 == 0 or w0 > rules.LAST_FLOOR:
            bad_words.setdefault(w0, []).append((x, y))
        if rules.is_door(w0):
            doors += 1
            for problem in rules.check_door(document, x, y):
                add(problem.code, problem.severity, problem.message, x, y, fix=problem.fix)
        if w1 in PLAYER_STARTS:
            starts.append((x, y))
        if w1 in ANIM_CODES:
            anims += 1
        kind = _kind(catalog, 1, w1)
        if 23 <= w1 <= 85:
            statics += 1
        if kind in ("enemy", "ally", "fire") or (catalog is None and 108 <= w1 <= LAST_SPAWN_CODE):
            actors += 1
        if not profile.enemies and (kind == "enemy"
                                    or (catalog is None and 108 <= w1 <= LAST_SPAWN_CODE)):
            add("OBC-ARENA-001", Severity.ERROR,
                "an enemy in a network arena: it spawns but cannot be killed in a network "
                "game, which also leaves the elevator exit unusable", x, y)
        if w1 > LAST_SPAWN_CODE and w1 != 0xFFFF:
            add("OBC-THING-002", Severity.ERROR,
                f"plane-1 word {w1} is past the original's spawn table (19-{LAST_SPAWN_CODE})",
                x, y)
        elif w1 >= 19 and catalog is not None and catalog.for_value(1, w1) is None \
                and not catalog.record(1, w1):
            add("OBC-THING-003", Severity.WARNING,
                f"plane-1 word {w1} means nothing to the game; it is ignored", x, y)
        solid = w0 < rules.AREATILE and not rules.is_door(w0) and w0 != rules.AMBUSH
        if solid and (w1 in PLAYER_STARTS or 23 <= w1 <= 85
                      or kind in ("enemy", "ally", "fire")):
            add("OBC-THING-001", preserved,
                f"plane-1 word {w1} stands inside a wall and will be stuck there", x, y)
        if w1 == rules.PUSHWALL:
            for problem in rules.check_pushwall(document, x, y):
                # The original leaves a few 98s on open floor: inert there.
                severity = preserved if problem.code == "OBC-PUSH-001" else problem.severity
                add(problem.code, severity, problem.message, x, y)
        if w0 == rules.AMBUSH and not (kind == "enemy" or (catalog is None and 108 <= w1)):
            add("OBC-AMBUSH-001", Severity.WARNING,
                "an ambush marker with no enemy standing on it stays a solid cell", x, y)
        if w1 in rules.OPENABLE_MARKERS and rules.is_wall(w0) and w0 not in DOOR_LOOK_FRAMES:
            add("OBC-WALL-002", Severity.WARNING,
                f"marker {w1} slides this wall open, but wall {w0} has no door-look "
                "animation frames; it will animate through unrelated walls", x, y)
        if w0 in rules.EXITS and not any(
            0 <= nx < width and 0 <= ny < height
            and _is_floor(plane0[linear_index(nx, ny, width)])
            for nx, ny in rules.neighbours(x, y)
        ):
            add("OBC-EXIT-002", Severity.WARNING,
                "an exit with no floor beside it cannot be used", x, y)
        if w1 == 71:
            add("OBC-RELEASE-001", Severity.INFORMATION,
                "object 71 is passable in the floppy release and blocks in the CD release",
                x, y)

    for word, cells in sorted(bad_words.items()):
        x, y = cells[0]
        more = f" ({len(cells)} cells)" if len(cells) > 1 else ""
        add("OBC-CELL-002", Severity.ERROR,
            f"plane-0 word {word} is not a wall, door or floor the engine knows "
            f"(floors are {rules.AREATILE}-{rules.LAST_FLOOR}){more}", x, y)

    # -- whole-map counts ----------------------------------------------
    if profile.starts is not None:
        if not starts:
            add("OBC-START-001", Severity.ERROR, "the map has no player start")
        elif len(starts) > profile.starts:
            for x, y in starts[1:]:
                add("OBC-START-002", Severity.ERROR,
                    f"the map has {len(starts)} player starts; the game uses exactly one",
                    x, y)
    if doors > limits["max_doors"]:
        add("OBC-LIMIT-001", Severity.ERROR,
            f"{doors} doors; the engine holds at most {limits['max_doors']}")
    if actors + 1 > limits["max_actors"]:
        add("OBC-LIMIT-002", Severity.ERROR,
            f"{actors} actors plus the player; the engine holds at most {limits['max_actors']}")
    if statics > limits["max_statics"]:
        add("OBC-LIMIT-003", Severity.ERROR,
            f"{statics} objects; the engine holds at most {limits['max_statics']}")
    if anims > limits["max_anim_walls"]:
        add("OBC-LIMIT-004", Severity.ERROR,
            f"{anims} animated walls; the engine holds at most {limits['max_anim_walls']}")

    # -- reachability (advisory) ---------------------------------------
    wants_reach = only is None or bool({"OBC-EXIT-001", "OBC-KILL-001", "OBC-UNREACH-001"} & only)
    if wants_reach and (profile.needs_exit or profile.reports_unreachable) and starts:
        reach = analyze(document, catalog)
        if not profile.needs_exit:
            pass
        elif not reach.exits and not reach.victor_reached:
            add("OBC-EXIT-001", Severity.WARNING,
                "no exit (ladder, stairs or elevator) can be reached from the player start")
        elif reach.exits and not reach.victor_reached and len(reach.enemies_reached) < reach.quota:
            add("OBC-KILL-001", Severity.WARNING,
                f"the exit needs {reach.quota} kills, but only {len(reach.enemies_reached)} "
                f"of {len(reach.enemies)} enemies can be reached")
        if profile.reports_unreachable:
            cut = unreachable_floor(document, reach)
            if cut:
                x, y = coordinates(cut[0], width)
                why = ("; network players reappear on random free cells, so one could "
                       "land there" if not profile.enemies else "")
                add("OBC-UNREACH-001", profile.unreachable_severity,
                    f"{len(cut)} floor cells cannot be reached from the start{why}", x, y)

    # -- patrols (advisory) --------------------------------------------
    if only is None or {"OBC-PATROL-001", "OBC-PATROL-002"} & only:
        for problem in patrol_problems(document, catalog, imported=imported):
            add(problem.code, problem.severity, problem.message, *problem.cell)

    return _sorted(problems)


def patrol_problems(document: MapDocument, catalog: Catalog | None = None, *,
                    imported: bool = False) -> list[Diagnostic]:
    """Patrols that stop for good, and arrows no patrol ever reads.

    Follows every moving patroller with the engine's rule (patrol.py). A
    patroller that walks into a wall away from an arrow stands there for the
    rest of the floor, which is rarely what was meant; an arrow that no patrol
    reaches does nothing (allies told to disperse also read arrows, but they
    start wherever the player leads them, so they are not followed here).

    Both are warnings on a new map and notes on one imported from the game,
    where the original's own floors do both.
    """
    severity = Severity.INFORMATION if imported else Severity.WARNING
    grid = patrol.Grid(document, catalog)
    problems = []
    read: set[tuple[int, int]] = set()
    for x, y, facing in patrol.patrollers(document, catalog):
        walk = patrol.follow(grid, x, y, facing)
        read |= walk.arrows
        if walk.stops_at is None:
            continue
        name = "this patroller"
        if catalog is not None:
            entry = catalog.for_value(1, document.planes.at(1, x, y))
            if entry is not None:
                name = entry.name.split(",")[0]
        sx, sy = walk.stops_at
        where = "where it starts" if (sx, sy) == (x, y) else f"at ({sx}, {sy})"
        heading = patrol.ARROW_ORDER[walk.stop_direction]
        bx, by = walk.blocked_by
        message = (f"{name} stops for good {where}: heading {heading}, it walks into "
                   f"{walk.reason} at ({bx}, {by}) with no arrow to turn it")
        word = document.planes.at(1, bx, by) if grid.inside(bx, by) else 0
        blocker = catalog.kind(1, word) if catalog is not None else ""
        if blocker == "enemy" or (catalog is None and 108 <= word <= 317):
            # Standing enemies leave their posts once they see the player, so
            # this one may be gone by the time the patrol gets there.
            problems.append(Diagnostic("OBC-PATROL-001", Severity.INFORMATION,
                                       message + ", unless that enemy has moved by then",
                                       _where(x, y), (x, y)))
            continue
        problems.append(Diagnostic("OBC-PATROL-001", severity, message, _where(x, y), (x, y)))
    for x, y in patrol.arrow_cells(document):
        if (x, y) not in read:
            problems.append(Diagnostic(
                "OBC-PATROL-002", severity,
                "no patrolling enemy's route reaches this arrow, so it turns nobody",
                _where(x, y), (x, y)))
    return problems


# -- safe repairs ---------------------------------------------------------


def _fix_seal_boundary(document: MapDocument) -> list[tuple[int, int, int, int]]:
    """Paint the open cells of the outer ring as explosion-proof wall."""
    width, height = document.width, document.height
    writes = []
    for index, value in enumerate(document.planes.planes[0]):
        x, y = coordinates(index, width)
        if (x in (0, width - 1) or y in (0, height - 1)) and value >= rules.AREATILE:
            writes.append((0, x, y, document.SOLID_WALL))
            writes.append((1, x, y, document.BORDER_OBJECT))
    return writes


def _fix_door_axes(document: MapDocument) -> list[tuple[int, int, int, int]]:
    """Give every door whose walls say otherwise the code for the other axis."""
    writes = []
    for x, y in rules.door_cells(document):
        value = document.planes.at(0, x, y)
        axis = rules.door_axis(document, x, y)
        if axis.two_sided and axis.vertical != rules.door_is_vertical(value):
            writes.append((0, x, y, rules.door_value(rules.door_lock(value), axis.vertical)))
    return writes


FIXES = {
    "seal_boundary": ("Seal the map boundary", _fix_seal_boundary),
    "door-axis": ("Turn doors to match their walls", _fix_door_axes),
}


def fix_writes(fix: str, document: MapDocument) -> list[tuple[int, int, int, int]]:
    entry = FIXES.get(fix)
    return entry[1](document) if entry else []


def fix_label(fix: str) -> str:
    entry = FIXES.get(fix)
    return entry[0] if entry else ""


def errors_in(problems) -> list[Diagnostic]:
    return [p for p in problems if p.severity is Severity.ERROR]


def summarize(problems) -> str:
    """One line for a status bar."""
    errors = sum(1 for problem in problems if problem.severity is Severity.ERROR)
    warnings = sum(1 for problem in problems if problem.severity is Severity.WARNING)
    if not problems:
        return "No problems found"
    parts = []
    if errors:
        parts.append(f"{errors} error{'s' if errors > 1 else ''}")
    if warnings:
        parts.append(f"{warnings} warning{'s' if warnings > 1 else ''}")
    if not parts:
        return f"{len(problems)} note{'s' if len(problems) > 1 else ''}"
    return ", ".join(parts)
