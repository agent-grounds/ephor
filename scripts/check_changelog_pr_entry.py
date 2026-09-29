#!/usr/bin/env python3
"""Require a pull request to add or change an Unreleased changelog bullet. §FS-002-release.6

One predicate reached at two moments: the pre-push hook, against the base
branch, and CI, against the pull request's own base commit. Neither asks for a
number that does not exist yet (§FS-002-release.1) — the release stamps it
(§FS-002-release.2).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple, Sequence

# `__file__` is set under `python scripts/...` and under the test harness's
# load-by-path alike, so this reaches the shared cut from both (§FS-002-release.6).
sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_unreleased import bullet_blocks, unreleased_range  # noqa: E402


HUNK_RE = re.compile(r"^@@ -[0-9]+(?:,[0-9]+)? \+(?P<start>[0-9]+)(?:,[0-9]+)? @@")

# The scanner for "a pull request number is written here". These are the three
# spellings the gate has always recognised; what changed is that they now
# report which number they found rather than testing for one we already have.
PR_NUMBER_PATTERNS = (
    re.compile(r"(?i)\bPR\s*#\s*([0-9]+)\b"),
    re.compile(r"(?i)\bpull request\s*#\s*([0-9]+)\b"),
    re.compile(r"/pull/([0-9]+)(?:\b|[/#?)])"),
)

# Where the local hook looks for a base, in order. A fork clone that has never
# fetched this repository has none of them, which is the degraded case.
BASE_CANDIDATES = ("origin/main", "upstream/main", "main")

BYPASS = "  To push anyway: SKIP=changelog-pr-entry git push"

# With no pull request template and no CONTRIBUTING.md in this repository, the
# refusal is the contributor-facing documentation for the rule (§FS-002-release.6).
GUIDANCE = """
  Every pull request adds one bullet saying what a user will notice. Add it under
  ## Unreleased in the right section (Added / Changed / Deprecated / Removed / Fixed /
  Security), for example:

      ### Fixed

      - **`ephor refresh` no longer drops a project whose remote is unreachable.**

  You do not need the pull request number — the release fills it in. If you know it, or
  want a placeholder, `PR #12` and `PR #TBD` are both fine."""


class ChangelogPrError(Exception):
    pass


class AddedLine(NamedTuple):
    """A line this change adds or alters inside `## Unreleased`."""

    number: int  # its line number in the changelog as this change leaves it
    text: str
    in_bullet: bool  # so that adding a bare `### Fixed` is not a pass


def added_unreleased_lines(base_rev: str, changelog: Path) -> list[AddedLine]:
    """The `## Unreleased` lines this change adds or alters, against `base_rev`. §FS-002-release.6"""
    lines = _changelog_lines(changelog)
    start, end = _unreleased_range(lines)
    bullet_lines = {
        index for block_start, block_end in bullet_blocks(lines, start, end) for index in range(block_start, block_end)
    }

    added = []
    for number, text in _added_lines(base_rev, changelog):
        index = number - 1
        if start <= index < end:
            added.append(AddedLine(number, text, index in bullet_lines))
    return added


def check_unreleased_entry(added: Sequence[AddedLine], pr_number: int | None) -> None:
    """A bullet is what is asked for; a number, only if one is written. §FS-002-release.6"""
    if not any(line.in_bullet for line in added):
        raise ChangelogPrError(
            "docs/changelog.md has no new or changed bullet under ## Unreleased.\n" + GUIDANCE
        )
    if pr_number is None:
        return
    for line in added:
        for written in _pr_numbers(line.text):
            if written != pr_number:
                raise ChangelogPrError(
                    f"docs/changelog.md ## Unreleased: line {line.number} names PR #{written}, "
                    f"but this is pull request #{pr_number}.\n"
                    "\n"
                    f"    {line.text.strip()}\n"
                    "\n"
                    f"  Write `PR #{pr_number}`, write `PR #TBD`, or leave the number out entirely — the\n"
                    "  release fills it in. Only the lines this pull request adds are checked; the\n"
                    "  numbers on bullets that were already here are not yours to fix."
                )


def check_any_bullet(changelog: Path) -> None:
    """The degraded check: `## Unreleased` carries a bullet at all. §FS-002-release.6"""
    lines = _changelog_lines(changelog)
    start, end = _unreleased_range(lines)
    if not bullet_blocks(lines, start, end):
        raise ChangelogPrError("docs/changelog.md ## Unreleased has no bullets at all.\n" + GUIDANCE)


def resolve_base(explicit: str | None) -> tuple[str | None, list[str]]:
    """The commit to compare against, and every candidate tried getting there. §FS-002-release.6"""
    if explicit is not None:
        # CI passes the pull request's own base commit, and `actions/checkout`
        # gives it the head branch already merged into that commit — so the
        # two-dot diff is exactly the pull request's net change and no merge
        # base is needed. Locally there is no merge commit, so one is.
        resolved = _rev_parse(explicit)
        return resolved, [explicit]

    tried = []
    for candidate in BASE_CANDIDATES:
        tried.append(candidate)
        resolved = _rev_parse(candidate)
        if resolved is None:
            continue
        merge_base = _merge_base(resolved)
        if merge_base is not None:
            return merge_base, tried
    return None, tried


def pr_number_from_event(event_path: Path) -> int | None:
    try:
        event = json.loads(event_path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ChangelogPrError(f"missing GitHub event file: {event_path}") from exc
    except json.JSONDecodeError as exc:
        raise ChangelogPrError(f"invalid GitHub event JSON: {event_path}: {exc}") from exc

    pull_request = event.get("pull_request")
    if isinstance(pull_request, dict):
        number = pull_request.get("number")
    else:
        number = event.get("number") if event.get("pull_request") is not None else None

    if number is None:
        return None
    if not isinstance(number, int) or number <= 0:
        raise ChangelogPrError(f"invalid pull request number in event: {number!r}")
    return number


def pr_number_from_current_branch() -> int | None:
    try:
        result = subprocess.run(
            ["gh", "pr", "view", "--json", "number", "--jq", ".number"],
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        return None

    if result.returncode != 0:
        return None
    output = result.stdout.strip()
    if not output:
        return None
    try:
        number = int(output)
    except ValueError as exc:
        raise ChangelogPrError(f"invalid pull request number from gh: {output!r}") from exc
    if number <= 0:
        raise ChangelogPrError(f"invalid pull request number from gh: {number!r}")
    return number


def _changelog_lines(changelog: Path) -> list[str]:
    try:
        return changelog.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise ChangelogPrError(f"missing changelog: {changelog}") from exc


def _unreleased_range(lines: Sequence[str]) -> tuple[int, int]:
    """The body of `## Unreleased`, in this check's own error vocabulary."""
    section = unreleased_range(lines)
    if section is None:
        raise ChangelogPrError("missing ## Unreleased section in docs/changelog.md")
    return section


