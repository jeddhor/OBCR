# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Can the player get there, and can they finish the floor?

Advisory, and it under-reports rather than inventing errors: this is not a
proof that a floor is completable. The model, from docs/re/LEVEL_CODES.md:

* the flood starts at the player start (plane-1 19-22) and moves through four
  neighbours;
* passable: floor (140-161), doors (130-137; Operation Body Count has no
  locks), ambush markers (138, cleared to floor under their actor), walls
  carrying an openable marker (plane-1 103 or 106, door-look walls the player
  opens with Use) and always-open passages (plane-1 107);
* a push wall (plane-1 98 on a wall) becomes passable once the flood reaches
  the cell it is pushed from, if the two cells ahead of it are floor -- it
  slides exactly two cells (rules.pushwall_directions);
* an exit (plane-0 113/114/115) is reachable when it touches a reached cell;
* the exit opens once half the floor's enemies are dead, so the floor can be
  finished when an exit is reachable and at least half of the enemies stand
  on reached cells. Victor's death (plane-1 214) also ends the game.

Enemies are counted at the hardest game type (all three tiers), as the
original's quota counts what spawned.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import rules
from .catalog import Catalog
from .document import MapDocument
from .planes import coordinates, linear_index

PLAYER_STARTS = (19, 20, 21, 22)
VICTOR = 214


def _is_floor(value: int) -> bool:
    return rules.is_floor(value)


@dataclass
class Reach:
    """What one floor's start can get to."""

    reached: set[int] = field(default_factory=set)
    started: bool = False
    #: Exit cells (plane-0 indices) touching the reached region.
    exits: list[int] = field(default_factory=list)
    #: Plane-1 indices of enemies, and those on reached cells.
    enemies: list[int] = field(default_factory=list)
    enemies_reached: list[int] = field(default_factory=list)
    #: Victor stands somewhere reachable.
    victor_reached: bool = False
    #: Push wall cells the flood went through.
    pushed: list[int] = field(default_factory=list)

    @property
    def quota(self) -> int:
        """The kills an exit needs: killtotal >> 1."""
        return len(self.enemies) >> 1

    @property
    def completable(self) -> bool:
        exit_ok = bool(self.exits) and len(self.enemies_reached) >= self.quota
        return exit_ok or self.victor_reached


def _is_enemy(catalog: Catalog | None, value: int) -> bool:
    if catalog is not None:
        return catalog.kind(1, value) == "enemy"
    return 108 <= value <= 317 and not (224 <= value <= 227 or 242 <= value <= 245
                                        or 260 <= value <= 263)


def analyze(document: MapDocument, catalog: Catalog | None = None) -> Reach:
    width, height = document.width, document.height
    plane0 = document.planes.planes[0]
    plane1 = document.planes.planes[1]
    reach = Reach()
    reach.enemies = [i for i, v in enumerate(plane1) if v >= 19 and _is_enemy(catalog, v)]

    starts = [i for i, v in enumerate(plane1) if v in PLAYER_STARTS]
    if not starts:
        return reach
    reach.started = True

    def passable(index: int) -> bool:
        w0, w1 = plane0[index], plane1[index]
        if _is_floor(w0) or rules.is_door(w0) or w0 == rules.AMBUSH:
            return True
        if w1 in rules.OPENABLE_MARKERS or w1 == rules.OPEN_PASSAGE:
            return True
        return False

    pending = [starts[0]]
    reach.reached.add(starts[0])
    pushwalls = {i for i, v in enumerate(plane1) if v == rules.PUSHWALL and rules.is_wall(plane0[i])}
    while pending:
        index = pending.pop()
        x, y = coordinates(index, width)
        for direction, (dx, dy) in enumerate(((0, -1), (1, 0), (0, 1), (-1, 0))):
            nx, ny = x + dx, y + dy
            if not (0 <= nx < width and 0 <= ny < height):
                continue
            neighbour = linear_index(nx, ny, width)
            if neighbour in reach.reached:
                continue
            if passable(neighbour):
                reach.reached.add(neighbour)
                pending.append(neighbour)
            elif neighbour in pushwalls and direction in rules.pushwall_directions(document, nx, ny):
                reach.reached.add(neighbour)
                reach.pushed.append(neighbour)
                pending.append(neighbour)

    for index, value in enumerate(plane0):
        if value in rules.EXITS:
            x, y = coordinates(index, width)
            for nx, ny in rules.neighbours(x, y):
                if 0 <= nx < width and 0 <= ny < height and linear_index(nx, ny, width) in reach.reached:
                    reach.exits.append(index)
                    break
    reach.enemies_reached = [i for i in reach.enemies if i in reach.reached]
    reach.victor_reached = any(plane1[i] == VICTOR for i in reach.enemies_reached)
    return reach


def unreachable_floor(document: MapDocument, reach: Reach) -> list[int]:
    """Floor cells the start cannot get to (empty when there is no start)."""
    if not reach.started:
        return []
    plane0 = document.planes.planes[0]
    return [i for i, v in enumerate(plane0) if _is_floor(v) and i not in reach.reached]
