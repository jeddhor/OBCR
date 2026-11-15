# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""Finding and validating the user's engine (OBCR), game data and workspace.

The output of first-run setup is not "valid" or "invalid" -- it is a checklist,
because "could not find your game" is the least useful thing a program can say.
Every check names what it looked for, what it found, and what to do about it.

Two rules, both about not being clever:

**Do not go looking.** Nothing here scans a home directory. Candidates come
from the installed layout (OBCEdit lives in `<install>/obcedit/`, beside
`<install>/obcr[.exe]` and `<install>/data/`), the PATH, the checkout's own
`target/` builds, `OBC_DATA_DIR`, and paths the user picked.

**Do not run it to identify it, until asked.** The engine is the authority on
which release a data folder holds (`obcr --check-data`), and asking means
running it, so that waits for a `probe=True`. Until then, identification is a
bounded read of the files' own bytes.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from .archive import MAPHEAD_NAME, MAPTEMP_NAME, archive_paths, parse_archive
from .assets import PALETTE_OFFSETS, PALETTE_SIZE, AssetError, load_palette, parse_gfx_header
from .bundle import workspace_root
from .paths import canonical, digest_file, looks_like_game_data

#: What the editor needs: the maps, the page file, and an executable (the
#: palette lives there). Either release's executable will do.
REQUIRED_DATA = (MAPHEAD_NAME, MAPTEMP_NAME, "GFXTILES.BC")
EXECUTABLES = ("COUNT.EXE", "BCCD.EXE")

#: Nice to have; never a blocker.
OPTIONAL_DATA = ("VGAGRAPH.BC", "VGADICT.BC", "VGAHEAD.BC")

OK = "ok"
WARN = "warning"
FAIL = "error"


@dataclass(frozen=True)
class Check:
    """One line of the first-run checklist."""

    name: str
    status: str
    detail: str = ""
    remedy: str = ""

    @property
    def passed(self) -> bool:
        return self.status != FAIL


@dataclass
class Report:
    """A checklist, and whether it is good enough to work with."""

    checks: list[Check] = field(default_factory=list)

    def add(self, name: str, status: str, detail: str = "", remedy: str = "") -> None:
        self.checks.append(Check(name, status, detail, remedy))

    @property
    def usable(self) -> bool:
        return all(check.passed for check in self.checks)

    @property
    def failures(self) -> list[Check]:
        return [check for check in self.checks if check.status == FAIL]

    @property
    def warnings(self) -> list[Check]:
        return [check for check in self.checks if check.status == WARN]

    def __iter__(self):
        return iter(self.checks)

    def __len__(self) -> int:
        return len(self.checks)


@dataclass
class Profile:
    """A trusted local association of engine, data and workspace.

    Lives in the user's own settings, never in a project file.
    """

    profile_id: str = ""
    engine_path: str = ""
    data_dir: str = ""
    workspace_dir: str = ""
    data_fingerprint: str = ""
    engine_version: str = ""
    #: The release id `obcr --check-data` reported (OBC-A, OBC-CD), if probed.
    release: str = ""

    def to_json(self) -> dict:
        return {
            "profile_id": self.profile_id,
            "engine_path": self.engine_path,
            "data_dir": self.data_dir,
            "workspace_dir": self.workspace_dir,
            "data_fingerprint": self.data_fingerprint,
            "engine_version": self.engine_version,
            "release": self.release,
        }

    @classmethod
    def from_json(cls, payload: dict) -> "Profile":
        return cls(**{key: str(payload.get(key, "")) for key in cls().to_json()})


def find_file(directory: Path | str, name: str) -> Path | None:
    """`name` in `directory`, in any letter case (data copied from DOS is upper)."""
    directory = Path(directory)
    direct = directory / name
    if direct.is_file():
        return direct
    lowered = name.lower()
    if directory.is_dir():
        for entry in directory.iterdir():
            if entry.name.lower() == lowered and entry.is_file():
                return entry
    return None


