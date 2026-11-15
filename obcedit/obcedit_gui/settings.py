# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""User settings: paths, profiles, window layout, recent projects.

Kept deliberately small and deliberately *local*. Profiles live here and never
in a project file, because a project is something people share and a profile
is a set of paths into somebody's own machine. A shared project refers to a
profile id and an expected data fingerprint; resolving that to real paths is
this side of the boundary, on the machine that owns them.

Nothing secret is stored, and no retail content: paths, fingerprints, window
geometry, and a list of recently opened projects.
"""

from __future__ import annotations

import json
from pathlib import Path

from PySide6.QtCore import QByteArray, QSettings

from obcedit_core.discovery import Profile

ORGANIZATION = "OBCR"
APPLICATION = "OBCEdit"

_RECENT_LIMIT = 10


def obcr_mods_dir() -> Path:
    """OBCR's user mods folder (docs/MODS_AND_OBCEDIT.md section 3.4).

    Windows: %APPDATA%\\OBCR\\data\\mods. Elsewhere: $XDG_DATA_HOME/obcr/mods,
    or ~/.local/share/obcr/mods.
    """
    import os
    import sys

    if sys.platform == "win32":
        base = os.environ.get("APPDATA") or str(Path.home() / "AppData" / "Roaming")
        return Path(base) / "OBCR" / "data" / "mods"
    base = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
    return Path(base) / "obcr" / "mods"


class Settings:
    """A typed face over QSettings, so the rest of the GUI never sees strings."""

    def __init__(self, backend: QSettings | None = None) -> None:
        self._settings = backend or QSettings(ORGANIZATION, APPLICATION)

    # -- where the editor keeps its own files -----------------------------

    #: Autosaves live here. Settable so a test can point it somewhere
    #: disposable: a test suite that writes into the user's real
    #: ~/.local/share is a test suite that leaves work behind and reads other
    #: runs' leftovers, which is how two of these first failed.
    @property
    def recovery_dir(self) -> Path:
        raw = self._settings.value("paths/recovery", "")
        if raw:
            return Path(raw)
        from PySide6.QtCore import QStandardPaths

        base = QStandardPaths.writableLocation(QStandardPaths.AppLocalDataLocation)
        return (Path(base) if base else Path.home() / ".local" / "share" / "obcedit") / "recovery"

    @recovery_dir.setter
    def recovery_dir(self, path: Path | str) -> None:
        self._settings.setValue("paths/recovery", str(path))

    #: Where Install in OBCR puts mods: OBCR's own mods folder unless set
    #: (which tests do, so they never write into the real one).
    @property
    def mods_dir(self) -> Path:
        raw = self._settings.value("paths/mods", "")
        return Path(raw) if raw else obcr_mods_dir()

    @mods_dir.setter
    def mods_dir(self, path: Path | str) -> None:
        self._settings.setValue("paths/mods", str(path))

    # -- profiles ---------------------------------------------------------

    @property
    def profile(self) -> Profile:
        raw = self._settings.value("profile/current", "")
        if not raw:
            return Profile()
        try:
            return Profile.from_json(json.loads(raw))
        except (ValueError, TypeError):
            return Profile()  # a corrupt profile is not a reason to fail startup

    @profile.setter
    def profile(self, profile: Profile) -> None:
        self._settings.setValue("profile/current", json.dumps(profile.to_json()))

    @property
    def configured(self) -> bool:
        """Whether first-run setup has been completed at least once."""
        profile = self.profile
        return bool(profile.data_dir and profile.workspace_dir)

    # -- recent projects --------------------------------------------------

    @property
    def recent_projects(self) -> list[str]:
        raw = self._settings.value("recent/projects", "[]")
        try:
            entries = json.loads(raw)
            return [str(entry) for entry in entries][:_RECENT_LIMIT]
        except (ValueError, TypeError):
            return []

    def remember_project(self, path: Path | str) -> None:
        path = str(Path(path).expanduser().resolve())
        entries = [entry for entry in self.recent_projects if entry != path]
        entries.insert(0, path)
        self._settings.setValue("recent/projects", json.dumps(entries[:_RECENT_LIMIT]))

    def forget_project(self, path: Path | str) -> None:
        path = str(Path(path).expanduser().resolve())
        self._settings.setValue(
            "recent/projects",
            json.dumps([entry for entry in self.recent_projects if entry != path]),
        )

    # -- window layout ----------------------------------------------------

    def save_layout(self, geometry: QByteArray, state: QByteArray) -> None:
        self._settings.setValue("window/geometry", geometry)
        self._settings.setValue("window/state", state)

    def layout(self) -> tuple[QByteArray | None, QByteArray | None]:
        return (
            self._settings.value("window/geometry"),
            self._settings.value("window/state"),
        )

    def reset_layout(self) -> None:
        """Forget the saved layout. The fix for a window that ends up off-screen."""
        self._settings.remove("window/geometry")
        self._settings.remove("window/state")

    # -- preferences ------------------------------------------------------

    def value(self, key: str, default=None):
        return self._settings.value(f"preferences/{key}", default)

    def set_value(self, key: str, value) -> None:
        self._settings.setValue(f"preferences/{key}", value)

    def sync(self) -> None:
        self._settings.sync()
