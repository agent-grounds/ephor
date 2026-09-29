#!/usr/bin/env python3
"""Prepare and read changelog release sections. §FS-002-release"""

from __future__ import annotations

import argparse
import datetime as _datetime
import re
import subprocess
import sys
from pathlib import Path
from typing import Sequence

# `__file__` is set under `python scripts/...` and under the test harness's
# load-by-path alike, so this reaches the shared cut from both (§FS-002-release.2).
sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_unreleased import bullet_blocks, unreleased_range  # noqa: E402


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
UNRELEASED_RE = re.compile(r"^## Unreleased\s*$")
RELEASE_RE = re.compile(
    r"^## (?P<number>[0-9]+)\. \[(?P<version>[0-9]+\.[0-9]+\.[0-9]+)\] — (?P<date>[0-9]{4}-[0-9]{2}-[0-9]{2})\s*$"
)
OLDER_RE = re.compile(r"^## (?P<number>[0-9]+)\. Older releases\s*$")
# The three spellings a written number takes, and the placeholder that is not one.
NUMBER_WRITTEN_RE = re.compile(r"(?i)\bPR\s*#\s*[0-9]+\b|\bpull request\s*#\s*[0-9]+\b|/pull/[0-9]+")
PLACEHOLDER_RE = re.compile(r"(?i)\bPR\s*#\s*TBD\b")
BLAME_HEADER_RE = re.compile(r"^(?P<sha>[0-9a-f]{40}) [0-9]+ (?P<final>[0-9]+)(?: [0-9]+)?$")


class ChangelogError(Exception):
    pass


def prepare_release(changelog: Path, version: str, release_date: str) -> None:
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

    unreleased_body = _trim_blank_lines(lines[unreleased + 1 : latest])
    if not _has_bullet(unreleased_body):
        raise ChangelogError("## Unreleased has no bullet entries to promote")

    previous_version = latest_match.group("version")
    previous_date = latest_match.group("date")
    previous_body = lines[latest + 1 : older]
    archived_body = [_rewrite_relative_links_for_archive(line) for line in previous_body]
    summary = _summary_from(previous_body)

    archive_path = changelog.parent / "changelog" / f"{previous_version}.md"
    if archive_path.exists():
        raise ChangelogError(f"archive already exists: {archive_path}")

    archive_lines = [f"# {previous_version} — {previous_date}\n", *archived_body]
    _write_lines(archive_path, archive_lines)

    older_body = lines[older + 1 :]
    older_body = _drop_leading_blank_lines(older_body)
    archive_link = f"- [{previous_version}](changelog/{previous_version}.md) — {previous_date}: {summary}\n"

    new_lines = [
        *lines[: unreleased + 1],
        "\n",
        f"## 2. [{version}] — {release_date}\n",
        "\n",
        *unreleased_body,
        "\n",
        "## 3. Older releases\n",
        "\n",
        archive_link,
        *older_body,
    ]
    _write_lines(changelog, new_lines)