def game_executable(directory: Path | str) -> Path | None:
    """COUNT.EXE or BCCD.EXE in `directory`, whichever is there."""
    for name in EXECUTABLES:
        found = find_file(directory, name)
        if found is not None:
            return found
    return None


def data_fingerprint(directory: Path | str) -> str:
    """A digest over the required files, identifying a data set by content."""
    import hashlib

    directory = Path(directory)
    digest = hashlib.sha256()
    executable = game_executable(directory)
    for name in REQUIRED_DATA:
        path = find_file(directory, name)
        digest.update(name.encode("ascii"))
        digest.update(digest_file(path).encode("ascii") if path else b"-")
    digest.update(b"EXE")
    digest.update(digest_file(executable).encode("ascii") if executable else b"-")
    return digest.hexdigest()


def check_data_directory(directory: Path | str) -> Report:
    """Bounded, read-only probes of a candidate game-data directory."""
    report = Report()
    directory = Path(directory).expanduser()

    if not directory.is_dir():
        report.add("Data directory", FAIL, f"{directory} is not a directory",
                   "Choose the folder holding GFXTILES.BC and COUNT.EXE or BCCD.EXE "
                   "(an OBCR installation's data folder works).")
        return report
    report.add("Data directory", OK, str(canonical(directory)))

    missing = [name for name in REQUIRED_DATA if find_file(directory, name) is None]
    executable = game_executable(directory)
    if executable is None:
        missing.append("COUNT.EXE or BCCD.EXE")
    if missing:
        report.add(
            "Required game files", FAIL, f"missing: {', '.join(missing)}",
            "Use the folder OBCR Setup installed the game files into, or your own copy "
            "of Operation Body Count. The editor never ships them.",
        )
    else:
        report.add("Required game files", OK, f"all present ({executable.name})")

    absent = [name for name in OPTIONAL_DATA if find_file(directory, name) is None]
    if absent:
        report.add("Optional content", WARN, f"not present: {', '.join(absent)}",
                   "Pictures will be unavailable; editing maps is unaffected.")
    else:
        report.add("Optional content", OK, "all present")

    if executable is not None:
        name = executable.name.upper()
        offset = PALETTE_OFFSETS.get(name, 0)
        size = executable.stat().st_size
        if size < offset + PALETTE_SIZE:
            report.add("Palette", FAIL, f"{name} is {size} bytes; too short for its palette",
                       "This is not the executable of a supported release.")
        else:
            with open(executable, "rb") as handle:
                handle.seek(offset)
                window = handle.read(PALETTE_SIZE)
            try:
                load_palette(b"\x00" * offset + window, name)
                report.add("Palette", OK, f"a 6-bit VGA palette is in {name}")
            except AssetError as error:
                report.add("Palette", FAIL, str(error),
                           "The executable is the wrong build or the wrong game.")

    try:
        head, temp = archive_paths(directory)
        parsed = parse_archive(head.read_bytes(), temp.read_bytes())
        report.add("Map archive", OK, f"{len(parsed)} maps")
    except Exception as error:  # any codec failure, reported not raised
        if find_file(directory, MAPTEMP_NAME):
            report.add("Map archive", FAIL, str(error),
                       "MAPHEAD.BC/MAPTEMP.BC did not parse; the copy may be damaged.")

    graphics = find_file(directory, "GFXTILES.BC")
    if graphics is not None:
        try:
            with open(graphics, "rb") as handle:
                head6 = handle.read(6)
                chunk_count = int.from_bytes(head6[0:2], "little")
                handle.seek(0)
                header = parse_gfx_header(handle.read(6 + chunk_count * 6))
            report.add("Wall and sprite artwork", OK,
                       f"{header.sprite_start} walls, "
                       f"{header.sound_start - header.sprite_start} sprites")
        except Exception as error:
            report.add("Wall and sprite artwork", FAIL, str(error),
                       "GFXTILES.BC did not parse.")
    return report


