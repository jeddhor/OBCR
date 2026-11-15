# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Compound structures: the things that are more than one word.

Operation Body Count builds many features from *pairs* of words on different
planes: a door-look wall is a wall frame on plane 0 with 103 (or 106) on
plane 1, a glass pane is wall 57 with window marker 104, a fan is wall 73 with
a speed 86-88, an ambush is floor marker 138 under a standing enemy. Painting
the raw words one at a time is how a map ends up with a marker on open floor
and no wall to push. A prefab writes every word at once, as one undo step,
and checks first that the cells can take it.

The words come from `docs/re/LEVEL_CODES.md` (sections 3-4); each prefab
records the section it follows (`evidence`).

Doors are a special case: their axis is part of the word (rules.py), so a door
prefab is placed with `placement_on(document, x, y)`, which picks the even or
odd code from the neighbouring walls.
"""

from __future__ import annotations

from dataclasses import dataclass, replace

from . import rules
from .document import MapDocument
from .errors import Diagnostic, Severity
from .planes import linear_index

EMPTY_OBJECT = rules.EMPTY_OBJECT
#: A plain wall for prefabs that need one and have no better choice.
DEFAULT_WALL = 1
#: What erasing a structure puts back on plane 0: area 4, floor set 0.
DEFAULT_FLOOR = MapDocument.DEFAULT_FLOOR

is_floor = rules.is_floor
is_wall = rules.is_wall


@dataclass(frozen=True)
class Write:
    """One word, at an offset from the prefab's anchor."""

    plane: int
    dx: int
    dy: int
    value: int

    def rotated(self, quarter_turns: int) -> "Write":
        dx, dy = self.dx, self.dy
        for _ in range(quarter_turns % 4):
            dx, dy = -dy, dx
        return Write(self.plane, dx, dy, self.value)


@dataclass(frozen=True)
class Precondition:
    """What must be true of a cell before a prefab may cover it."""

    dx: int
    dy: int
    #: "floor", "wall", "any", "empty" (nothing on plane 1), or "reachable"
    #: (at least one orthogonal neighbour is floor).
    requires: str
    why: str

    def holds(self, document: MapDocument, x: int, y: int) -> bool:
        cx, cy = x + self.dx, y + self.dy
        if not (0 <= cx < document.width and 0 <= cy < document.height):
            return False
        index = linear_index(cx, cy, document.width)
        plane0 = document.planes.planes[0][index]
        plane1 = document.planes.planes[1][index]
        if self.requires == "floor":
            return is_floor(plane0)
        if self.requires == "wall":
            return is_wall(plane0)
        if self.requires == "empty":
            return plane1 < 19
        if self.requires == "reachable":
            for nx, ny in rules.neighbours(cx, cy):
                if 0 <= nx < document.width and 0 <= ny < document.height and is_floor(
                    document.planes.planes[0][linear_index(nx, ny, document.width)]
                ):
                    return True
            return False
        return True

    def rotated(self, quarter_turns: int) -> "Precondition":
        dx, dy = self.dx, self.dy
        for _ in range(quarter_turns % 4):
            dx, dy = -dy, dx
        return Precondition(dx, dy, self.requires, self.why)


