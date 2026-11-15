# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Patrol routes: where a patrolling enemy walks, and the arrows that steer it.

The engine's rule (docs/re/ACTORS.md sections 7.2-7.3, docs/re/LEVEL_CODES.md;
ported in crates/obc-sim `select_path_dir`, `t_path`, `try_walk`):

* a patroller starts on its own cell (OBC's SpawnPatrol puts it back there),
  facing the way its code says: east, north, west or south;
* each time it reaches a cell centre, `SelectPathDir` reads plane 1 there: a
  patrol arrow (90-97: east, northeast, north, northwest, west, southwest,
  south, southeast) turns it to that direction; anything else leaves it
  facing as it was;
* then `TryWalk` tests the next cell. A straight move is blocked by a wall,
  a blocking object or a standing (shootable) actor, and passes doors, which
  the patroller opens; a diagonal move needs the cell and both cells beside
  the corner free of walls, doors, blocking objects and standing actors;
* a blocked move leaves it with no direction, and away from an arrow it
  never gets one again: it stands there for good (until it sees the player).

So a route is a set of arrows on the cells where the walk turns. This module
plans routes from clicked cells (`plan_route`) and follows patrols the way
the engine would (`follow`), for the validator.

What the model leaves out, deliberately: other patrollers and allies (they
move, so they block only for a moment), the player, and objects that move
during play. Static 71 blocks in the CD release only, and is treated as
blocking so a route works in both. Hovering saucers (speed 0) never move.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import rules
from .catalog import ARROW_ORDER, Catalog

#: Plane-1 codes 90-97 (ICONARROWS), in Wolf3D `dirtype` order.
FIRST_ARROW = 90
#: (dx, dy) for each direction index: E, NE, N, NW, W, SW, S, SE.
STEPS = ((1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1))
#: Where a patrol actor's code says it faces (+0 east, +1 north, +2 west,
#: +3 south) as a direction index.
CARDINAL = {"east": 0, "north": 2, "west": 4, "south": 6}

#: Patrolling actors when there is no catalog (LEVEL_CODES section 5): the
#: first code of each group of four (east, north, west, south).
_PATROL_GROUPS = (112, 148, 184, 120, 156, 192, 220, 238, 256, 274, 292, 310)
_HOVERING_GROUPS = (270, 288, 306)


def arrow_value(direction: int, catalog: Catalog | None = None) -> int:
    """The plane-1 word of the arrow pointing `direction` (0 E .. 7 SE)."""
    if catalog is not None:
        entry = catalog.by_key("patrol-arrow")
        if entry is not None and entry.directions:
            return entry.value_for(ARROW_ORDER[direction])
    return FIRST_ARROW + direction


def arrow_direction(value: int) -> int | None:
    """The direction an arrow word turns a patroller, or None for other words."""
    spot = value - FIRST_ARROW
    return spot if 0 <= spot < 8 else None


def direction_between(a: tuple[int, int], b: tuple[int, int]) -> int | None:
    """The direction from cell `a` to cell `b`, if they lie on one straight or
    45-degree line (and differ)."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    if (dx, dy) == (0, 0):
        return None
    if dx != 0 and dy != 0 and abs(dx) != abs(dy):
        return None
    step = ((dx > 0) - (dx < 0), (dy > 0) - (dy < 0))
    return STEPS.index(step)


def _record(catalog: Catalog | None, value: int) -> dict:
    return catalog.record(1, value) if catalog is not None else {}


def patrol_facing(value: int, catalog: Catalog | None = None) -> int | None:
    """The start direction of a patrolling actor's code, or None when `value`
    is not a patroller that moves (standing actors, hovering saucers, objects)."""
    if catalog is not None:
        record = catalog.record(1, value)
        if not record.get("patrol") or "hovering" in str(record.get("name", "")):
            return None
        return CARDINAL.get(str(record.get("facing", "")))
    for first in _PATROL_GROUPS:
        if first <= value < first + 4:
            return (value - first) * 2
    return None


def is_hovering(value: int, catalog: Catalog | None = None) -> bool:
    if catalog is not None:
        record = catalog.record(1, value)
        return bool(record.get("patrol")) and "hovering" in str(record.get("name", ""))
    return any(first <= value < first + 4 for first in _HOVERING_GROUPS)


def turned_to(value: int, direction: int, catalog: Catalog | None = None) -> int | None:
    """The same patroller's code facing `direction`, or None if it cannot
    (actors face only east, north, west or south)."""
    names = {index: name for name, index in CARDINAL.items()}
    if direction not in names:
        return None
    if catalog is not None:
        entry = catalog.for_value(1, value)
        if entry is None:
            return None
        for name, raw in entry.directions:
            if name == names[direction]:
                return raw
        return None
    for first in _PATROL_GROUPS:
        if first <= value < first + 4:
            return first + direction // 2
    return None


class Grid:
    """What stands in a patroller's way on one map, worked out once."""

    def __init__(self, document, catalog: Catalog | None = None) -> None:
        self.document = document
        self.catalog = catalog
        self.width, self.height = document.width, document.height
        self.plane0 = document.planes.planes[0]
        self.plane1 = document.planes.planes[1]

    def inside(self, x: int, y: int) -> bool:
        return 0 <= x < self.width and 0 <= y < self.height

    def word(self, plane: int, x: int, y: int) -> int:
        return (self.plane0 if plane == 0 else self.plane1)[y * self.width + x]

    def wall(self, x: int, y: int) -> bool:
        """Solid in `tilemap`: walls, push walls, exits, ambush markers (under
        a standing enemy they become floor, but the enemy blocks instead)."""
        if not self.inside(x, y):
            return True
        value = self.word(0, x, y)
        return not (rules.is_floor(value) or rules.is_door(value))

    def door(self, x: int, y: int) -> bool:
        return self.inside(x, y) and rules.is_door(self.word(0, x, y))

    def obstacle(self, x: int, y: int) -> str:
        """Why a cell's plane-1 word blocks a patroller, or "" when it does not."""
        if not self.inside(x, y):
            return "the map edge"
        value = self.word(1, x, y)
        if value < 19:
            return ""
        record = _record(self.catalog, value)
        if self.catalog is None:
            # Statics 23-85 might block; standing actors do. Without the
            # catalog there is no telling which statics block, so none do.
            if 108 <= value <= 317 and patrol_facing(value) is None and not is_hovering(value):
                return "a standing enemy"
            return ""
        kind = record.get("kind", "")
        if kind == "static" and (record.get("blocking") or record.get("blocking_cd")):
            return str(record.get("name", "an object"))
        if kind == "enemy" and not record.get("patrol"):
            return str(record.get("name", "a standing enemy"))
        if kind == "enemy" and is_hovering(value, self.catalog):
            # Shootable and never moves.
            return str(record.get("name", "a hovering saucer"))
        return ""

    def blocked(self, x: int, y: int, direction: int) -> tuple[int, int, str] | None:
        """`TryWalk` from (x, y) towards `direction`: None when it can, else the
        cell that stops it and why."""
        dx, dy = STEPS[direction]
        tx, ty = x + dx, y + dy
        if direction % 2 == 0:
            # CHECKSIDE: walls and blocking objects stop it; doors open.
            if self.wall(tx, ty):
                return tx, ty, "a wall"
            why = self.obstacle(tx, ty)
            return (tx, ty, why) if why else None
        # CHECKDIAG on the target and both cells beside the corner.
        for cx, cy in ((tx, ty), (tx, y), (x, ty)):
            if self.wall(cx, cy):
                return cx, cy, "a wall"
            if self.door(cx, cy):
                return cx, cy, "a door (a diagonal step cannot pass one)"
            why = self.obstacle(cx, cy)
            if why:
                return cx, cy, why
        return None


# -- following a patrol ------------------------------------------------------


@dataclass
class Patrol:
    """Where one patroller goes."""

    start: tuple[int, int]
    facing: int
    #: Every cell it walks through, in order, starting with its own.
    cells: list[tuple[int, int]] = field(default_factory=list)
    #: Arrow cells it reads.
    arrows: set[tuple[int, int]] = field(default_factory=set)
    #: True when it walks the same loop forever.
    loops: bool = False
    #: Where it stops for good, the direction it was trying, and why.
    stops_at: tuple[int, int] | None = None
    stop_direction: int | None = None
    blocked_by: tuple[int, int] | None = None
    reason: str = ""


def follow(grid: Grid, x: int, y: int, facing: int) -> Patrol:
    """Walk a patroller from its cell the way the engine would."""
    patrol = Patrol((x, y), facing, cells=[(x, y)])
    direction = facing
    seen: set[tuple[int, int, int]] = set()
    while True:
        turn = arrow_direction(grid.word(1, x, y))
        if turn is not None:
            direction = turn
            patrol.arrows.add((x, y))
        if (x, y, direction) in seen:
            patrol.loops = True
            return patrol
        seen.add((x, y, direction))
        stop = grid.blocked(x, y, direction)
        if stop is not None:
            patrol.stops_at = (x, y)
            patrol.stop_direction = direction
            patrol.blocked_by = stop[:2]
            patrol.reason = stop[2]
            return patrol
        dx, dy = STEPS[direction]
        x, y = x + dx, y + dy
        patrol.cells.append((x, y))


def patrollers(document, catalog: Catalog | None = None) -> list[tuple[int, int, int]]:
    """(x, y, facing) of every patroller that moves, in reading order."""
    out = []
    width = document.width
    for index, value in enumerate(document.planes.planes[1]):
        facing = patrol_facing(value, catalog)
        if facing is not None:
            out.append((index % width, index // width, facing))
    return out


def arrow_cells(document) -> list[tuple[int, int]]:
    width = document.width
    return [(i % width, i // width) for i, v in enumerate(document.planes.planes[1])
            if arrow_direction(v) is not None]


# -- planning a route ---------------------------------------------------------


@dataclass
class Route:
    """A planned route: what to write, or why it cannot be drawn."""

    waypoints: tuple[tuple[int, int], ...] = ()
    #: Every cell the route passes through, in order.
    cells: tuple[tuple[int, int], ...] = ()
    #: (x, y, plane-1 word) of each arrow the route needs.
    arrows: tuple[tuple[int, int, int], ...] = ()
    #: Everything to write, as (plane, x, y, value).
    writes: tuple[tuple[int, int, int, int], ...] = ()
    closed: bool = False
    #: The patroller on the first cell, turned to face the first leg:
    #: (x, y, old word, new word), or None.
    turned: tuple[int, int, int, int] | None = None
    ok: bool = False
    reason: str = ""
    bad_cell: tuple[int, int] | None = None

    @property
    def complete(self) -> bool:
        """At least one leg, and nothing wrong with it."""
        return self.ok and len(self.waypoints) >= 2


def _refuse(route: Route, reason: str, cell) -> Route:
    route.ok = False
    route.reason = reason
    route.bad_cell = cell
    return route


def _name(catalog: Catalog | None, value: int) -> str:
    entry = catalog.for_value(1, value) if catalog is not None else None
    if entry is not None:
        return entry.name
    record = _record(catalog, value)
    return str(record.get("name", f"plane-1 word {value}"))


def plan_route(document, cells, catalog: Catalog | None = None) -> Route:
    """Arrows for a patrol through the clicked `cells`, or the reason it cannot go.

    Consecutive cells must lie on one horizontal, vertical or 45-degree line;
    the route walks straight from each to the next. An arrow goes on the first
    cell and on every cell where the direction changes, pointing along the
    next leg. A route whose last cell is its first is closed: the arrow on the
    first cell then also catches the patroller coming round again.

    The first cell may be a patrolling enemy's own cell. That cell cannot hold
    an arrow (one word per cell), so the enemy must already face the first leg
    -- if it does not and the leg is straight, the route turns it to face it --
    and a closed route must pass straight through it.
    """
    points: list[tuple[int, int]] = []
    for cell in cells:
        cell = (int(cell[0]), int(cell[1]))
        if not points or points[-1] != cell:
            points.append(cell)
    route = Route(waypoints=tuple(points))
    if not points:
        return _refuse(route, "click the cell where the route starts", None)
    grid = Grid(document, catalog)
    for x, y in points:
        if not grid.inside(x, y):
            return _refuse(route, "the route leaves the map", (x, y))
        if grid.wall(x, y):
            return _refuse(route, f"({x}, {y}) is a wall; a route runs over floor", (x, y))

    start = points[0]
    start_word = grid.word(1, *start)
    owner_facing = patrol_facing(start_word, catalog)
    if start_word >= 19 and owner_facing is None and arrow_direction(start_word) is None:
        if is_hovering(start_word, catalog):
            return _refuse(route, "a hovering saucer never moves; use a moving one", start)
        if 108 <= start_word <= 317 or _record(catalog, start_word).get("kind") in ("enemy", "ally"):
            return _refuse(route, f"{_name(catalog, start_word)} does not patrol; "
                                  "start on a patrolling enemy or on an empty cell", start)
        return _refuse(route, f"({start[0]}, {start[1]}) holds {_name(catalog, start_word)}; "
                              "an arrow needs the cell to itself", start)

    # The legs, cell by cell, with the engine's walking rule. `walked` keeps
    # the direction the patroller has as it enters each cell after the first.
    directions: list[int] = []
    path = [start]
    walked: list[tuple[tuple[int, int], int]] = []
    for a, b in zip(points, points[1:]):
        direction = direction_between(a, b)
        if direction is None:
            return _refuse(route, "a route runs straight: horizontally, vertically or at 45 "
                                  "degrees between clicks", b)
        x, y = a
        while (x, y) != b:
            stop = grid.blocked(x, y, direction)
            if stop is not None:
                return _refuse(route, f"the way is blocked at ({stop[0]}, {stop[1]}) by {stop[2]}",
                               stop[:2])
            dx, dy = STEPS[direction]
            x, y = x + dx, y + dy
            path.append((x, y))
            walked.append(((x, y), direction))
        directions.append(direction)
    route.cells = tuple(path)
    route.closed = len(points) >= 3 and points[-1] == points[0]
    owner = owner_facing is not None

    # Where the arrows go: the first cell, and every cell where the walk turns.
    arrows: dict[tuple[int, int], int] = {}
    if directions:
        if route.closed and owner and directions[-1] != directions[0]:
            return _refuse(route, "the enemy's own cell cannot hold an arrow, so a route that "
                                  "closes there must pass straight through it; start the "
                                  "route at a corner instead", start)
        if not owner:
            arrows[start] = directions[0]
        for i in range(1, len(directions)):
            if directions[i] != directions[i - 1]:
                if points[i] in arrows and arrows[points[i]] != directions[i]:
                    return _refuse(route, "a route cannot turn twice on one cell", points[i])
                arrows[points[i]] = directions[i]
    # Every time the walk enters an arrow cell, the arrow decides where it goes
    # next, so the route must leave that way: a route cannot cross itself where
    # it turns.
    for k, (cell, _direction) in enumerate(walked):
        if cell not in arrows:
            continue
        if k + 1 < len(walked):
            leaves = walked[k + 1][1]
        else:
            leaves = directions[0] if route.closed else None
        if leaves is not None and leaves != arrows[cell]:
            return _refuse(route, f"the route crosses itself at ({cell[0]}, {cell[1]}), "
                                  "where it also turns", cell)

    writes: list[tuple[int, int, int, int]] = []
    placed: list[tuple[int, int, int]] = []
    for (x, y), direction in arrows.items():
        word = grid.word(1, x, y)
        if word >= 19 and arrow_direction(word) is None:
            if patrol_facing(word, catalog) is not None:
                return _refuse(route, "an arrow cannot go on an enemy's own cell; turn on the "
                                      "cell before or after it", (x, y))
            return _refuse(route, f"({x}, {y}) holds {_name(catalog, word)}; an arrow needs "
                                  "the cell to itself", (x, y))
        value = arrow_value(direction, catalog)
        placed.append((x, y, value))
        if word != value:
            writes.append((1, x, y, value))

    # An arrow already on a cell the route only passes through would turn the
    # patroller off it, unless it points the way the route goes anyway.
    for (x, y), direction in walked:
        if (x, y) in arrows or (x, y) == start:
            continue
        existing = arrow_direction(grid.word(1, x, y))
        if existing is not None and existing != direction:
            return _refuse(route, f"an arrow at ({x}, {y}) already turns patrols another way; "
                                  "erase it or turn the route there", (x, y))

    if owner and directions and owner_facing != directions[0]:
        new = turned_to(start_word, directions[0], catalog)
        if new is None:
            return _refuse(route, "an enemy starts facing east, north, west or south; begin "
                                  "with a straight leg", start)
        route.turned = (start[0], start[1], start_word, new)
        writes.append((1, start[0], start[1], new))

    route.arrows = tuple(placed)
    route.writes = tuple(writes)
    route.ok = True
    return route


__all__ = [
    "CARDINAL", "FIRST_ARROW", "Grid", "Patrol", "Route", "STEPS", "arrow_cells",
    "arrow_direction", "arrow_value", "direction_between", "follow", "is_hovering",
    "patrol_facing", "patrollers", "plan_route", "turned_to",
]
