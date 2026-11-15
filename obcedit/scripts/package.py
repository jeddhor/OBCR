#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Freeze OBCEdit into a self-contained folder: the editor, a Python and Qt.

    python scripts/package.py --output ../target/obcedit

writes `<output>/OBCEdit/` holding `OBCEdit[.exe]`, its `_internal/` runtime,
`LICENSE.txt` and `THIRD-PARTY.txt`. The OBCR installers copy that folder to
`<install>/obcedit/`, beside the game, where the editor finds `obcr` and the
game data by itself (obcedit_core.bundle).

PyInstaller onedir, not onefile: EC7Edit measured the two (its E4 ledger) and
they are the same download once compressed, while onefile unpacks itself on
every launch and starts five times slower.

The package is data-free: it holds the editor's own sources and the catalog
generated from OBCR's code, nothing from the game. The check at the end
refuses a package containing an original game file's name.

Run it with the Python that should be frozen into the package (it needs
PySide6 and PyInstaller); `--python` re-runs under another interpreter.
"""

from __future__ import annotations

import argparse
import importlib.metadata as metadata
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

EDITOR = Path(__file__).resolve().parents[1]
REPO = EDITOR.parent
NAME = "OBCEdit"

# Qt modules the editor never imports; leaving them out keeps the package a
# reasonable download. A wrong exclusion shows up at once as an editor that
# will not start, which the smoke test below catches.
EXCLUDES = [
    "tkinter",
    "PySide6.QtWebEngineCore",
    "PySide6.QtWebEngineWidgets",
    "PySide6.QtWebEngineQuick",
    "PySide6.QtQuick",
    "PySide6.QtQml",
    "PySide6.Qt3DCore",
    "PySide6.QtMultimedia",
    "PySide6.QtCharts",
    "PySide6.QtDataVisualization",
    "PySide6.QtPdf",
    "PySide6.QtSql",
]

# Files PyInstaller's Qt hooks pull in that a widgets-only editor never
# loads: Qt Quick/QML/PDF/virtual keyboard, the 20 MB software OpenGL
# renderer, and the network/TLS plugins with their OpenSSL. Removed after the
# freeze; the self-test below builds the real window to prove nothing needed
# went with them.
PRUNE = [
    "**/opengl32sw.dll",
    "**/*Qt6Quick*",
    "**/*Qt6Qml*",
    "**/*Qt6Pdf*",
    "**/*Qt6VirtualKeyboard*",
    "**/*Qt6Network*",
    "**/plugins/tls",
    "**/plugins/networkinformation",
    "**/plugins/platforminputcontexts",
    "**/plugins/imageformats/*pdf*",
    "**/plugins/platforms/*direct2d*",
    "**/libcrypto-3-x64.dll",
    "**/libssl-3-x64.dll",
]

# Files of the original game: none may end up in the package.
GAME_FILES = re.compile(
    r"^(COUNT|BCCD)\.EXE$|^(GFXTILES|MAPHEAD|MAPTEMP|VGAHEAD|VGADICT|VGAGRAPH|AUDIOHED|AUDIOT|AUDIOMUS|F\d)\.BC$",
    re.IGNORECASE,
)


def obcr_version() -> str:
    """The workspace version from OBCR's Cargo.toml: one product, one number."""
    text = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        raise SystemExit("FAIL: no version in Cargo.toml")
    return m.group(1)