#: `obcr --version` prints `obcr <version>`.
IDENTITY_FLAG = "--version"
_IDENTITY = re.compile(r"^(obcr)\s+(\S+)", re.MULTILINE | re.IGNORECASE)


def _run(command: list[str], timeout: float, cwd=None) -> tuple[int, str]:
    result = subprocess.run(command, capture_output=True, text=True, timeout=timeout,
                            cwd=str(cwd) if cwd else None, stdin=subprocess.DEVNULL)
    return result.returncode, (result.stdout or "") + (result.stderr or "")


def check_engine(
    path: Path | str,
    *,
    probe: bool = False,
    timeout: float = 20.0,
    cwd: Path | str | None = None,
) -> Report:
    """Check an OBCR binary. Only runs it when `probe` is explicitly asked for."""
    report = Report()
    path = Path(path).expanduser()

    if not path.exists():
        report.add("Engine", FAIL, f"{path} does not exist",
                   "Choose the obcr executable of your OBCR installation or build.")
        return report
    if not path.is_file():
        report.add("Engine", FAIL, f"{path} is not a regular file",
                   "Choose the executable itself, not a directory.")
        return report
    report.add("Engine", OK, str(canonical(path)))

    if not os.access(path, os.X_OK):
        report.add("Executable bit", FAIL, "the file is not executable",
                   "chmod +x it, or choose a different build.")
        return report
    report.add("Executable bit", OK, "set")

    if not probe:
        report.add("Engine identity", WARN, "not checked",
                   "Run the identity probe to confirm this is OBCR.")
        return report

    try:
        _, text = _run([str(canonical(path)), IDENTITY_FLAG], timeout, cwd)
    except subprocess.TimeoutExpired:
        report.add("Engine identity", FAIL, f"the probe did not return within {timeout:g}s",
                   "The binary may be waiting for input or for a display.")
        return report
    except (OSError, subprocess.SubprocessError) as error:
        report.add("Engine identity", FAIL, f"the probe failed: {error}",
                   "The file may not be an executable for this machine.")
        return report

    found = _IDENTITY.search(text)
    if found:
        report.add("Engine identity", OK, found.group(0).strip())
    else:
        first = text.strip().splitlines()[0] if text.strip() else "no output"
        report.add("Engine identity", FAIL, first, "This does not identify itself as OBCR.")
    return report


def engine_version(report: Report) -> str:
    """The version string out of a probed engine report, if there is one."""
    for check in report:
        if check.name == "Engine identity" and check.status == OK:
            return check.detail
    return ""


@dataclass
class DataIdentity:
    """What `obcr --check-data` said about a folder or CD image."""

    release: str = ""
    description: str = ""
    complete: bool = False
    source: str = ""
    files: list[tuple[str, str, str]] = field(default_factory=list)  # role, status, name
    error: str = ""


def parse_check_data(text: str) -> DataIdentity:
    """Parse `obcr --check-data` output (tab-separated records)."""
    found = DataIdentity()
    for line in text.splitlines():
        fields = line.rstrip("\r").split("\t")
        tag = fields[0]
        if tag == "release" and len(fields) > 2:
            found.release, found.description = fields[1], fields[2]
        elif tag == "source" and len(fields) > 1:
            found.source = fields[1]
        elif tag == "file" and len(fields) > 3:
            found.files.append((fields[1], fields[2], fields[3]))
        elif tag == "complete" and len(fields) > 1:
            found.complete = fields[1] == "yes"
        elif tag == "error" and len(fields) > 1:
            found.error = fields[1]
    return found


def identify_data(engine: Path | str, directory: Path | str, *, timeout: float = 60.0) -> DataIdentity:
    """Ask the engine which release `directory` holds. Runs `obcr`."""
    try:
        _, text = _run([str(engine), "--check-data", str(directory)], timeout)
    except (OSError, subprocess.SubprocessError) as error:
        return DataIdentity(error=f"could not run {engine}: {error}")
    identity = parse_check_data(text)
    if not identity.release and not identity.error:
        identity.error = "the engine gave no answer"
    return identity