def pull_requests_for_commit(sha: str) -> list[int]:
    """The pull requests a commit belongs to, asked of the forge. §FS-002-release.2"""
    result = subprocess.run(
        ["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls", "--jq", ".[].number"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [int(token) for token in result.stdout.split()]


def stamp_unreleased(changelog: Path) -> None:
    """Write the numbers the contributors did not. Never fails a release. §FS-002-release.2"""
    try:
        lines = _read_lines(changelog)
        section = unreleased_range(lines)
        if section is None:
            raise ChangelogError("missing ## Unreleased section")
        blocks = bullet_blocks(lines, *section)
    except ChangelogError as exc:
        # A release is never failed over stamping, not even by a changelog it
        # cannot read: `prepare` is the step that is allowed to refuse.
        print(f"warning: not stamping ## Unreleased: {exc}", file=sys.stderr)
        return

    resolved: dict[str, list[int]] = {}
    changed = False
    for start, stop in blocks:
        block = lines[start:stop]
        if NUMBER_WRITTEN_RE.search("".join(block)):
            continue  # written by hand always beats stamped, and is not verified
        number = _single_pull_request(changelog, start, stop, block, resolved)
        if number is None:
            continue
        lines[start:stop] = _stamp_block(block, number)
        changed = True

    if changed:
        _write_lines(changelog, lines)


def _single_pull_request(
    changelog: Path, start: int, stop: int, block: Sequence[str], resolved: dict[str, list[int]]
) -> int | None:
    """The one pull request every non-blank line of the bullet resolves to, or nothing."""
    first = _line_text(block[0]).strip()
    try:
        shas = _blame_shas(changelog, start + 1, stop)
        numbers = set()
        for offset, line in enumerate(block):
            if not line.strip():
                continue
            sha = shas.get(start + 1 + offset)
            if sha is None:
                numbers.add(None)
                continue
            if sha not in resolved:
                resolved[sha] = pull_requests_for_commit(sha)
            pulls = resolved[sha]
            numbers.add(pulls[0] if len(pulls) == 1 else None)
    except Exception as exc:  # a forge, a token, or a blame this release will not fail over
        print(f"warning: cannot resolve a pull request for {first!r}: {exc}", file=sys.stderr)
        return None

    if len(numbers) == 1:
        only = numbers.pop()
        if only is not None:
            return only
    print(
        f"warning: leaving {first!r} unstamped — its lines do not all resolve to one pull request",
        file=sys.stderr,
    )
    return None


def _stamp_block(block: Sequence[str], number: int) -> list[str]:
    """`PR #TBD` replaced in place, otherwise appended to the bullet's last written line."""
    stamped = list(block)
    for index, line in enumerate(stamped):
        if PLACEHOLDER_RE.search(line):
            stamped[index] = PLACEHOLDER_RE.sub(f"PR #{number}", line, count=1)
            return stamped

    for index in range(len(stamped) - 1, -1, -1):
        if not stamped[index].strip():
            continue
        body = stamped[index].rstrip("\r\n")
        ending = stamped[index][len(body) :]
        stamped[index] = f"{body} (PR #{number}){ending}"
        return stamped
    return stamped


def _blame_shas(changelog: Path, start: int, stop: int) -> dict[int, str]:
    """The commit behind each line of `changelog`, by one-based line number."""
    result = subprocess.run(
        ["git", "blame", "--porcelain", "-L", f"{start},{stop}", "--", str(changelog)],
        check=True,
        capture_output=True,
        text=True,
    )
    shas = {}
    for raw in result.stdout.splitlines():
        match = BLAME_HEADER_RE.match(raw)
        if match is not None:
            shas[int(match.group("final"))] = match.group("sha")
    return shas


def extract_notes(changelog: Path, version: str, output: Path) -> None:
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
    """Every `## ` heading, for `prepare` and `notes` — deliberately not the shared scan.

    `stamp` reads `## Unreleased` through `changelog_unreleased.unreleased_range`,
    because the gate has to agree with it about where a stampable bullet lives.
    `prepare` and `notes` want every top-level section rather than one range, and
    are outside this rule, so they keep the walk they have always had.
    """
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
    trimmed = list(lines)
    while trimmed and not trimmed[0].strip():
        trimmed.pop(0)
    while trimmed and not trimmed[-1].strip():
        trimmed.pop()
    if trimmed and not trimmed[-1].endswith(("\n", "\r")):
        trimmed[-1] += "\n"
    return trimmed


def _drop_leading_blank_lines(lines: Sequence[str]) -> list[str]:
    trimmed = list(lines)
    while trimmed and not trimmed[0].strip():
        trimmed.pop(0)
    return trimmed


def _has_bullet(lines: Sequence[str]) -> bool:
    return any(line.lstrip().startswith("- ") for line in lines)


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


def _rewrite_relative_links_for_archive(line: str) -> str:
    def rewrite(match: re.Match[str]) -> str:
        destination = match.group("destination")
        if destination.startswith(("#", "/", "../", "http://", "https://", "mailto:")):
            return match.group(0)
        fragment = match.group("fragment") or ""
        return f"{match.group('prefix')}../{destination}{fragment})"

    return re.sub(
        r"(?P<prefix>\]\()(?P<destination>[^)#][^)#]*)(?P<fragment>#[^)]*)?\)",
        rewrite,
        line,
    )


def _validate_version(version: str) -> None:
    if VERSION_RE.match(version) is None:
        raise ChangelogError(f"version must look like 0.1.0, got {version!r}")


def _validate_date(release_date: str) -> None:
    try:
        _datetime.date.fromisoformat(release_date)
    except ValueError as exc:
        raise ChangelogError(f"date must look like YYYY-MM-DD, got {release_date!r}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Prepare or read docs/changelog.md release sections.")
    parser.add_argument("--changelog", type=Path, default=Path("docs/changelog.md"))
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare = subparsers.add_parser("prepare", help="promote Unreleased into a numbered release")
    prepare.add_argument("version")
    prepare.add_argument("--date", default=_datetime.date.today().isoformat())

    notes = subparsers.add_parser("notes", help="write release notes for the inline release")
    notes.add_argument("version")
    notes.add_argument("--output", type=Path, required=True)

    subparsers.add_parser("stamp", help="write PR numbers onto Unreleased bullets that carry none")

    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            prepare_release(args.changelog, args.version, args.date)
        elif args.command == "stamp":
            stamp_unreleased(args.changelog)
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
