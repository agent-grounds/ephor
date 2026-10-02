#!/usr/bin/env python3
"""Stamp, collect and read changelog release sections. §FS-002-release.2

Three commands, the ones the release workflows have always called. `stamp`
writes the numbers the contributors could not know onto the pending entries
(§FS-002-release.2.2, in `changelog_stamp`). `prepare` collects the entries
under `docs/changelog/unreleased/` into a numbered release inline, rotates the
previous release out to its archive, and consumes the entries
(§FS-002-release.2.3). `notes` writes the inline section the GitHub release
publishes (§FS-002-release.3).
"""

from __future__ import annotations

import argparse
import datetime as _datetime
import re
import sys
from pathlib import Path
from typing import NamedTuple, Sequence

# `__file__` is set under `python scripts/...` and under the test harness's
# load-by-path alike, so this reaches the shared reader from both (§FS-002-release.2).
sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_history import GitError, HistoryUnavailable, landings, repository_path  # noqa: E402
from changelog_stamp import stamp_entries  # noqa: E402
from changelog_unreleased import (  # noqa: E402
    CATEGORIES,
    ENTRY_PARTS,
    ENTRY_README,
    entry_directory,
    entry_lines,
    entry_name,
    entry_problem,
    rebase_links,
    stray_bullets,
)


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
UNRELEASED_RE = re.compile(r"^## Unreleased\s*$")
RELEASE_RE = re.compile(
    r"^## (?P<number>[0-9]+)\. \[(?P<version>[0-9]+\.[0-9]+\.[0-9]+)\] — (?P<date>[0-9]{4}-[0-9]{2}-[0-9]{2})\s*$"
)
OLDER_RE = re.compile(r"^## (?P<number>[0-9]+)\. Older releases\s*$")
# An archive is read in `docs/changelog/`, one directory below the inline release.
ARCHIVE_PARTS = ("changelog",)


class ChangelogError(Exception):
    pass


class Entry(NamedTuple):
    """A pending entry as the release reads it: its file, its slug and category, and its bullet."""

    path: Path
    slug: str
    category: str
    lines: tuple[str, ...]


def prepare_release(changelog: Path, version: str, release_date: str) -> None:
    """Collect the entries into a release, rotate the previous one out, consume them. §FS-002-release.2.3

    Everything is read and every refusal made before the first write, so a
    refused release leaves the tree exactly as it found it.
    """
    _validate_version(version)
    _validate_date(release_date)

    lines = _read_lines(changelog)
    sections = _find_top_level_sections(lines)
    unreleased = _find_section(lines, sections, UNRELEASED_RE, "## Unreleased")
    latest = _next_section_after(sections, unreleased, "latest release")
    older = _find_section_after(lines, sections, latest, OLDER_RE, "Older releases")

    latest_match = RELEASE_RE.match(_line_text(lines[latest]))
    if latest_match is None:
        raise ChangelogError(f"expected latest release heading after ## Unreleased, got: {_line_text(lines[latest])}")

    if latest_match.group("version") == version:
        raise ChangelogError(f"docs/changelog.md already has {version} as the inline latest release")

    # §FS-002-release.2.3: a bullet written the old way would be lost if the release went on.
    stray = stray_bullets(lines)
    if stray:
        raise ChangelogError(
            f"## Unreleased holds {len(stray)} bullet(s), the first at line {stray[0] + 1}: "
            f"{_line_text(lines[stray[0]]).strip()}\n"
            f"  ## Unreleased is a pointer now. Move each bullet into an entry of its own under "
            f"{entry_directory(changelog).as_posix()}/, or this release would drop it."
        )
    entries = read_entries(changelog)

    previous_version = latest_match.group("version")
    previous_date = latest_match.group("date")
    previous_body = lines[latest + 1 : older]
    archive_path = changelog.parent / "changelog" / f"{previous_version}.md"
    if archive_path.exists():
        raise ChangelogError(f"archive already exists: {archive_path}")

    release_body = _release_body(order_entries(entries, entry_directory(changelog)))
    archived_body = [rebase_links(line, (), ARCHIVE_PARTS) for line in previous_body]
    summary = _summary_from(previous_body)
    older_body = _drop_leading_blank_lines(lines[older + 1 :])
    archive_link = f"- [{previous_version}](changelog/{previous_version}.md) — {previous_date}: {summary}\n"

    new_lines = [
        # Everything through the `## Unreleased` pointer stays as it is.
        *_trim_trailing_blank_lines(lines[:latest]),
        "\n",
        f"## 2. [{version}] — {release_date}\n",
        "\n",
        *release_body,
        "\n",
        "## 3. Older releases\n",
        "\n",
        archive_link,
        *older_body,
    ]
    _write_lines(archive_path, [f"# {previous_version} — {previous_date}\n", *archived_body])
    _write_lines(changelog, new_lines)
    for entry in entries:
        entry.path.unlink()


