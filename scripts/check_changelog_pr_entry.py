#!/usr/bin/env python3
"""Require a pull request to add a changelog entry of its own. §FS-002-release.6

One predicate reached at two moments: the pre-push hook, against the base
branch, and CI, against the pull request's own base commit. Both ask whether
the change adds an entry whose slug the base did not have, whether every entry
it adds or changes is well-formed (§FS-002-release.1.1), and whether any pull
request number it newly writes is its own. Neither asks for a number that does
not exist yet (§FS-002-release.1) — the release stamps it (§FS-002-release.2.2).

Both sides of the comparison are read out of commits, not the checkout: on a
`pull_request` event the checkout is the head merged into the base, and before
a push an uncommitted file is not one the push carries.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Sequence

# `__file__` is set under `python scripts/...` and under the test harness's
# load-by-path alike, so this reaches the shared reader from both (§FS-002-release.6).
sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_history import (  # noqa: E402
    GitError,
    blob_text,
    file_at,
    files_at,
    merge_base,
    renames,
    repository_path,
    rev_parse,
)
from changelog_switchover import SharedSection  # noqa: E402
from changelog_unreleased import ENTRY_README, entry_directory, entry_name, entry_problem, pr_numbers  # noqa: E402


# Where the local hook looks for a base, in order. A fork clone that has never
# fetched this repository has none of them, which is the degraded case.
BASE_CANDIDATES = ("origin/main", "upstream/main", "main")

BYPASS = "  To push anyway: SKIP=changelog-pr-entry git push"


class ChangelogPrError(Exception):
    pass


def guidance(directory: str) -> str:
    """With no pull request template and no CONTRIBUTING.md here, the refusal is the rule's documentation. §FS-002-release.6"""
    return f"""

  Every pull request adds one file under {directory}/ saying what a user will
  notice: one bullet, in a file named <slug>.<category>.md. For this branch, for example:

      {directory}/{suggested_slug()}.fixed.md

      - **`ephor refresh` no longer drops a project whose remote is unreachable.**

  The slug is yours, lowercase letters, digits and hyphens, and stays with the entry;
  the category is one of added, changed, deprecated, removed, fixed, security or note.
  You do not need the pull request number — the release fills it in. If you know it,
  or want a placeholder, end the bullet with `(PR #12)` or `(PR #TBD)`. Editing an
  entry that is already there does not count. The format is {directory}/README.md."""


def suggested_slug() -> str:
    """A slug from the branch's name, which is a good default and not the contributor's only choice."""
    branch = os.environ.get("GITHUB_HEAD_REF") or ""
    if not branch:
        result = subprocess.run(["git", "rev-parse", "--abbrev-ref", "HEAD"], check=False, capture_output=True, text=True)
        branch = result.stdout.strip() if result.returncode == 0 else ""
    slug = re.sub(r"[^a-z0-9]+", "-", branch.lower()).strip("-")
    return slug if slug and slug != "head" else "my-change"


def check_entries(base: str, directory: str, changelog: str, pr_number: int | None) -> None:
    """The base-relative predicate, the same at both moments. §FS-002-release.6"""
    base_files = files_at(base, directory)
    head_files = files_at("HEAD", directory)
    # A file that was already there and is unchanged is not this change's to judge.
    touched = sorted(name for name, blob in head_files.items() if name != ENTRY_README and base_files.get(name) != blob)
    texts = {name: blob_text(head_files[name]) for name in touched}
    base_slugs = _slugs(base_files)
    head_slugs = _slugs(head_files)

    problems = []
    # §FS-002-release.1.1: every entry the change adds or changes is held to the format.
    malformed = {name for name in touched if entry_problem(name, texts[name]) is not None}
    problems += [f"{directory}/{entry_problem(name, texts[name])}" for name in sorted(malformed)]
    for slug, names in sorted(head_slugs.items()):
        if len(names) > 1 and any(name in touched for name in names):
            problems.append(
                f"{directory}/: the slug `{slug}` is used by {' and '.join(names)}; a slug is unique across every category"
            )
    # §FS-002-release.2.1: the slug carries the entry's lifetime, so changing it is refused outright.
    for old, new in renames(base, "HEAD", directory):
        before, after = entry_name(old), entry_name(new)
        if before and after and before.slug != after.slug and before.slug not in head_slugs:
            problems.append(
                f"{directory}/{old} is renamed to {new}: an entry keeps its slug for life, because the release "
                f"orders and attributes it by its slug. Keep `{before.slug}` and change only the text or the "
                "category; a new change is a new entry."
            )

    # §FS-002-release.1.2: a copy of a bullet the base's shared section held is neither added nor numbered here.
    shared = SharedSection(file_at(base, changelog))
    copies = set()
    added = []
    for name in touched:
        parsed = entry_name(name)
        if name in malformed or parsed is None or parsed.slug in base_slugs:
            continue
        if shared and shared.take(parsed.category, texts[name]):
            copies.add(name)
        else:
            added.append(name)

    if pr_number is not None:
        for name in touched:
            parsed = entry_name(name)
            if name in copies or parsed is None or texts[name] is None:
                continue
            # A number the entry already carried at the base is not this change's to fix.
            carried = {number for old in base_slugs.get(parsed.slug, []) for number in pr_numbers(_text(base_files[old]))}
            for written in dict.fromkeys(pr_numbers(texts[name])):
                if written != pr_number and written not in carried:
                    problems.append(
                        f"{directory}/{name} names PR #{written}, but this is pull request #{pr_number}. Write "
                        f"`(PR #{pr_number})`, write `(PR #TBD)`, or leave the number out — the release fills it in."
                    )

    if not added:
        problems.insert(0, f"this change adds no entry of its own under {directory}/.")
    if problems:
        raise ChangelogPrError("\nerror: ".join(problems) + (guidance(directory) if not added else ""))


