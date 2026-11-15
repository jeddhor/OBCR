# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""OBCEdit core: the Qt-free half of the Operation Body Count level editor.

Nothing in this package may import Qt. The GUI depends on the core; the core
never depends on the GUI, so every format contract stays testable headless and
every editor operation stays scriptable.

The map format is the one OBCR reads (`crates/obc-formats/src/maps.rs`); the
meaning of every map word comes from `resources/catalog.json`, generated from
OBCR's code; the mod format is docs/MODS_AND_OBCEDIT.md section 3.
"""

from __future__ import annotations

__all__ = ["__version__"]


def __getattr__(name: str) -> str:
    """`__version__`, computed the first time anybody reads it.

    PEP 562. The version is OBCR's (version.py), read the first time it is asked
    for rather than at every import.
    """
    if name in ("__version__", "__pep440__"):
        from .version import pep440, version
        return pep440() if name == "__pep440__" else version()
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
