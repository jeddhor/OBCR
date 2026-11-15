# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Operation Body Count's placement rules, mirrored from the engine.

Sources: `docs/re/LEVEL_CODES.md` (sections 2-4) and the engine's
`SetupGameLevel`, `SpawnDoor` and `MovePWalls` ports in `crates/obc-sim`.

**Plane 0 splits at 140.** Words below 140 are solid (`tilemap`); 140-161 are
open floor, the area being `word - 140` (22 areas). 130-137 are doors and
138 an ambush marker, which the engine turns into floor under a standing enemy.

**A door's axis is in its word.** Unlike Corridor 7, where the engine decides
by topology, OBC's door codes come in pairs: even = vertical (the slab runs
north-south, you pass east-west, walls to the north and south), odd =
horizontal. The engine copies the area from the west (vertical) or north
(horizontal) neighbour, so that side must be floor. The door tool picks the
code from the neighbouring walls; the validator checks the word agrees.

**A push wall moves exactly two cells** away from whoever pushes it, and
stops early only if the player stands in the way; at rest it cannot be pushed
again (LEVEL_CODES section 4.7).
"""

from __future__ import annotations

from dataclasses import dataclass

from .errors import Diagnostic, Severity

AREATILE = 140
NUM_AREAS = 22
LAST_FLOOR = AREATILE + NUM_AREAS - 1
AMBUSH = 138
FIRST_DOOR = 130
LAST_DOOR = 137
PUSHWALL = 98
EMPTY_OBJECT = 18
#: Door-look walls (first frames) and the plane-1 words that make them open.
DOOR_LOOK_BASES = (13, 29, 49, 65, 69)
OPENABLE_MARKERS = (103, 106)
EXITS = {113: "ladder", 114: "stairs", 115: "elevator"}
#: Plane-1 107: always open, thin (pipe mouths, rock holes).
OPEN_PASSAGE = 107

_STEPS = ((0, -1), (1, 0), (0, 1), (-1, 0))  # N, E, S, W


def is_floor(value: int) -> bool:
    """An open cell with a valid area (140-161)."""
    return AREATILE <= value <= LAST_FLOOR


is_open = is_floor


def is_door(value: int) -> bool:
    return FIRST_DOOR <= value <= LAST_DOOR


def is_wall(value: int) -> bool:
    """A solid wall word (1-129): any texture, including special walls."""
    return 1 <= value <= 129


def door_value(lock: int, vertical: bool) -> int:
    """The plane-0 word for a door of look `lock` (0-3) and axis."""
    return FIRST_DOOR + 2 * lock + (0 if vertical else 1)


def door_lock(value: int) -> int:
    return (value - FIRST_DOOR) // 2


def door_is_vertical(value: int) -> bool:
    return (value - FIRST_DOOR) % 2 == 0


def _word(document, plane: int, x: int, y: int) -> int | None:
    if 0 <= x < document.width and 0 <= y < document.height:
        return document.planes.at(plane, x, y)
    return None


def _solid(value: int | None) -> bool:
    return value is not None and value < AREATILE and not is_door(value)


@dataclass(frozen=True)
class DoorAxis:
    """What the neighbours say about a door's orientation."""

    walls_north_south: bool
    walls_east_west: bool
    open_east_west: bool
    open_north_south: bool

    @property
    def vertical(self) -> bool:
        """Vertical (pass east-west) unless only the other way fits."""
        if self.walls_north_south and self.open_east_west:
            return True
        if self.walls_east_west and self.open_north_south:
            return False
        return self.walls_north_south or not self.walls_east_west

    @property
    def two_sided(self) -> bool:
        """Walls on both sides of the slab and floor on both passage sides."""
        return (self.walls_north_south and self.open_east_west) or (
            self.walls_east_west and self.open_north_south
        )

    @property
    def label(self) -> str:
        return "vertical (pass east-west)" if self.vertical else "horizontal (pass north-south)"


def door_axis(document, x: int, y: int) -> DoorAxis:
    def opened(dx, dy):
        value = _word(document, 0, x + dx, y + dy)
        return value is not None and (is_floor(value) or value == AMBUSH)

    def walled(dx, dy):
        return _solid(_word(document, 0, x + dx, y + dy))

    return DoorAxis(
        walls_north_south=walled(0, -1) and walled(0, 1),
        walls_east_west=walled(-1, 0) and walled(1, 0),
        open_east_west=opened(-1, 0) and opened(1, 0),
        open_north_south=opened(0, -1) and opened(0, 1),
    )


