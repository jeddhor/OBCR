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
    "**/plugins/imageformats/*pdf*",
    "**/plugins/platforms/*direct2d*",
    "**/libcrypto-3-x64.dll",
    "**/libssl-3-x64.dll",
]

# Linux only, on top of PRUNE: Qt's translations (the editor is English
# only), the GTK platform theme (Qt's own widgets and the XDG portal theme
# serve instead), and the platforms a desktop editor never runs on (EGLFS,
# framebuffer, VNC, Vulkan display). The libraries only these used are then
# found and removed by prune_orphans. The xcb, Wayland and offscreen
# platforms and the input-method plugins (compose key, IBus) stay.
PRUNE_LINUX = [
    "**/PySide6/Qt/translations",
    "**/plugins/platformthemes/libqgtk3.so",
    "**/plugins/egldeviceintegrations",
    "**/plugins/platforms/libqeglfs.so",
    "**/plugins/platforms/libqminimalegl.so",
    "**/plugins/platforms/libqlinuxfb.so",
    "**/plugins/platforms/libqvnc.so",
    "**/plugins/platforms/libqvkkhrdisplay.so",
    # OpenSSL, as on Windows (PRUNE): the editor makes no network
    # connections, and hashlib falls back to its built-in SHA-256.
    "**/libssl.so*",
    "**/libcrypto.so*",
    "**/lib-dynload/_ssl.*",
    "**/lib-dynload/_hashlib.*",
]

# Libraries prune_orphans keeps even when nothing links them any more: the X
# client libraries are small and Qt's xcb support can load some at run time.
KEEP_ORPHANS = re.compile(r"^libX")

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


def elf_needed(path: Path) -> list[str] | None:
    """The DT_NEEDED names of a 64-bit little-endian ELF file, [] for a file
    that is not ELF, None when it is ELF but cannot be read (the caller then
    removes nothing: an unreadable file might need anything)."""
    import struct

    try:
        data = path.read_bytes()
    except OSError:
        return None
    if data[:4] != b"\x7fELF":
        return []
    if data[4] != 2 or data[5] != 1:
        return None
    try:
        shoff, = struct.unpack_from("<Q", data, 0x28)
        shentsize, shnum = struct.unpack_from("<HH", data, 0x3A)
        sections = [struct.unpack_from("<IIQQQQIIQQ", data, shoff + i * shentsize) for i in range(shnum)]
        names = []
        for _, kind, _, _, off, size, link, _, _, entsize in sections:
            if kind != 6:  # SHT_DYNAMIC
                continue
            stroff = sections[link][4]
            for at in range(off, off + size, entsize or 16):
                tag, val = struct.unpack_from("<qQ", data, at)
                if tag == 0:
                    break
                if tag == 1:  # DT_NEEDED
                    end = data.index(b"\0", stroff + val)
                    names.append(data[stroff + val:end].decode())
        return names
    except (struct.error, IndexError, ValueError, UnicodeDecodeError):
        return None


def linked_libraries(root: Path) -> set[Path] | None:
    """Every shared library under `root` that another ELF file under it
    links, following PyInstaller's top-level links to the real files; None
    when a file could not be read."""
    files = [p for p in root.rglob("*") if p.is_file() and not p.is_symlink()]
    by_name: dict[str, list[Path]] = {}
    for p in files:
        by_name.setdefault(p.name, []).append(p)
    for p in root.rglob("*"):
        if p.is_symlink() and p.exists():
            by_name.setdefault(p.name, []).append(p.resolve())
    linked = set()
    for p in files:
        needed = elf_needed(p)
        if needed is None:
            return None
        for name in needed:
            linked.update(by_name.get(name, []))
    return linked


def prune_orphans(root: Path, before: set[Path] | None) -> int:
    """Removes the libraries that were linked before pruning and are linked
    by nothing now (GTK and the rest that only the GTK theme pulled in).
    Libraries nothing linked to begin with are left alone: they may be
    loaded with dlopen. Returns the bytes removed."""
    removed = 0
    while before is not None:
        now = linked_libraries(root)
        if now is None:
            break
        orphans = [p for p in before - now
                   if p.is_file() and not KEEP_ORPHANS.match(p.name)]
        if not orphans:
            break
        for p in orphans:
            removed += p.stat().st_size
            p.unlink()
            before.discard(p)
    return removed


def remove_dangling_links(root: Path) -> None:
    """Links whose target was pruned (PyInstaller links libraries into the
    top folder on Linux)."""
    for p in list(root.rglob("*")):
        if p.is_symlink() and not p.exists():
            p.unlink()


def tree_size(root: Path) -> int:
    """Bytes on disk, counting a link as nothing rather than as its target."""
    return sum(p.lstat().st_size for p in root.rglob("*") if p.is_file() and not p.is_symlink())


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
        linux = sys.platform.startswith("linux")
        before = linked_libraries(built) if linux else None
        start = tree_size(built)
        for pattern in PRUNE + (PRUNE_LINUX if linux else []):
            for p in list(built.glob(pattern)):
                if p.is_symlink() or p.is_file():
                    p.unlink()
                elif p.is_dir():
                    shutil.rmtree(p)
        if linux:
            remove_dangling_links(built)
            orphans = prune_orphans(built, before)
            remove_dangling_links(built)
            print(f"  of which {orphans / 1e6:.1f} MB of libraries only pruned plugins used")
        print(f"  pruned {(start - tree_size(built)) / 1e6:.1f} MB of unused Qt")
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
    print(f"wrote {target} ({tree_size(target) / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