def read_entries(changelog: Path) -> list[Entry]:
    """Every pending entry, or a refusal naming each malformed one. §FS-002-release.2.3"""
    directory = entry_directory(changelog)
    shown = directory.as_posix()
    found: list[Entry] = []
    problems: list[str] = []
    for path in sorted(directory.iterdir()) if directory.is_dir() else []:
        if path.name == ENTRY_README:
            continue
        if path.is_dir():
            problems.append(f"{shown}/{path.name}: a directory, where the entries are files")
            continue
        try:
            text: str | None = path.read_bytes().decode("utf-8")
        except UnicodeDecodeError:
            text = None
        except OSError as exc:
            problems.append(f"{shown}/{path.name}: cannot be read: {exc}")
            continue
        problem = entry_problem(path.name, text)
        if problem is not None:
            problems.append(f"{shown}/{problem}")
            continue
        parsed = entry_name(path.name)
        found.append(Entry(path, parsed.slug, parsed.category, tuple(entry_lines(text))))

    slugs: dict[str, list[str]] = {}
    for entry in found:
        slugs.setdefault(entry.slug, []).append(entry.path.name)
    problems += [
        f"{shown}/: the slug `{slug}` is used by {' and '.join(names)}; a slug is unique across every category"
        for slug, names in slugs.items()
        if len(names) > 1
    ]
    if problems:
        raise ChangelogError("\n".join(problems))
    if not found:
        raise ChangelogError(f"{shown}/ holds no entry to release; its {ENTRY_README} is not one")
    return found


def order_entries(entries: Sequence[Entry], directory: Path) -> list[Entry]:
    """By category, then oldest-landed first, then by file name; uncommitted ones last. §FS-002-release.2.1"""
    try:
        landed = landings(repository_path(directory))
    except (HistoryUnavailable, GitError) as exc:
        print(f"warning: ordering the entries by file name alone: {exc}", file=sys.stderr)
        landed = {}

    def order(entry: Entry) -> tuple[int, bool, int, str]:
        landing = landed.get(entry.slug)
        return (
            CATEGORIES.index(entry.category),
            landing is None,
            landing.position if landing else 0,
            entry.path.name,
        )

    return sorted(entries, key=order)


def _release_body(entries: Sequence[Entry]) -> list[str]:
    """One section per category in release order, each bullet as published, links rebased for `docs/`."""
    body: list[str] = []
    for category in CATEGORIES:
        members = [entry for entry in entries if entry.category == category]
        if not members:
            continue
        if body:
            body.append("\n")
        body += [f"### {category.capitalize()}\n", "\n"]
        for index, entry in enumerate(members):
            if index:
                body.append("\n")
            body += [rebase_links(line, ENTRY_PARTS, ()) + "\n" for line in entry.lines]
    return body


def extract_notes(changelog: Path, version: str, output: Path) -> None:
    """The inline section of `version`, which is what the GitHub release publishes. §FS-002-release.3"""
    _validate_version(version)
    lines = _read_lines(changelog)
    sections = _find_top_level_sections(lines)

    for index, section_start in enumerate(sections):
        match = RELEASE_RE.match(_line_text(lines[section_start]))
        if match is None or match.group("version") != version:
            continue
        section_end = sections[index + 1] if index + 1 < len(sections) else len(lines)
        body = _trim_blank_lines(lines[section_start + 1 : section_end])
        if not body:
            raise ChangelogError(f"release {version} has an empty changelog section")
        _write_lines(output, [*body, "\n"])
        return

    raise ChangelogError(f"release {version} is not the inline changelog release")


