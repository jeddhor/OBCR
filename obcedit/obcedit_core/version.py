# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""One version number for the editor and the game.

OBCEdit ships with OBCR and is tested against it, so it carries OBCR's version
(`1.0.0-beta.3`, the `[workspace.package] version` in the repository's
`Cargo.toml`) rather than one of its own: one product, one number.

* a packaged build reads `_build_version.py`, written into the bundle when it
  was frozen (there is no Cargo.toml inside a package);
* a checkout reads `../Cargo.toml`;
* anything else falls back to the constant below.

`import obcedit_core` reads nothing; reading `obcedit_core.__version__` does,
once.
"""

from __future__ import annotations

import functools
import re
from pathlib import Path

#: Used when there is neither a stamp nor a Cargo.toml. Bumped with releases.
FALLBACK = "1.0.0-beta.6"

_VERSION = re.compile(r'^\s*version\s*=\s*"([^"]+)"', re.MULTILINE)


def _from_stamp() -> str:
    try:
        from ._build_version import VERSION  # type: ignore[import-not-found]
    except Exception:  # noqa: BLE001 - absent is normal
        return ""
    return str(VERSION)


def _from_cargo() -> str:
    cargo = Path(__file__).resolve().parents[2] / "Cargo.toml"
    try:
        text = cargo.read_text(encoding="utf-8")
    except OSError:
        return ""
    section = text.split("[workspace.package]", 1)
    found = _VERSION.search(section[1]) if len(section) == 2 else None
    return found.group(1) if found else ""


@functools.lru_cache(maxsize=1)
def version() -> str:
    """`1.0.0-beta.N`, the same as the OBCR built from the same tree."""
    return _from_stamp() or _from_cargo() or FALLBACK


def pep440() -> str:
    """The same number spelled for packaging tools: 1.0.0-beta.2 -> 1.0.0b2."""
    return re.sub(r"-beta\.?", "b", version())


__all__ = ["FALLBACK", "pep440", "version"]
