# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Snapshot: an exact OBCR frame of the map you are drawing.

Not an approximation of the renderer, and deliberately not one: a second
renderer written in Python would be a second authority on what Operation Body
Count looks like. So the picture comes from OBCR itself, headless:

    obcr --data DATA --mod MOD --play N --start X,Y,DIR --shot TICS --shot-out FILE.png

The player stands on tile (X, Y) facing a compass direction, the game runs
`TICS` tics (past the level's fade-in) and writes the frame.

The camera is checked before the engine is asked (a tile outside the map or in
a wall is a picture of nothing), and the cache key covers everything that
could change the picture: the engine binary, the game data, the exported mod,
the camera, the tic and the render profile.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path

from .document import MapDocument
from .errors import Diagnostic, ObcEditError, Severity

#: Classic: the original 320x200 picture. Enhanced shots follow the player's
#: own settings and so are not reproducible; the snapshot pins Classic.
RENDER_PROFILE = {"mode": "classic"}

#: A floor fades in over its first ~25 tics; 30 is past it and before anything
#: wanders off (verified against OBCR: earlier frames are nearly black).
SNAPSHOT_TIC = 30


def _error(message: str) -> ObcEditError:
    return ObcEditError(Diagnostic("OBC-SNAPSHOT-001", Severity.ERROR, message))


@dataclass(frozen=True)
class Camera:
    """Where the picture is taken from, and which way it looks.

    Tile coordinates, and degrees counter-clockwise from east (0 east, 90
    north, 180 west, 270 south). OBCR's `--start` takes a compass direction,
    so the angle is rounded to the nearest quarter turn.
    """

    x: float
    y: float
    angle: float = 0.0

    def normalized(self) -> "Camera":
        quarter = round((self.angle % 360.0) / 90.0) % 4
        return Camera(float(int(self.x)), float(int(self.y)), quarter * 90.0)

    @property
    def direction(self) -> int:
        """`--start`'s DIR: 0 N, 1 E, 2 S, 3 W."""
        return {0.0: 1, 90.0: 0, 180.0: 3, 270.0: 2}[self.normalized().angle]

    def arguments(self) -> list[str]:
        camera = self.normalized()
        return ["--start", f"{int(camera.x)},{int(camera.y)},{self.direction}"]

    _COMPASS = {0.0: "east", 90.0: "north", 180.0: "west", 270.0: "south"}

    def describe(self) -> str:
        camera = self.normalized()
        return f"({camera.x:g}, {camera.y:g}) facing {self._COMPASS[camera.angle]}"


def check_camera(document: MapDocument, camera: Camera) -> None:
    """Refuse a camera that cannot produce a picture. Raises, or returns."""
    from .rules import is_floor

    for value, name in ((camera.x, "x"), (camera.y, "y"), (camera.angle, "angle")):
        if value != value or value in (float("inf"), float("-inf")):
            raise _error(f"the camera's {name} is not a number")
    tile_x, tile_y = int(camera.x), int(camera.y)
    if not (0 <= camera.x and 0 <= camera.y
            and tile_x < document.width and tile_y < document.height):
        raise _error(
            f"the camera at {camera.describe()} is outside this "
            f"{document.width}x{document.height} map")
    if not is_floor(document.cell(0, tile_x, tile_y)):
        raise _error(
            f"the camera at {camera.describe()} is inside a wall; "
            "put it on floor the player could stand on")


def _digest_file(path: Path | str) -> str:
    try:
        return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    except OSError:
        return "missing"


def snapshot_key(*, engine: Path | str, data_fingerprint: str, export_digest: str,
                 camera: Camera, tic: int = SNAPSHOT_TIC, pk3: Path | str = "") -> str:
    """What this picture is a picture of. (`pk3` is accepted and ignored: EC7Edit's.)"""
    camera = camera.normalized()
    parts = [
        "obcr-v1",
        _digest_file(engine),
        data_fingerprint or "no-data",
        export_digest or "no-export",
        f"{camera.x:g},{camera.y:g},{camera.angle:g}",
        str(tic),
        "|".join(f"{k}={v}" for k, v in sorted(RENDER_PROFILE.items())),
    ]
    return hashlib.sha256("\n".join(parts).encode("utf-8")).hexdigest()


def snapshot_arguments(camera: Camera, output: Path | str, tic: int = SNAPSHOT_TIC) -> list[str]:
    """The engine arguments for one snapshot, beyond `--data/--mod/--play`."""
    return [*camera.arguments(), "--shot", str(tic), "--shot-out", str(output), "--classic"]


def looks_like_a_world(png: Path | str, *, view_fraction: float = 0.5) -> bool:
    """Whether a PNG holds a rendered world rather than a blank frame.

    Counts distinct colours in the upper part of the frame (world, not status
    bar) using the editor's own PNG reader, so no imaging library is needed.
    """
    from .imagery import read_png

    try:
        width, height, rgb = read_png(png)
    except Exception:  # noqa: BLE001 - unreadable means no picture
        return False
    rows = max(1, int(height * view_fraction))
    channels = len(rgb) // (width * height) if width * height else 3
    colors = set()
    for y in range(rows):
        row = rgb[y * width * channels:(y + 1) * width * channels]
        for x in range(0, len(row), channels * 4):
            colors.add(bytes(row[x:x + 3]))
            if len(colors) >= 8:
                return True
    return False
