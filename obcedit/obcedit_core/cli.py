# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Jason Tripp
"""`obcedit` -- the headless half of the editor.

Verbs:

* `inspect DIR`          -- the maps in a MAPHEAD.BC/MAPTEMP.BC pair;
* `validate DIR`         -- each map against Operation Body Count's rules;
* `verify-archive DIR`   -- decode and re-encode; byte-identical or not;
* `catalog-info`         -- what the shipped catalog holds;
* `project-new`, `project-inspect`, `project-import` -- projects;
* `project-export` / `mod-export` -- a project as a mod (folder or .obcmod);
* `mod-audit PATH`       -- read a mod back and report everything about it.

Every verb is read-only with respect to its input. Writing verbs protect the
source's directory automatically, so exporting next to the game data and
clobbering it is refused rather than trusted not to happen.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import replace
from pathlib import Path

from . import __version__
from .archive import Archive, MAX_MAPS, read_archive, verify_archive
from .campaign import Campaign
from .campaign import validate as campaign_validate
from .catalog import CATEGORIES, default_catalog
from .document import MapDocument, SourceReference, utc_now
from .errors import ObcEditError, Severity
from .modexport import audit_mod, build_mod, write_mod
from .paths import OutputGuard, SourceIdentity
from .project import load_project, new_project, save_project
from .validation import profile_for_map, profile_for_slot, summarize, validate_map

EXIT_OK = 0
EXIT_ERROR = 1
EXIT_USAGE = 2


def _describe(archive: Archive) -> dict:
    return {
        "format": "Operation Body Count MAPHEAD.BC + MAPTEMP.BC",
        "map_count": len(archive),
        "rlew_tag": archive.rlew_tag,
        "maps": [
            {
                "slot": record.number,
                "name": record.name.text,
                "name_raw": record.name.raw.hex(),
                "name_canonical": record.name.is_canonical,
                "width": record.width,
                "height": record.height,
                "header_offset": record.source.header_offset if record.source else None,
                "plane_offsets": list(record.source.plane_offsets) if record.source else None,
                "plane_lengths": list(record.source.plane_lengths) if record.source else None,
            }
            for record in archive
        ],
        "diagnostics": [str(entry) for entry in archive.diagnostics],
    }


def command_inspect(args) -> int:
    archive = read_archive(args.archive)
    if args.json:
        json.dump(_describe(archive), sys.stdout, indent=2)
        print()
        return EXIT_OK
    print(f"{args.archive}: {len(archive)} maps, MAPHEAD.BC + MAPTEMP.BC (RLEW tag "
          f"0x{archive.rlew_tag:04X})")
    for record in archive:
        print(f"  slot {record.number:02d}  {record.width}x{record.height}  {record.name.text}")
    return EXIT_OK


def command_validate(args) -> int:
    archive = read_archive(args.archive)
    catalog = default_catalog()
    worst = EXIT_OK
    for record in archive:
        if args.map is not None and record.number not in args.map:
            continue
        document = MapDocument.from_record(record)
        problems = validate_map(document, catalog, profile=profile_for_slot(record.number))
        print(f"slot {record.number:02d} {record.name.text}: {summarize(problems)}")
        for problem in problems:
            if problem.severity >= (Severity.INFORMATION if args.all else Severity.WARNING):
                print(f"  {problem}")
        if any(p.severity is Severity.ERROR for p in problems):
            worst = EXIT_ERROR
    return worst


def command_verify_archive(args) -> int:
    result = verify_archive(args.archive)
    if result.identical:
        print(f"{args.archive}: {result.maps} maps; re-encoding is byte-identical")
        return EXIT_OK
    print(f"{args.archive}: {result.maps} maps; re-encoding DIFFERS "
          f"(MAPHEAD {'same' if result.maphead_identical else 'differs'}, MAPTEMP first "
          f"difference at {result.first_difference})")
    return EXIT_ERROR


def command_catalog_info(args) -> int:
    catalog = default_catalog()
    print(f"catalog version {catalog.schema}: {len(catalog)} palette entries")
    for category in CATEGORIES:
        print(f"  {category:<9} {len(catalog.in_category(category))}")
    print("limits: " + ", ".join(f"{k}={v}" for k, v in sorted(catalog.limits.items())))
    return EXIT_OK


def command_project_new(args) -> int:
    project = new_project(args.name, author=args.author)
    if args.room:
        project = project.added(MapDocument.new_room(name=args.room[:15]))
    save_project(project, args.output)
    print(f"{args.output}: new project {project.name!r} ({project.uuid})")
    return EXIT_OK


def command_project_inspect(args) -> int:
    project = load_project(args.project)
    campaign = Campaign.from_json(project.campaign) if project.campaign else None
    if args.json:
        json.dump({
            "uuid": project.uuid, "name": project.name, "author": project.author,
            "schema_version": project.schema_version,
            "maps": [{"uuid": d.uuid, "slot": d.slot, "name": d.name, "width": d.width,
                      "height": d.height,
                      "source_sha256": d.source.sha256 if d.source else ""}
                     for d in project.maps],
            "campaign": campaign.to_json() if campaign else None,
        }, sys.stdout, indent=2)
        print()
        return EXIT_OK
    print(f"{args.project}: {project.name!r} by {project.author or 'nobody'}, "
          f"{len(project)} map(s), schema {project.schema_version}")
    for document in project.maps:
        origin = ""
        if document.source and document.source.sha256:
            origin = f"  from slot {document.source.map_number} of {document.source.sha256[:12]}"
        print(f"  {document.label}  {document.width:3d}x{document.height:<3d}  "
              f"{document.name:<16}{origin}")
    if campaign:
        print(f"  mod {campaign.mod_id!r}: {len(campaign.levels)} floor(s)")
    return EXIT_OK


def command_project_import(args) -> int:
    """Copy a map out of an archive into a project. The archive is read only."""
    source = Path(args.archive)
    from .archive import archive_paths

    head, temp = archive_paths(source)
    identity = SourceIdentity.probe(temp)
    archive = read_archive(source)
    project = load_project(args.project) if args.project.exists() else new_project(args.name)
    reference = SourceReference(display_path=str(temp), sha256=identity.digest,
                                map_number=args.map, imported_at=utc_now())
    record = archive.by_number(args.map)
    document = MapDocument.from_record(record, source=reference)
    if args.slot is not None:
        document = replace(document, slot=args.slot)
    project = project.added(document)
    guard = OutputGuard.for_source(temp, extra_roots=args.protect)
    guard.check(args.project)
    save_project(project, args.project)
    identity.verify_unchanged()
    print(f"{args.project}: imported slot {args.map} {record.name.text!r} "
          f"({len(project)} map(s))")
    print(f"  source {identity.resolved} unchanged ({identity.digest[:16]})")
    return EXIT_OK


def command_mod_export(args) -> int:
    """Export a project as a mod: a folder, or a .obcmod file."""
    project = load_project(args.project)
    if not project.maps and not project.campaign:
        print("the project has no maps to export", file=sys.stderr)
        return EXIT_USAGE
    campaign = Campaign.from_json(project.campaign) if project.campaign \
        else Campaign.default_for(project)
    music = [str(r.target) for r in project.resources if r.kind == "music"]
    problems = campaign_validate(campaign, project.maps, sharing=not args.private,
                                 music_names=music)
    for problem in problems:
        print(f"  {problem}", file=sys.stderr)
    if any(p.severity is Severity.ERROR for p in problems):
        return EXIT_ERROR
    catalog = default_catalog()
    for uuid in campaign.map_order():
        document = next(d for d in project.maps if d.uuid == uuid)
        profile = profile_for_map(document, campaign.arena_maps())
        errors = [p for p in validate_map(document, catalog, profile=profile)
                  if p.severity is Severity.ERROR]
        if errors and not args.force:
            print(f"{document.name}: {summarize(errors)}", file=sys.stderr)
            for problem in errors[:10]:
                print(f"  {problem}", file=sys.stderr)
            print("not exported (--force exports anyway; OBCR will refuse the map)",
                  file=sys.stderr)
            return EXIT_ERROR
    from .errors import ObcEditError
    from .resources import store_for

    # A saved project keeps its replacement files beside it (<name>.assets/).
    store = store_for(args.project, workspace="", project_uuid=project.uuid)
    try:
        build = build_mod(project, campaign, sharing=not args.private, store=store)
    except ObcEditError as error:
        print(f"not exported: {error}", file=sys.stderr)
        return EXIT_ERROR
    guard = OutputGuard(protected_roots=tuple(args.protect))
    written = write_mod(build, args.output, guard=guard)
    audit = audit_mod(written, catalog=catalog)
    print(f"{written}: mod {campaign.mod_id!r}, {len(campaign.levels)} floor(s), "
          f"{len(build.slots)} map(s), digest {build.digest[:16]}")
    for problem in audit.problems:
        print(f"  {problem}")
    return EXIT_OK if audit.ok else EXIT_ERROR


def command_mod_audit(args) -> int:
    audit = audit_mod(args.path, catalog=default_catalog())
    campaign = audit.campaign
    print(f"{args.path}: {len(audit.files)} file(s)")
    for name, size in sorted(audit.files.items()):
        print(f"  {name}  {size} bytes")
    if campaign:
        print(f"mod {campaign.mod_id!r} {campaign.title!r} version {campaign.version}, "
              f"{len(campaign.levels)} floor(s), release {campaign.release}")
        for index, level in enumerate(campaign.levels):
            what = level.map_uuid.replace("slot:", "map ") if level.map_uuid \
                else f"original floor {level.stock + 1}"
            print(f"  floor {index + 1}: {what}" + (f" {level.name!r}" if level.name else ""))
    for problem in audit.problems:
        print(f"  {problem}")
    print("result: " + ("ok" if audit.ok else "errors"))
    return EXIT_OK if audit.ok else EXIT_ERROR


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="obcedit", description="OBCEdit, the Operation Body Count level editor (headless).")
    parser.add_argument("--version", action="version", version=f"obcedit {__version__}")
    verbs = parser.add_subparsers(dest="verb", required=True)

    inspect = verbs.add_parser("inspect", help="list the maps of a MAPHEAD.BC/MAPTEMP.BC pair")
    inspect.add_argument("archive", type=Path, help="the folder, or either of its two files")
    inspect.add_argument("--json", action="store_true")
    inspect.set_defaults(handler=command_inspect)

    validate = verbs.add_parser("validate", help="check each map against the game's rules")
    validate.add_argument("archive", type=Path)
    validate.add_argument("--map", type=int, nargs="*", help="only these slots")
    validate.add_argument("--all", action="store_true", help="include information notes")
    validate.set_defaults(handler=command_validate)

    verify = verbs.add_parser("verify-archive",
                              help="decode and re-encode an archive; report byte identity")
    verify.add_argument("archive", type=Path)
    verify.set_defaults(handler=command_verify_archive)

    catalog = verbs.add_parser("catalog-info", help="summarize the shipped catalog")
    catalog.set_defaults(handler=command_catalog_info)

    project_new = verbs.add_parser("project-new", help="create a project")
    project_new.add_argument("output", type=Path)
    project_new.add_argument("--name", default="Untitled")
    project_new.add_argument("--author", default="")
    project_new.add_argument("--room", default="", help="add a new walled room with this name")
    project_new.set_defaults(handler=command_project_new)

    project_inspect = verbs.add_parser("project-inspect", help="list a project's maps")
    project_inspect.add_argument("project", type=Path)
    project_inspect.add_argument("--json", action="store_true")
    project_inspect.set_defaults(handler=command_project_inspect)

    project_import = verbs.add_parser("project-import",
                                      help="copy a map from an archive into a project")
    project_import.add_argument("archive", type=Path)
    project_import.add_argument("--project", type=Path, required=True)
    project_import.add_argument("--map", type=int, required=True, help="the slot to import")
    project_import.add_argument("--slot", type=int, default=None)
    project_import.add_argument("--name", default="Imported")
    project_import.add_argument("--protect", type=Path, nargs="*", default=[])
    project_import.set_defaults(handler=command_project_import)

    for verb in ("mod-export", "project-export"):
        export = verbs.add_parser(verb, help="export a project as a mod (folder or .obcmod)")
        export.add_argument("project", type=Path)
        export.add_argument("--output", type=Path, required=True)
        export.add_argument("--private", action="store_true",
                            help="allow maps imported from the original game (not for sharing)")
        export.add_argument("--force", action="store_true",
                            help="export maps with validation errors")
        export.add_argument("--protect", type=Path, nargs="*", default=[])
        export.set_defaults(handler=command_mod_export)

    audit = verbs.add_parser("mod-audit", help="read a mod back and report its contents")
    audit.add_argument("path", type=Path)
    audit.set_defaults(handler=command_mod_audit)
    return parser


def main(argv=None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.handler(args)
    except ObcEditError as error:
        print(f"error: {error.diagnostic}", file=sys.stderr)
        return EXIT_ERROR
    except OSError as error:
        print(f"error: {error}", file=sys.stderr)
        return EXIT_ERROR


__all__ = ["MAX_MAPS", "build_parser", "main"]

if __name__ == "__main__":
    raise SystemExit(main())
