# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Shared helpers for OBCEdit's unit tests: fixtures, small maps, real data."""

from __future__ import annotations

import importlib.util
import os
import sys
from pathlib import Path

EDITOR = Path(__file__).resolve().parents[2]
REPO = EDITOR.parent
if str(EDITOR) not in sys.path:
    sys.path.insert(0, str(EDITOR))

from obcedit_core.catalog import default_catalog  # noqa: E402
from obcedit_core.document import MapDocument, new_uuid  # noqa: E402
from obcedit_core.names import NativeName  # noqa: E402
from obcedit_core.planes import MapPlanes  # noqa: E402

_spec = importlib.util.spec_from_file_location("make_fixtures", EDITOR / "scripts" / "make_fixtures.py")
make_fixtures = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(make_fixtures)

CATALOG = default_catalog()
WALL, FLOOR, EMPTY, BORDER = 1, 144, 18, 89


def write_archive(directory: Path, prefix: str = "synthetic") -> Path:
    """Write fixture `<prefix>.MAPHEAD.BC/.MAPTEMP.BC` into `directory` under their real names."""
    fixtures = make_fixtures.fixture_set()
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "MAPHEAD.BC").write_bytes(fixtures[f"{prefix}.MAPHEAD.BC"])
    (directory / "MAPTEMP.BC").write_bytes(fixtures[f"{prefix}.MAPTEMP.BC"])
    return directory


def room(text: list[str], *, name: str = "TEST", slot: int = 0, size: int = 64) -> MapDocument:
    """A map from a picture, placed at the top-left of a walled size x size map.

    Characters: `#` wall 1, `.` floor 144, `S` player start facing east on
    floor, `E` elevator exit 115, `D` door (axis left for the test to set:
    130), `P` push wall (wall 1 + 98), `e` standing soldier facing east (116),
    `A` ally (224), `B` brute/Victor (214), `L` door-look wall 69 + 103.
    """
    cells = size * size
    p0 = [WALL] * cells
    p1 = [EMPTY] * cells
    for y in range(size):
        for x in range(size):
            if x in (0, size - 1) or y in (0, size - 1):
                p1[y * size + x] = BORDER
    for y, row in enumerate(text):
        for x, ch in enumerate(row):
            i = y * size + x
            if ch == "#":
                p0[i] = WALL
            elif ch in ".SeAB":
                p0[i] = FLOOR
                p1[i] = {"S": 20, "e": 116, "A": 224, "B": 214}.get(ch, EMPTY)
            elif ch == "E":
                p0[i] = 115
            elif ch == "D":
                p0[i] = 130
            elif ch == "P":
                p0[i], p1[i] = WALL, 98
            elif ch == "L":
                p0[i], p1[i] = 69, 103
    return MapDocument(new_uuid(), slot, NativeName.from_text(name),
                       MapPlanes(size, size, (tuple(p0), tuple(p1), (0,) * cells)))


def real_data(release: str = "full") -> Path | None:
    """The developer's own game data, for data-gated tests (never copied into a test)."""
    env = os.environ.get("OBC_DATA_DIR") if release == "full" else os.environ.get("OBC_CD_DATA_DIR")
    candidates = [Path(env)] if env else []
    candidates.append(REPO / ".local" / "data" / f"obc-{release}")
    for candidate in candidates:
        if (candidate / "MAPTEMP.BC").is_file() or (candidate / "maptemp.bc").is_file():
            return candidate
    return None


def engine() -> Path | None:
    """A built `obcr` from this checkout, for engine-gated tests."""
    for build in ("debug", "release"):
        for name in ("obcr.exe", "obcr"):
            path = REPO / "target" / build / name
            if path.is_file():
                return path
    return None
