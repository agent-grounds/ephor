"""What the changelog scripts ask git: trees, files at a commit, and where each entry landed. §FS-002-release.2.1

The gate compares the entries two commits hold (§FS-002-release.6) and the
release orders and attributes them by the history that added them
(§FS-002-release.2.1). Both read the repository the script runs in, through
the one wrapper here, and neither reads the checkout's files for a commit's.

Paths are compared in one canonical form: a path the caller names is resolved
and taken relative to the resolved `git rev-parse --show-toplevel`, because on
macOS `TMPDIR` is a symlink and the raw and resolved spellings of one directory
are two different strings.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import NamedTuple, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_unreleased import entry_name  # noqa: E402


class GitError(Exception):
    """Git refused, in its own words."""


class HistoryUnavailable(Exception):
    """The history the release orders and attributes by is not there to read. §FS-002-release.2.1"""


class Landing(NamedTuple):
    """Where an entry's lifetime began: the commit that added its slug, and its file name then."""

    position: int  # how far down main's first-parent history, 0 the oldest
    commit: str
    name: str


def git(arguments: Sequence[str]) -> str:
    """git's stdout as UTF-8 text, or `GitError` carrying git's stderr."""
    return git_bytes(arguments).decode("utf-8", errors="replace")


def git_bytes(arguments: Sequence[str]) -> bytes:
    try:
        result = subprocess.run(["git", *arguments], check=False, capture_output=True)
    except FileNotFoundError as exc:
        raise GitError("git is not on PATH") from exc
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise GitError(f"git {' '.join(arguments)} failed: {detail or 'no output'}")
    return result.stdout


def rev_parse(revision: str) -> str | None:
    """`revision` as a commit sha, or `None` where nothing resolves."""
    try:
        return git(["rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}"]).strip() or None
    except GitError:
        return None


def merge_base(revision: str) -> str | None:
    try:
        return git(["merge-base", "HEAD", revision]).strip() or None
    except GitError:
        return None


def repository_path(path: Path) -> str:
    """`path` as git names it, from the repository root down, in posix form."""
    toplevel = Path(git(["rev-parse", "--show-toplevel"]).strip()).resolve()
    absolute = path if path.is_absolute() else Path.cwd() / path
    # Resolve the deepest part that exists, so a directory not created yet still lands inside the tree.
    existing = absolute
    while not existing.exists() and existing != existing.parent:
        existing = existing.parent
    resolved = existing.resolve() / absolute.relative_to(existing)
    try:
        return resolved.relative_to(toplevel).as_posix()
    except ValueError as exc:
        raise GitError(f"{path.as_posix()} is outside the repository at {toplevel}") from exc


def files_at(commit: str, directory: str) -> dict[str, str]:
    """The files under `directory` at `commit`, by path relative to it, each its blob id."""
    listing = git(["ls-tree", "-r", "--full-tree", "-z", commit, "--", f"{directory}/"])
    files = {}
    for record in filter(None, listing.split("\0")):
        description, _, path = record.partition("\t")
        files[path[len(directory) + 1 :]] = description.split()[2]
    return files


def blob_text(blob: str) -> str | None:
    """A blob's text, or `None` where it is not UTF-8."""
    try:
        return git_bytes(["cat-file", "blob", blob]).decode("utf-8")
    except UnicodeDecodeError:
        return None


def file_at(commit: str, path: str) -> str | None:
    """The file's text at `commit`, or `None` where that commit does not hold it."""
    try:
        return git_bytes(["show", f"{commit}:{path}"]).decode("utf-8", errors="replace")
    except GitError:
        return None


def first_parent(commit: str) -> str | None:
    return rev_parse(f"{commit}^1")


def renames(base: str, head: str, directory: str) -> list[tuple[str, str]]:
    """The files under `directory` git sees renamed between two commits, as (old, new) names."""
    output = git(["diff", "-z", "--name-status", "--find-renames", base, head, "--", f"{directory}/"])
    fields = output.split("\0")
    found = []
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        if status.startswith(("R", "C")):
            old, new = fields[index + 1], fields[index + 2]
            if status.startswith("R"):
                found.append((old[len(directory) + 1 :], new[len(directory) + 1 :]))
            index += 3
        else:
            index += 2
    return found


def is_shallow() -> bool:
    return git(["rev-parse", "--is-shallow-repository"]).strip() == "true"


def landings(directory: str) -> dict[str, Landing]:
    """For each slug HEAD's history holds, the commit that began its lifetime. §FS-002-release.2.1

    Main's first-parent history of the directory is walked oldest first, a merge
    read against its first parent so the entries a merged branch brought land at
    the merge. A slug lands at the commit after which a file of that slug exists
    and before which none did, so an edit, or a change of category made in one
    commit, keeps it where it landed; and a slug whose last file a release
    deleted lands again wherever it is used next. History rather than blame,
    because an edit by another pull request splits a blame.
    """
    try:
        shallow = is_shallow()
    except GitError as exc:
        raise HistoryUnavailable(f"git cannot read the history: {exc}") from exc
    if shallow:
        raise HistoryUnavailable("this clone is shallow, so where each entry landed is not in it")
    walk = ["log", "--first-parent", "-m", "--no-renames", "--name-status", "--format=@%H", "--reverse"]
    try:
        log = git([*walk, "HEAD", "--", f"{directory}/"])
    except GitError as exc:
        raise HistoryUnavailable(str(exc)) from exc

    present: dict[str, set[str]] = {}
    landed: dict[str, Landing] = {}
    position = -1
    commit = ""
    changes: list[tuple[str, str]] = []

    def settle() -> None:
        before = {slug for slug, names in present.items() if names}
        for status, name in changes:
            parsed = entry_name(name)
            if parsed is None:
                continue
            names = present.setdefault(parsed.slug, set())
            if status == "D":
                names.discard(name)
            elif status == "A":
                names.add(name)
        for slug, names in present.items():
            if names and slug not in before:
                landed[slug] = Landing(position, commit, min(names))
            elif not names:
                landed.pop(slug, None)

    for line in log.splitlines():
        if line.startswith("@"):
            if commit:
                settle()
            position, commit, changes = position + 1, line[1:], []
        elif "\t" in line:
            status, _, path = line.partition("\t")
            if path.startswith(f"{directory}/"):
                changes.append((status[:1], path[len(directory) + 1 :]))
    if commit:
        settle()
    return landed