def check_any_entry(directory: str) -> None:
    """The degraded check, with no base to compare against: a well-formed entry is there at all. §FS-002-release.6

    Whether this change added it cannot be told, and neither can whether a
    malformed one beside it is this change's, so only the absence of any
    well-formed entry refuses — the local gate never refuses what CI would pass.
    """
    files = files_at("HEAD", directory)
    problems = [entry_problem(name, blob_text(blob)) for name, blob in sorted(files.items()) if name != ENTRY_README]
    if any(problem is None for problem in problems):
        return
    found = "".join(f"\nerror: {directory}/{problem}" for problem in problems)
    raise ChangelogPrError(f"{directory}/ holds no well-formed entry.{found}" + guidance(directory))


def resolve_base(explicit: str | None) -> tuple[str | None, list[str]]:
    """The commit to compare against, and every candidate tried getting there. §FS-002-release.6"""
    if explicit is not None:
        # CI passes the pull request's own base commit, and `actions/checkout`
        # gives it the head branch already merged into that commit — so
        # comparing the two trees is exactly the pull request's net change and
        # no merge base is needed. Locally there is no merge commit, so one is.
        return rev_parse(explicit), [explicit]

    tried = []
    for candidate in BASE_CANDIDATES:
        tried.append(candidate)
        resolved = rev_parse(candidate)
        if resolved is None:
            continue
        found = merge_base(resolved)
        if found is not None:
            return found, tried
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


def _slugs(files: dict[str, str]) -> dict[str, list[str]]:
    """The entries' file names by slug, the README and anything not named as an entry aside."""
    slugs: dict[str, list[str]] = {}
    for name in sorted(files):
        parsed = entry_name(name)
        if parsed is not None:
            slugs.setdefault(parsed.slug, []).append(name)
    return slugs


def _text(blob: str) -> str:
    return blob_text(blob) or ""


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check that this change adds an entry under docs/changelog/unreleased/."
    )
    parser.add_argument(
        "--changelog",
        type=Path,
        default=Path("docs/changelog.md"),
        help="the changelog; its entries are the changelog/unreleased/ directory beside it",
    )
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
    directory = "docs/changelog/unreleased"
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

        directory = repository_path(entry_directory(args.changelog))
        # A number nobody has yet is no reason to stand down: the entry is
        # checked either way, and only the number clause needs one.
        base_rev, tried = resolve_base(args.base_rev)
        if base_rev is None:
            print(
                "changelog PR entry: could not resolve a base to compare against\n"
                f"  (tried {', '.join(tried)}), so this only checked that\n"
                f"  {directory}/ holds a well-formed entry — not that you added one.\n"
                "  CI will compare against the pull request's base."
            )
            check_any_entry(directory)
        else:
            compared_against = tried[-1]
            check_entries(base_rev, directory, repository_path(args.changelog), pr_number)
    except (ChangelogPrError, GitError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        if compared_against is not None:
            print(f"\n  (compared against {compared_against}; the format is {directory}/README.md)", file=sys.stderr)
        if args.local_pr:
            print(f"\n{BYPASS}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