def _find_top_level_sections(lines: Sequence[str]) -> list[int]:
    """Every `## ` heading, for `prepare` and `notes`, which want every top-level section rather than one."""
    return [index for index, line in enumerate(lines) if line.startswith("## ") and not line.startswith("### ")]


def _find_section(lines: Sequence[str], sections: Sequence[int], pattern: re.Pattern[str], name: str) -> int:
    for section in sections:
        if pattern.match(_line_text(lines[section])):
            return section
    raise ChangelogError(f"missing {name} section")


def _find_section_after(
    lines: Sequence[str], sections: Sequence[int], after: int, pattern: re.Pattern[str], name: str
) -> int:
    for section in sections:
        if section <= after:
            continue
        if pattern.match(_line_text(lines[section])):
            return section
    raise ChangelogError(f"missing {name} section")


def _next_section_after(sections: Sequence[int], after: int, name: str) -> int:
    for section in sections:
        if section > after:
            return section
    raise ChangelogError(f"missing {name} section")


def _read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding="utf-8").splitlines(keepends=True)
    except FileNotFoundError as exc:
        raise ChangelogError(f"missing changelog: {path}") from exc


def _write_lines(path: Path, lines: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(lines), encoding="utf-8")


def _line_text(line: str) -> str:
    return line.rstrip("\r\n")


def _trim_blank_lines(lines: Sequence[str]) -> list[str]:
    trimmed = _trim_trailing_blank_lines(_drop_leading_blank_lines(lines))
    if trimmed and not trimmed[-1].endswith(("\n", "\r")):
        trimmed[-1] += "\n"
    return trimmed


def _trim_trailing_blank_lines(lines: Sequence[str]) -> list[str]:
    trimmed = list(lines)
    while trimmed and not trimmed[-1].strip():
        trimmed.pop()
    return trimmed


def _drop_leading_blank_lines(lines: Sequence[str]) -> list[str]:
    trimmed = list(lines)
    while trimmed and not trimmed[0].strip():
        trimmed.pop(0)
    return trimmed


def _summary_from(lines: Sequence[str]) -> str:
    paragraph: list[str] = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            if paragraph:
                break
            continue
        if stripped.startswith("#"):
            continue
        paragraph.append(stripped)

    if not paragraph:
        return "release notes."

    text = re.sub(r"\s+", " ", " ".join(paragraph))
    first_sentence = re.match(r"(.+?[.!?])(?:\s|$)", text)
    if first_sentence is not None:
        return first_sentence.group(1)
    return text


def _validate_version(version: str) -> None:
    if VERSION_RE.match(version) is None:
        raise ChangelogError(f"version must look like 0.1.0, got {version!r}")


def _validate_date(release_date: str) -> None:
    try:
        _datetime.date.fromisoformat(release_date)
    except ValueError as exc:
        raise ChangelogError(f"date must look like YYYY-MM-DD, got {release_date!r}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Stamp, collect or read docs/changelog.md release sections.")
    parser.add_argument("--changelog", type=Path, default=Path("docs/changelog.md"))
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare = subparsers.add_parser("prepare", help="collect the pending entries into a numbered release")
    prepare.add_argument("version")
    prepare.add_argument("--date", default=_datetime.date.today().isoformat())

    notes = subparsers.add_parser("notes", help="write release notes for the inline release")
    notes.add_argument("version")
    notes.add_argument("--output", type=Path, required=True)

    subparsers.add_parser("stamp", help="write PR numbers onto pending entries that end with none")

    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            prepare_release(args.changelog, args.version, args.date)
        elif args.command == "stamp":
            stamp_entries(args.changelog)
        elif args.command == "notes":
            extract_notes(args.changelog, args.version, args.output)
        else:
            raise AssertionError(args.command)
    except ChangelogError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