def _added_lines(base_rev: str, changelog: Path) -> list[tuple[int, str]]:
    diff = _git(["diff", "--unified=0", base_rev, "--", str(changelog)])
    added: list[tuple[int, str]] = []
    number = 0
    in_hunk = False
    for raw in diff.splitlines():
        hunk = HUNK_RE.match(raw)
        if hunk is not None:
            number = int(hunk.group("start"))
            in_hunk = True
            continue
        if not in_hunk or not raw.startswith("+"):
            continue
        added.append((number, raw[1:]))
        number += 1
    return added


def _pr_numbers(text: str) -> list[int]:
    return [int(match) for pattern in PR_NUMBER_PATTERNS for match in pattern.findall(text)]


def _git(args: Sequence[str]) -> str:
    try:
        result = subprocess.run(["git", *args], check=False, capture_output=True, text=True)
    except FileNotFoundError as exc:
        raise ChangelogPrError("git is not on PATH") from exc
    if result.returncode != 0:
        raise ChangelogPrError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def _rev_parse(rev: str) -> str | None:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip() or None


def _merge_base(rev: str) -> str | None:
    result = subprocess.run(
        ["git", "merge-base", "HEAD", rev], check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip() or None


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check that this change adds or alters a bullet under ## Unreleased."
    )
    parser.add_argument("--changelog", type=Path, default=Path("docs/changelog.md"))
    parser.add_argument("--pr-number", type=int)
    parser.add_argument("--event-path", type=Path, default=None)
    parser.add_argument(
        "--base-rev",
        default=None,
        help="the commit to compare against; CI passes the pull request's own base",
    )
    parser.add_argument(
        "--local-pr",
        action="store_true",
        help="run as the pre-push hook: resolve the branch's PR number with gh if it has one",
    )
    args = parser.parse_args(argv)

    compared_against = None
    try:
        pr_number = args.pr_number
        if pr_number is None:
            event_path = args.event_path
            if event_path is None:
                raw_event_path = os.environ.get("GITHUB_EVENT_PATH")
                event_path = Path(raw_event_path) if raw_event_path else None
            pr_number = pr_number_from_event(event_path) if event_path is not None else None

        if pr_number is None and args.local_pr:
            pr_number = pr_number_from_current_branch()
        if pr_number is not None and pr_number <= 0:
            raise ChangelogPrError(f"invalid pull request number: {pr_number}")

        # A number nobody has yet is no longer a reason to stand down: the
        # bullet is checked either way, and only the number clause needs one.
        base_rev, tried = resolve_base(args.base_rev)
        if base_rev is None:
            print(
                "changelog PR entry: could not resolve a base to compare against\n"
                f"  (tried {', '.join(tried)}), so this only checked that\n"
                "  ## Unreleased has a bullet at all — not that you added one.\n"
                "  CI will compare against the pull request's base."
            )
            check_any_bullet(args.changelog)
        else:
            compared_against = tried[-1]
            check_unreleased_entry(added_unreleased_lines(base_rev, args.changelog), pr_number)
    except ChangelogPrError as exc:
        print(f"error: {exc}", file=sys.stderr)
        if compared_against is not None:
            print(
                f"\n  (compared against {compared_against}; conventions are in docs/changelog.md"
                " section 1.2)",
                file=sys.stderr,
            )
        if args.local_pr:
            print(f"\n{BYPASS}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
