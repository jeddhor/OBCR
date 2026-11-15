# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Shared helpers for the GUI tests: synthetic game data and fake engines.

Nothing here is the game's. The data files are the synthetic fixtures from
scripts/make_fixtures.py (right names, right shapes, no retail bytes), and a
"fake engine" is a short Python script standing in for `obcr`, so the
playtest and mod-check paths run on any machine -- Windows included -- with
no game and no display.
"""

from __future__ import annotations

import os
import sys
import textwrap
from pathlib import Path

EDITOR = Path(__file__).resolve().parents[2]
for _path in (EDITOR, EDITOR / "tests" / "unit"):
    if str(_path) not in sys.path:
        sys.path.insert(0, str(_path))

import obc_support  # noqa: E402

#: The interpreter for fake engines. On Windows a venv's python.exe is a
#: launcher that starts the real interpreter as a child; killing the launcher
#: would leave that child running, so the base interpreter is used directly
#: (fake engines need only the standard library).
PYTHON = getattr(sys, "_base_executable", "") or sys.executable


def make_game_data(directory: Path) -> Path:
    """A synthetic game directory OBCEdit accepts: MAPHEAD/MAPTEMP, GFXTILES, COUNT.EXE."""
    fixtures = obc_support.make_fixtures.fixture_set()
    directory.mkdir(parents=True, exist_ok=True)
    for name in ("MAPHEAD.BC", "MAPTEMP.BC", "GFXTILES.BC", "COUNT.EXE"):
        (directory / name).write_bytes(fixtures[f"synthetic.{name}"])
    return directory


def fake_script(directory: Path, body: str, name: str = "fake_obcr") -> Path:
    """Write a Python script whose `main(argv)` is `body` (indented for you)."""
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / f"{name}.py"
    script.write_text(
        "import os, sys, time\n"
        "argv = sys.argv[1:]\n"
        "def out(*lines):\n"
        "    for line in lines:\n"
        "        print(line, flush=True)\n"
        + textwrap.dedent(body) + "\n",
        encoding="utf-8")
    return script


def fake_engine(directory: Path, body: str, name: str = "fake_obcr") -> Path:
    """An executable standing in for `obcr`: a launcher that runs `body`.

    A `.cmd` on Windows and a shell script elsewhere, because the editor
    starts the engine by path. Use it for short-lived calls (`--version`,
    `--check-data`, `--check-mod`); for a long-running playtest, start
    PYTHON on the script directly so stopping it stops the script.
    """
    script = fake_script(directory, body, name)
    if os.name == "nt":
        launcher = directory / f"{name}.cmd"
        launcher.write_text(f'@"{PYTHON}" "{script}" %*\r\n@exit /b %errorlevel%\r\n',
                            encoding="ascii")
    else:
        launcher = directory / name
        launcher.write_text(f'#!/bin/sh\nexec "{PYTHON}" "{script}" "$@"\n', encoding="utf-8")
        launcher.chmod(0o755)
    return launcher


#: A fake OBCR that knows the editor's contract: --version, --check-data,
#: --check-mod (accepting everything), and a playtest that records its
#: arguments into `launched.txt` beside itself and exits.
OBCR_CONTRACT = r'''
here = os.path.dirname(os.path.abspath(__file__))
if "--version" in argv:
    out("obcr 1.0.0-beta.2")
    sys.exit(0)
if "--check-data" in argv:
    out("release\tOBC-A\tOperation Body Count (synthetic)", "complete\tyes")
    sys.exit(0)
if "--check-mod" in argv:
    with open(os.path.join(here, "checked.txt"), "a", encoding="utf-8") as log:
        log.write(" ".join(argv) + "\n")
    out("mod\tobcedit-preview\tOBCEdit playtest\t1.0\t1", "result\tok")
    sys.exit(0)
with open(os.path.join(here, "launched.txt"), "a", encoding="utf-8") as log:
    log.write("\t".join(argv) + "\n")
out("OBCR 1.0.0-beta.2 playing")
sys.exit(0)
'''
