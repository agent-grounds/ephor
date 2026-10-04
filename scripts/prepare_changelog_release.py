#!/usr/bin/env python3
"""Write a release's changelog section from the pull requests it ships, and read it back. §FS-002-release.2

Two commands. `prepare` reads the range and the pull requests that landed on it
from git and the forge (§FS-002-release.1.3, in `release_pulls`), and the
compatibility notices from the published schemas (§FS-002-release.1.4, in
`release_notices`), then writes the numbered release inline and rotates the
previous one out to its archive (§FS-002-release.2.3). `notes` writes the
inline section the GitHub release publishes (§FS-002-release.3). Nobody writes
anything first.
"""

from __future__ import annotations

import argparse
import datetime as _datetime
import posixpath
import re
import sys
from pathlib import Path
from typing import NamedTuple, Sequence

# `__file__` is set under `python scripts/...` and under a load by path alike,
# so this reaches the sibling modules from both (§FS-002-release.2).
sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_notices import compatibility_notices  # noqa: E402
from release_pulls import (  # noqa: E402
    VERSION_RE,
    Forge,
    ReleaseRefused,
    landed_pulls,
    release_lines,
    release_range,
    repository,
    repository_root,
    version_key,
)


CONVENTIONS_RE = re.compile(r"^## 1\. Conventions\s*$")
RELEASE_RE = re.compile(
    r"^## (?P<number>[0-9]+)\. \[(?P<version>[0-9]+\.[0-9]+\.[0-9]+)\] — (?P<date>[0-9]{4}-[0-9]{2}-[0-9]{2})\s*$"
)
OLDER_RE = re.compile(r"^## 3\. Older releases\s*$")
PLACEHOLDER = "_None yet._"
NOTICES_HEADING = "### Compatibility notices"
# A relative Markdown link: its destination and its fragment, apart.
LINK_RE = re.compile(r"(?P<prefix>\]\()(?P<destination>[^)#\s][^)#\s]*)(?P<fragment>#[^)\s]*)?\)")
# An archive is read in `docs/changelog/`, one directory below the inline release.
ARCHIVE_PARTS = ("changelog",)


class ChangelogError(ReleaseRefused):
    pass


class Layout(NamedTuple):
    """Where `docs/changelog.md` keeps its slot, and the release already in it, if any. §FS-002-release.2.3"""

    lines: list[str]
    slot: int  # the first line of the slot: the inline release's heading, or `## 3. Older releases`
    older: int
    inline: re.Match[str] | None


def prepare_release(changelog: Path, version: str, release_date: str) -> None:
    """Write `version` from what it ships, rotating the previous release out. §FS-002-release.2.3

    Everything — the layout, the range, every pull request and file listing,
    the schema comparisons — is read and every refusal made before the first
    write, so a refused release leaves the tree exactly as it found it.
    """
    _validate_version(version)
    _validate_date(release_date)
    layout = read_layout(changelog)
    archive_path = None
    if layout.inline is not None:
        if version_key(version) <= version_key(layout.inline.group("version")):
            raise ChangelogError(
                f"{version} is not past {layout.inline.group('version')}, the release already inline in {changelog}"
            )
        archive_path = changelog.parent / "changelog" / f"{layout.inline.group('version')}.md"
        if archive_path.exists():
            raise ChangelogError(f"archive already exists: {archive_path}")

    root = repository_root(changelog.resolve().parent)
    forge = Forge(repository(root))
    branch = forge.default_branch()
    span = release_range(root, branch)
    if span.tag is not None and version_key(version) <= version_key(span.tag[1:]):
        raise ChangelogError(f"{version} is not past the previous tag, {span.tag}")
    lines = release_lines(landed_pulls(forge, branch, span))
    notices = compatibility_notices(root, forge.repo, span)

    body = [f"{text}\n" for text in lines]
    if notices:
        body += ["\n", f"{NOTICES_HEADING}\n", "\n", *(f"{text}\n" for text in notices)]
    section = [f"## 2. [{version}] — {release_date}\n", "\n", *body, "\n"]
    head = [*_trim_trailing_blank_lines(layout.lines[: layout.slot]), "\n"]
    older = layout.lines[layout.older :]

    if layout.inline is not None:
        previous_version, previous_date = layout.inline.group("version"), layout.inline.group("date")
        previous_body = layout.lines[layout.slot + 1 : layout.older]
        pointer = f"- [{previous_version}](changelog/{previous_version}.md) — {previous_date}: {_summary(previous_body)}\n"
        rest = [line for line in _drop_leading_blank_lines(older[1:]) if line.strip() != PLACEHOLDER]
        older = [older[0], "\n", pointer, *rest]
        archived = [_rebase_links(line, (), ARCHIVE_PARTS) for line in previous_body]
        _write_lines(archive_path, [f"# {previous_version} — {previous_date}\n", *archived])
    _write_lines(changelog, [*head, *section, *older])


