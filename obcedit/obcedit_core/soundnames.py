# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""What each digitized sound is used for, so the Resources panel can say.

From OBCR's own code, which names the sound ids it plays (the `snd` modules in
crates/obc-sim/src/actors.rs and player.rs, `sounds` in doors.rs, the
animated-wall sound in animwalls.rs and the menu's in obc-ui menu.rs; their
numbers come from the disassembly, docs/re/ACTORS.md section 9 and
docs/re/PLAYER.md). These are descriptions written for this project, not data
from the game. A number not listed is one the engine's code never plays.
"""

from __future__ import annotations

_GROUPS = (
    ((0, 1), "Swamp creature dies"),
    ((2, 3), "Rat dies"),
    ((4, 5), "Giant rat dies"),
    ((8, 9), "Saucer destroyed"),
    ((10, 11, 12, 13), "Soldier dies"),
    ((14, 15, 27, 28), "Soldier burns to death"),
    ((18, 19), "Red vest dies"),
    ((20, 21), "Gunner dies"),
    ((22, 23, 24, 25, 26), "Ally dies"),
    ((30, 31, 32), "Rat attacks"),
    ((33, 34), "Giant rat attacks"),
    ((35, 36), "Swamp creature attacks"),
    ((38, 39), "Disguised soldier attacks"),
    ((45,), "Weapon 1 fires"),
    ((46,), "Weapon 2 fires"),
    ((47,), "Weapon 3 fires"),
    ((48,), "Weapon 4 fires"),
    ((49,), "Weapon 5 fires"),
    ((51, 53, 54, 55, 56), "Soldier sees you / idles"),
    ((52, 61), "Red vest sees you / idles"),
    ((62,), "Door opens; splat on the visor"),
    ((63,), "Door closes; splat on the visor"),
    ((64, 65, 66, 67, 68), "Wall breaks"),
    ((69, 70), "Wall chips"),
    ((71, 72, 73), "Fire"),
    ((74,), "Explosion"),
    ((75,), "Explosion; booby trap"),
    ((76, 77), "Pain"),
    ((78, 79, 80, 81), "Wall switch"),
    ((83,), "Animated wall"),
    ((84,), "Generator switch"),
    ((85,), "Pickup; menu move"),
)

NAMES: dict[int, str] = {number: text for numbers, text in _GROUPS for number in numbers}


def sound_name(number: int) -> str:
    return NAMES.get(number, "not played by the engine's code")


__all__ = ["NAMES", "sound_name"]