@dataclass(frozen=True)
class Prefab:
    """One compound structure, complete enough to place, check, and undo."""

    key: str
    name: str
    description: str
    category: str
    writes: tuple[Write, ...]
    preconditions: tuple[Precondition, ...] = ()
    diagnostic: str = "OBC-CELL-004"
    erase_to: tuple[Write, ...] = ()
    rotatable: bool = False
    advanced: bool = False
    evidence: str = ""
    notes: str = ""
    #: For doors: the look (0-3); the axis is chosen where it is placed.
    door_lock: int | None = None

    def __post_init__(self) -> None:
        if not self.writes:
            raise ValueError(f"{self.key}: a prefab with no writes is not a prefab")
        if not self.evidence:
            raise ValueError(f"{self.key}: every prefab records where its words came from")

    @property
    def footprint(self) -> tuple[tuple[int, int], ...]:
        cells = {(write.dx, write.dy) for write in self.writes}
        cells |= {(check.dx, check.dy) for check in self.preconditions}
        return tuple(sorted(cells, key=lambda cell: (cell[1], cell[0])))

    def rotated(self, quarter_turns: int) -> "Prefab":
        if not self.rotatable or quarter_turns % 4 == 0:
            return self
        return replace(
            self,
            writes=tuple(write.rotated(quarter_turns) for write in self.writes),
            preconditions=tuple(c.rotated(quarter_turns) for c in self.preconditions),
        )

    def check(self, document: MapDocument, x: int, y: int) -> list[Diagnostic]:
        problems = []
        for cell in self.footprint:
            cx, cy = x + cell[0], y + cell[1]
            if not (0 <= cx < document.width and 0 <= cy < document.height):
                problems.append(Diagnostic(
                    "OBC-BOUNDARY-001", Severity.ERROR,
                    f"{self.name} does not fit here; it would reach outside the map",
                    f"cell ({cx}, {cy})", cell=(cx, cy),
                ))
                return problems
        for check in self.preconditions:
            if not check.holds(document, x, y):
                cx, cy = x + check.dx, y + check.dy
                problems.append(Diagnostic(
                    self.diagnostic, Severity.ERROR, f"{self.name} needs {check.why}",
                    f"cell ({cx}, {cy})", cell=(cx, cy),
                ))
        if self.door_lock is not None and not problems:
            axis = rules.door_axis(document, x, y)
            if not axis.two_sided:
                problems.append(Diagnostic(
                    "OBC-DOOR-003", Severity.ERROR,
                    f"{self.name} needs walls on two opposite sides and floor on the "
                    "other two", f"cell ({x}, {y})", cell=(x, y),
                ))
        return problems

    def placement(self, x: int, y: int) -> list[tuple[int, int, int, int]]:
        """The `(plane, x, y, value)` writes for an anchor at `(x, y)`."""
        return [(w.plane, x + w.dx, y + w.dy, w.value) for w in self.writes]

    def placement_on(self, document: MapDocument, x: int, y: int) -> list[tuple[int, int, int, int]]:
        """Like `placement`, resolving anything that depends on the map (a door's axis)."""
        writes = self.placement(x, y)
        if self.door_lock is None:
            return writes
        code = rules.door_for_cell(document, x, y, self.door_lock)
        return [(p, wx, wy, code if p == 0 and (wx, wy) == (x, y) else v)
                for p, wx, wy, v in writes]

    def removal(self, x: int, y: int) -> list[tuple[int, int, int, int]]:
        if self.erase_to:
            return [(w.plane, x + w.dx, y + w.dy, w.value) for w in self.erase_to]
        writes = []
        for cell in self.footprint:
            writes.append((0, x + cell[0], y + cell[1], DEFAULT_FLOOR))
            writes.append((1, x + cell[0], y + cell[1], EMPTY_OBJECT))
        return writes


_REACH = Precondition(0, 0, "reachable", "floor beside it, to reach it from")
_TO_FLOOR = (Write(0, 0, 0, DEFAULT_FLOOR), Write(1, 0, 0, EMPTY_OBJECT))


def _door(lock: int, name: str) -> Prefab:
    return Prefab(
        key=f"prefab.door.{lock + 1}", name=name,
        description="A door. Its axis follows the walls around it: walls north and "
                    "south make a door you walk through east-west. The look is "
                    "cosmetic; Operation Body Count has no locked doors.",
        category="specials", writes=(Write(0, 0, 0, rules.door_value(lock, True)),
                                     Write(1, 0, 0, EMPTY_OBJECT)),
        erase_to=_TO_FLOOR, door_lock=lock,
        evidence="LEVEL_CODES 3.4: 130-137, even vertical, lock = (code - 130) / 2",
    )


def _door_look(base: int, marker: int) -> Prefab:
    see = marker == 106
    return Prefab(
        key=f"prefab.doorlook.{base}.{marker}",
        name=f"Sliding panel {base}{' (see-through)' if see else ''}",
        description="A wall that slides open when used and stays open until used again; "
                    "enemies open it by walking into it. "
                    + ("Enemies can see through it." if see else "It blocks sight when shut."),
        category="specials", writes=(Write(0, 0, 0, base), Write(1, 0, 0, marker)),
        preconditions=(_REACH,), erase_to=_TO_FLOOR,
        evidence="LEVEL_CODES 3.3 and 4.4: door-look walls 13/29/49/65/69 with plane-1 "
                 "103 (opaque) or 106 (see-through)",
        notes=f"Walls {base + 1}-{base + 3} are its animation frames and are not paint.",
    )


def _wall_with(key, name, wall, marker, description, evidence, *, reach=True,
               category="specials", erase_wall=None) -> Prefab:
    writes = [Write(0, 0, 0, wall)]
    if marker is not None:
        writes.append(Write(1, 0, 0, marker))
    return Prefab(
        key=key, name=name, description=description, category=category, writes=tuple(writes),
        preconditions=(_REACH,) if reach else (),
        erase_to=(Write(0, 0, 0, erase_wall), Write(1, 0, 0, EMPTY_OBJECT))
        if erase_wall is not None else _TO_FLOOR,
        evidence=evidence,
    )


def _marker_on_wall(key, name, marker, description, evidence, *, advanced=False) -> Prefab:
    """A plane-1 marker on whatever wall is already there."""
    return Prefab(
        key=key, name=name, description=description, category="walls",
        writes=(Write(1, 0, 0, marker),),
        preconditions=(Precondition(0, 0, "wall", "a wall to mark"),),
        erase_to=(Write(1, 0, 0, EMPTY_OBJECT),), evidence=evidence, advanced=advanced,
    )