def check_workspace(directory: Path | str, *, data_dir: Path | str = "") -> Report:
    """Check the workspace: writable, and not inside the game data."""
    report = Report()
    directory = Path(directory).expanduser()
    try:
        directory.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        report.add("Workspace", FAIL, f"cannot create {directory}: {error}",
                   "Choose a folder you can write to.")
        return report
    if not os.access(directory, os.W_OK):
        report.add("Workspace", FAIL, f"{directory} is not writable",
                   "Choose a folder you can write to.")
        return report
    report.add("Workspace", OK, str(canonical(directory)))

    resolved = canonical(directory)
    if looks_like_game_data(resolved):
        report.add("Separation", FAIL, f"{resolved} holds game data",
                   "Keep projects out of the game directory so an export can never "
                   "land on the originals.")
    elif data_dir:
        data_resolved = canonical(data_dir)
        if resolved == data_resolved or resolved.is_relative_to(data_resolved):
            report.add("Separation", FAIL, f"{resolved} is inside {data_resolved}",
                       "Choose a workspace outside the game data directory.")
        else:
            report.add("Separation", OK, "workspace and game data are separate")
    else:
        report.add("Separation", OK, "no game data here")
    return report


def check_profile(profile: Profile, *, probe: bool = False) -> Report:
    """The whole first-run checklist, in the order the page shows it."""
    report = Report()
    if profile.engine_path:
        report.checks.extend(check_engine(profile.engine_path, probe=probe))
    if profile.data_dir:
        report.checks.extend(check_data_directory(profile.data_dir))
        if probe and profile.engine_path and Path(profile.engine_path).is_file():
            identity = identify_data(profile.engine_path, profile.data_dir)
            if identity.error:
                report.add("Release", WARN, identity.error,
                           "The engine could not identify the data; editing still works.")
            elif identity.complete:
                report.add("Release", OK, f"{identity.release}: {identity.description}")
            else:
                report.add("Release", WARN, f"{identity.release}: incomplete or modified",
                           "Playtesting needs a complete copy of a supported release.")
    if profile.workspace_dir:
        report.checks.extend(check_workspace(profile.workspace_dir, data_dir=profile.data_dir))
    if not report.checks:
        report.add("Profile", FAIL, "nothing has been chosen yet",
                   "Pick OBCR, a game-data directory and a workspace.")
    return report


ENGINE_NAMES = ("obcr.exe", "obcr") if os.name == "nt" else ("obcr", "obcr.exe")


def candidate_engines(start: Path | str | None = None) -> list[Path]:
    """Places an OBCR binary plausibly is. A short list, never a search."""
    root = Path(start) if start else workspace_root()
    places = [root, root / "target" / "release", root / "target" / "debug"]
    found: list[Path] = []
    for place in places:
        for name in ENGINE_NAMES:
            candidate = place / name
            if candidate.is_file() and candidate not in found:
                found.append(candidate)
    on_path = shutil.which("obcr")
    if on_path and Path(on_path) not in found:
        found.append(Path(on_path))
    return found


def candidate_data_dirs(start: Path | str | None = None) -> list[Path]:
    """Likewise for the game data: named places, never a scan."""
    root = Path(start) if start else workspace_root()
    places = [root / "data", root / ".local" / "data" / "obc-full",
              root / ".local" / "data" / "obc-cd"]
    env = os.environ.get("OBC_DATA_DIR")
    if env:
        places.insert(0, Path(env))
    return [p for p in places if p.is_dir() and looks_like_game_data(p)]


def default_workspace() -> Path:
    """Where projects go unless the user says otherwise."""
    return Path.home() / "OBCEdit Projects"