def read_layout(changelog: Path) -> Layout:
    """The conventions, an empty slot or one inline release, then the older releases — or a refusal."""
    lines = _read_lines(changelog)
    sections = [index for index, line in enumerate(lines) if line.startswith("## ")]
    conventions = next((i for i in sections if CONVENTIONS_RE.match(lines[i])), None)
    if conventions is None:
        raise ChangelogError(f"{changelog} has no `## 1. Conventions` section to write the release after")
    older = next((i for i in sections if i > conventions and OLDER_RE.match(lines[i])), None)
    if older is None:
        raise ChangelogError(f"{changelog} has no `## 3. Older releases` section after its conventions")
    between = [i for i in sections if conventions < i < older]
    if not between:
        return Layout(lines, older, older, None)
    inline = RELEASE_RE.match(lines[between[0]])
    if len(between) > 1 or inline is None:
        raise ChangelogError(
            f"{changelog} holds `{lines[between[-1 if inline else 0]].strip()}` between its conventions and its "
            f"older releases, where only the latest release belongs"
        )
    return Layout(lines, between[0], older, inline)


def extract_notes(changelog: Path, version: str, output: Path) -> None:
    """The inline section of `version`, notices included, which the GitHub release publishes. §FS-002-release.3"""
    _validate_version(version)
    lines = _read_lines(changelog)
    sections = [index for index, line in enumerate(lines) if line.startswith("## ")]
    for index, start in enumerate(sections):
        match = RELEASE_RE.match(lines[start])
        if match is None or match.group("version") != version:
            continue
        end = sections[index + 1] if index + 1 < len(sections) else len(lines)
        body = _trim_trailing_blank_lines(_drop_leading_blank_lines(lines[start + 1 : end]))
        if not body:
            raise ChangelogError(f"release {version} has an empty changelog section")
        _write_lines(output, [*body[:-1], body[-1].rstrip("\r\n") + "\n"])
        return
    raise ChangelogError(f"release {version} is not the inline changelog release")


def _summary(body: Sequence[str]) -> str:
    """The archive pointer's one line: how many pull requests the rotated release shipped."""
    count = sum(1 for line in body if re.match(r"^- .*\(PR #[0-9]+\)\s*$", line))
    return f"{count} pull request{'' if count == 1 else 's'}." if count else "release notes."


def _rebase_links(line: str, source: Sequence[str], target: Sequence[str]) -> str:
    """Each relative link written in `source` rewritten to reach the same target from `target`. §FS-002-release.2.3

    Both are directories as components under `docs/`: the inline release is
    read in `docs/` itself and an archive in `changelog`. An anchor, an absolute
    path and a URL mean the same thing anywhere; a relative destination is
    resolved against where it was written and re-expressed from where it lands.
    """

    def rewrite(match: re.Match[str]) -> str:
        destination = match.group("destination")
        if destination.startswith(("/", "<")) or re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", destination):
            return match.group(0)
        # Rooted at placeholders well above `docs/`, so a link that climbs out of it keeps its `..`s.
        root = "/_/_/_/_/_/_/_/_/docs"
        resolved = posixpath.normpath(posixpath.join(root, *source, destination))
        rebased = posixpath.relpath(resolved, posixpath.join(root, *target))
        if destination.endswith("/") and rebased != ".":
            rebased += "/"
        return f"{match.group('prefix')}{rebased}{match.group('fragment') or ''})"

    return LINK_RE.sub(rewrite, line)


def _read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding="utf-8").splitlines(keepends=True)
    except FileNotFoundError as exc:
        raise ChangelogError(f"missing changelog: {path}") from exc


def _write_lines(path: Path, lines: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(lines), encoding="utf-8")


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


def _validate_version(version: str) -> None:
    if VERSION_RE.match(version) is None:
        raise ChangelogError(f"version must look like 0.1.0, got {version!r}")


def _validate_date(release_date: str) -> None:
    try:
        _datetime.date.fromisoformat(release_date)
    except ValueError as exc:
        raise ChangelogError(f"date must look like YYYY-MM-DD, got {release_date!r}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Write or read the release sections of docs/changelog.md.")
    parser.add_argument("--changelog", type=Path, default=Path("docs/changelog.md"))
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare = subparsers.add_parser("prepare", help="write a numbered release from the pull requests it ships")
    prepare.add_argument("version")
    prepare.add_argument("--date", default=_datetime.date.today().isoformat())

    notes = subparsers.add_parser("notes", help="write release notes for the inline release")
    notes.add_argument("version")
    notes.add_argument("--output", type=Path, required=True)

    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            prepare_release(args.changelog, args.version, args.date)
        elif args.command == "notes":
            extract_notes(args.changelog, args.version, args.output)
        else:
            raise AssertionError(args.command)
    except ReleaseRefused as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