PREFABS: tuple[Prefab, ...] = (
    *(_door(lock, f"Door, look {lock + 1}") for lock in range(4)),
    *(_door_look(base, marker) for base in rules.DOOR_LOOK_BASES for marker in (103, 106)),
    _marker_on_wall(
        "prefab.pushwall", "Push wall (secret)", rules.PUSHWALL,
        "The wall slides exactly two cells away from whoever uses it, then stays put "
        "for good. Counts as a secret. Needs floor on the side you push from and two "
        "open cells ahead.",
        "LEVEL_CODES 4.7: plane-1 98, MovePWalls moves it exactly two cells",
    ),
    *(_wall_with(f"prefab.exit.{kind}", f"Exit: {kind}", code, None,
                 "Leaves the floor when used, once half the floor's enemies are dead. "
                 + ("Plays the elevator sequence." if code == 115 else ""),
                 "LEVEL_CODES 3.3: exits 113 ladder, 114 stairs, 115 elevator (exitkind 1)")
      for code, kind in rules.EXITS.items()),
    _wall_with("prefab.generator.on", "Generator switch (lights on)", 95, None,
               "Using it turns the lights off (GENERATORS OFF); again turns them on.",
               "LEVEL_CODES 3.3: 95/96 toggled by Cmd_Use; 95 -> 96 = GENERATORS OFF"),
    _wall_with("prefab.generator.off", "Generator switch (lights off)", 96, None,
               "A floor with this switch starts dark; using it turns the lights on.",
               "start_play_loop: a 96 tile on the floor means it starts dark"),
    _wall_with("prefab.glass", "Glass pane", 57, 104,
               "See-through for you, blocks enemy sight; an explosion breaks it.",
               "LEVEL_CODES 3.3 and 4.6: 57 with window marker 104", reach=False),
    _wall_with("prefab.screen", "Screen pane", 117, 104,
               "A second glass pane, broken by explosions.",
               "LEVEL_CODES 3.3 and 4.6: 117 with window marker 104", reach=False),
    *(_wall_with(f"prefab.fan.{speed}", f"Porthole fan ({speed})", 73, marker,
                 "A spinning porthole fan; it stays solid.",
                 "LEVEL_CODES 4.4: wall 73 with plane-1 86/87/88", reach=False)
      for marker, speed in ((86, "slow"), (87, "medium"), (88, "fast"))),
    _wall_with("prefab.rock-opening", "Rock opening (sewer)", 88, rules.OPEN_PASSAGE,
               "A hole in the rock you can walk through; using it heals 25 below your "
               "healing limit, as in the original.",
               "LEVEL_CODES 3.3 and 4.4: wall 88 with plane-1 107", reach=False),
    _marker_on_wall("prefab.explosion-proof", "Explosion-proof wall", 89,
                    "Grenades cannot damage this wall. Every original map marks its border "
                    "this way.", "LEVEL_CODES 4.8: plane-1 89"),
    _marker_on_wall("prefab.window", "See-through wall (window)", 104,
                    "The wall is drawn see-through; blocks enemy sight.",
                    "LEVEL_CODES 4.6: plane-1 104", advanced=True),
    _marker_on_wall("prefab.grille", "See-through wall (grille)", 105,
                    "The wall is drawn see-through; enemies see through it too.",
                    "LEVEL_CODES 4.6: plane-1 105", advanced=True),
    Prefab(
        key="prefab.ambush.soldier", name="Ambush: soldier",
        description="A soldier who waits where he stands until he sees you. The floor "
                    "under him is an ambush marker the game clears to the neighbouring "
                    "floor's area.",
        category="enemies", writes=(Write(0, 0, 0, rules.AMBUSH), Write(1, 0, 0, 116)),
        preconditions=(Precondition(0, 0, "floor", "open floor"),), erase_to=_TO_FLOOR,
        evidence="LEVEL_CODES 2 step 9 and 4.5: 138 under a SpawnStand actor",
    ),
    Prefab(
        key="prefab.squad", name="Squad of three allies",
        description="Three allies in a row facing east, as the original lines them up at "
                    "the start of a floor.",
        category="starts",
        writes=tuple(Write(1, dx, 0, 224) for dx in range(3)),
        preconditions=tuple(Precondition(dx, 0, "floor", "open floor") for dx in range(3)),
        erase_to=tuple(Write(1, dx, 0, EMPTY_OBJECT) for dx in range(3)),
        rotatable=True,
        evidence="LEVEL_CODES 4.5: 224-227 SpawnAlly",
    ),
)


def by_key(key: str) -> Prefab | None:
    for prefab in PREFABS:
        if prefab.key == key:
            return prefab
    return None


def in_category(category: str) -> list[Prefab]:
    return [prefab for prefab in PREFABS if prefab.category == category]


def placeable(*, include_advanced: bool = False) -> list[Prefab]:
    return [p for p in PREFABS if include_advanced or not p.advanced]