def door_for_cell(document, x: int, y: int, lock: int) -> int:
    """The door word of look `lock` whose axis fits the cell's neighbours."""
    return door_value(lock, door_axis(document, x, y).vertical)


def check_door(document, x: int, y: int) -> list[Diagnostic]:
    """Problems with the door at (x, y): axis, edges, frame."""
    value = document.planes.at(0, x, y)
    where = f"cell ({x}, {y})"
    problems = []
    if x in (0, document.width - 1) or y in (0, document.height - 1):
        problems.append(Diagnostic("OBC-DOOR-002", Severity.ERROR,
                                   "a door on the map edge leads out of the world", where,
                                   cell=(x, y)))
        return problems
    vertical = door_is_vertical(value)
    if vertical:
        passage = [_word(document, 0, x - 1, y), _word(document, 0, x + 1, y)]
        frame = [_word(document, 0, x, y - 1), _word(document, 0, x, y + 1)]
    else:
        passage = [_word(document, 0, x, y - 1), _word(document, 0, x, y + 1)]
        frame = [_word(document, 0, x - 1, y), _word(document, 0, x + 1, y)]
    if not all(v is not None and (is_floor(v) or v == AMBUSH) for v in passage):
        axis = door_axis(document, x, y)
        if axis.two_sided and axis.vertical != vertical:
            problems.append(Diagnostic(
                "OBC-DOOR-001", Severity.ERROR,
                f"door {value} is {'vertical' if vertical else 'horizontal'}, but its walls "
                f"say {axis.label}; use {door_value(door_lock(value), axis.vertical)}",
                where, cell=(x, y), fix="door-axis",
            ))
        else:
            problems.append(Diagnostic(
                "OBC-DOOR-003", Severity.ERROR,
                "a door needs open floor on both sides it opens to "
                f"({'west and east' if vertical else 'north and south'})",
                where, cell=(x, y),
            ))
    elif not all(_solid(v) for v in frame):
        problems.append(Diagnostic(
            "OBC-DOOR-004", Severity.WARNING,
            "a door should have walls on both sides of its slab "
            f"({'north and south' if vertical else 'west and east'})",
            where, cell=(x, y),
        ))
    return problems


def door_cells(document, catalog=None) -> list[tuple[int, int]]:
    """Every door cell, in reading order."""
    cells = []
    for y, row in enumerate(document.planes.rows(0)):
        for x, value in enumerate(row):
            if is_door(value):
                cells.append((x, y))
    return cells


def pushwall_directions(document, x: int, y: int) -> list[int]:
    """Directions (0 N, 1 E, 2 S, 3 W) a push wall at (x, y) can travel.

    The player stands on the far side and pushes; the wall needs the two cells
    ahead of it open, as `MovePWalls` makes it move exactly two.
    """
    out = []
    for direction, (dx, dy) in enumerate(_STEPS):
        behind = _word(document, 0, x - dx, y - dy)
        ahead = [_word(document, 0, x + dx * k, y + dy * k) for k in (1, 2)]
        if behind is not None and is_floor(behind) and all(
            v is not None and is_floor(v) for v in ahead
        ):
            out.append(direction)
    return out


def pushwall_path(x: int, y: int, direction: int) -> list[tuple[int, int]]:
    """The two cells a push wall at (x, y) travels through when pushed `direction`."""
    dx, dy = _STEPS[direction]
    return [(x + dx, y + dy), (x + 2 * dx, y + 2 * dy)]


def check_pushwall(document, x: int, y: int) -> list[Diagnostic]:
    where = f"cell ({x}, {y})"
    if not is_wall(document.planes.at(0, x, y)):
        return [Diagnostic("OBC-PUSH-001", Severity.ERROR,
                           "push wall marker 98 is not on a wall; there is nothing to push",
                           where, cell=(x, y))]
    if not pushwall_directions(document, x, y):
        return [Diagnostic("OBC-PUSH-002", Severity.WARNING,
                           "this push wall cannot move: no side has floor to push from "
                           "with two open floor cells ahead", where, cell=(x, y))]
    return []


def neighbours(x: int, y: int):
    for dx, dy in _STEPS:
        yield x + dx, y + dy


__all__ = [
    "AMBUSH", "AREATILE", "DoorAxis", "EMPTY_OBJECT", "EXITS", "LAST_FLOOR", "NUM_AREAS",
    "PUSHWALL", "check_door", "check_pushwall", "door_axis", "door_cells", "door_for_cell",
    "door_is_vertical", "door_lock", "door_value", "is_door", "is_floor", "is_open", "is_wall",
    "neighbours", "pushwall_directions", "pushwall_path",
]