def third_party(path: Path) -> None:
    lines = [
        "Third-party software inside this package",
        "========================================",
        "",
        "OBCEdit itself is GPL-3.0-or-later; see LICENSE.txt. This package also",
        "contains the following, which keep their own terms.",
        "",
    ]
    for name in ("PySide6", "PySide6-Essentials", "PySide6-Addons", "shiboken6"):
        try:
            md = metadata.metadata(name)
        except metadata.PackageNotFoundError:
            continue
        lic = md.get("License-Expression") or md.get("License") or "see the project"
        lines += [
            f"  {name} {md.get('Version')}",
            f"      {lic}",
            "      Qt itself is included under the same terms (LGPL-3.0).",
            "      https://www.qt.io/licensing  https://pypi.org/project/PySide6/",
            "",
        ]
    lines += [
        f"  CPython {sys.version.split()[0]}",
        "      Python Software Foundation License 2.0",
        "      https://docs.python.org/3/license.html",
        "",
        "  PyInstaller runtime (the bootloader in this executable)",
        "      GPL-2.0-or-later with the bootloader exception.",
        "      https://pyinstaller.org/en/stable/license.html",
        "",
        "No part of Operation Body Count is in this package. The game remains",
        "the property of its owners and must be obtained separately.",
        "",
    ]
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--python", help="freeze with this interpreter instead")
    args = ap.parse_args()

    if args.python and Path(args.python).resolve() != Path(sys.executable).resolve():
        return subprocess.call([args.python, __file__, "--output", str(args.output)])

    for mod in ("PySide6", "PyInstaller"):
        try:
            __import__(mod)
        except ImportError:
            print(f"FAIL: {sys.executable} has no {mod} (pip install PySide6 pyinstaller)")
            return 1

    version = obcr_version()
    out = args.output.resolve()
    target = out / NAME
    print(f"{NAME} {version}: freezing with Python {sys.version.split()[0]}")

    stamp = EDITOR / "obcedit_core" / "_build_version.py"
    stamp.write_text(
        f'# Generated by scripts/package.py. Not committed.\nVERSION = "{version}"\n',
        encoding="utf-8",
    )
    work = Path(tempfile.mkdtemp(prefix="obcedit-package-"))
    try:
        shutil.copy2(EDITOR / "resources" / "catalog.json", work / "catalog.json")
        cmd = [
            sys.executable, "-m", "PyInstaller",
            "--noconfirm", "--clean", "--log-level", "WARN",
            "--distpath", str(work / "dist"), "--workpath", str(work / "build"),
            "--specpath", str(work),
            "--name", NAME, "--windowed", "--onedir",
            # SRC is relative to --specpath; DEST follows os.pathsep.
            "--add-data", f"catalog.json{os.pathsep}resources",
            "--hidden-import", "obcedit_gui.application",
            "--hidden-import", "obcedit_core._build_version",
            *[a for m in EXCLUDES for a in ("--exclude-module", m)],
            str(EDITOR / "obcedit"),
        ]
        done = subprocess.run(cmd, cwd=EDITOR)
        if done.returncode != 0:
            print("FAIL: PyInstaller did not finish")
            return 1
        built = work / "dist" / NAME
        exe = built / (NAME + (".exe" if os.name == "nt" else ""))
        if not exe.is_file():
            print(f"FAIL: no {exe.name} in the freeze")
            return 1
        pruned = 0
        for pattern in PRUNE:
            for p in list(built.glob(pattern)):
                pruned += sum(f.stat().st_size for f in ([p] if p.is_file() else p.rglob("*")) if f.is_file())
                shutil.rmtree(p) if p.is_dir() else p.unlink()
        print(f"  pruned {pruned / 1e6:.1f} MB of unused Qt")
        shutil.copy2(REPO / "LICENSE", built / "LICENSE.txt")
        third_party(built / "THIRD-PARTY.txt")
        bad = [p for p in built.rglob("*") if p.is_file() and GAME_FILES.match(p.name)]
        if bad:
            print(f"FAIL: game files in the package: {bad}")
            return 1
        if target.exists():
            shutil.rmtree(target)
        out.mkdir(parents=True, exist_ok=True)
        shutil.move(str(built), str(target))
    finally:
        stamp.unlink(missing_ok=True)
        shutil.rmtree(work, ignore_errors=True)

    # Smoke test: the frozen editor reports itself without a display or data.
    exe = target / (NAME + (".exe" if os.name == "nt" else ""))
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen")
    probe = subprocess.run([str(exe), "--selftest"], capture_output=True, text=True, env=env, timeout=120)
    print(probe.stdout.strip() or probe.stderr.strip())
    if probe.returncode != 0:
        print(f"FAIL: {exe.name} --selftest exited {probe.returncode}")
        return 1
    size = sum(p.stat().st_size for p in target.rglob("*") if p.is_file())
    print(f"wrote {target} ({size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
