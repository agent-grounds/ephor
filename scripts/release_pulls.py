"""The range a release covers and the pull requests that landed on it. §FS-002-release.1.3

The range is read from git: the committed `HEAD`, which must lie on the
first-parent history of the fetched default branch, back to the nearest
`vX.Y.Z` tag on that history, or to the root commit before the first release.
The pull requests are read from the forge through `gh api`, with the REST
endpoints only, and a pull request is in the range by the commit it landed as.
Every listing is checked whole against the forge's own count, so a listing that
stopped early refuses the release rather than shortening it (§FS-002-release.2.3).

Nothing here writes. Every failure is a `ReleaseRefused` naming the reason.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import NamedTuple, Sequence

from release_paths import is_substantive


VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
TAG_RE = re.compile(r"^v(?P<version>[0-9]+\.[0-9]+\.[0-9]+)$")
GITHUB_REMOTE_RE = re.compile(r"github\.com[:/]+(?P<owner>[^/\s]+)/(?P<name>[^/\s]+?)(?:\.git)?/?$")
# What the forge lists of one pull request's files, at most (§FS-002-release.1.3).
FILE_CAP = 3000
PER_PAGE = "100"
# Each of these is escaped in a title, so it reads as typed and cannot close the link.
TITLE_SPECIAL_RE = re.compile(r"([\\`*_\[\]<>])")


class ReleaseRefused(Exception):
    """A release that cannot be prepared, and why. §FS-002-release.2.3"""


class Range(NamedTuple):
    """First-parent commits from `HEAD` back to the previous tag, excluded, or the root, included."""

    head: str
    commits: tuple[str, ...]  # newest first
    tag: str | None
    tag_commit: str | None

    def describe(self) -> str:
        return f"since {self.tag}" if self.tag else "from the root commit"


class Pull(NamedTuple):
    number: int
    title: str
    url: str
    merged_at: str


def version_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


# --- git -------------------------------------------------------------------


def git(root: Path, *args: str) -> str:
    result = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if result.returncode != 0:
        raise ReleaseRefused(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def repository_root(start: Path) -> Path:
    return Path(git(start, "rev-parse", "--show-toplevel").strip())


def release_range(root: Path, default_branch: str) -> Range:
    """The range §FS-002-release.1.3 defines, or a refusal when the history cannot vouch for it."""
    if git(root, "rev-parse", "--is-shallow-repository").strip() == "true":
        raise ReleaseRefused("the history is shallow; a release reads its range from the whole history (fetch-depth: 0)")
    head = git(root, "rev-parse", "HEAD^{commit}").strip()
    remote = f"refs/remotes/origin/{default_branch}"
    if subprocess.run(["git", "rev-parse", "--verify", "--quiet", remote], cwd=root, capture_output=True).returncode:
        raise ReleaseRefused(f"origin/{default_branch} is not fetched; the release is drawn from the default branch")
    if head not in git(root, "rev-list", "--first-parent", remote).split():
        raise ReleaseRefused(
            f"HEAD {head[:12]} is not on the first-parent history of origin/{default_branch}; "
            f"a release runs on the default branch"
        )

    tags: dict[str, str] = {}
    listing = git(root, "for-each-ref", "refs/tags", "--format=%(refname:strip=2) %(objectname) %(*objectname)")
    for row in listing.splitlines():
        name, target, *peeled = row.split()
        match = TAG_RE.match(name)
        if match is None:
            continue
        commit = peeled[0] if peeled else target
        if commit not in tags or version_key(match.group("version")) > version_key(tags[commit][1:]):
            tags[commit] = name

    commits: list[str] = []
    for commit in git(root, "rev-list", "--first-parent", "HEAD").split():
        if commit in tags:
            return Range(head, tuple(commits), tags[commit], commit)
        commits.append(commit)
    return Range(head, tuple(commits), None, None)


# --- the forge -------------------------------------------------------------


def repository(root: Path) -> str:
    """`owner/name`: what `GITHUB_REPOSITORY` names, otherwise the GitHub repository origin points at."""
    named = os.environ.get("GITHUB_REPOSITORY", "").strip()
    if named:
        return named
    url = git(root, "remote", "get-url", "origin").strip()
    match = GITHUB_REMOTE_RE.search(url)
    if match is None:
        raise ReleaseRefused(f"origin ({url}) is not a GitHub repository and GITHUB_REPOSITORY is unset")
    return f"{match.group('owner')}/{match.group('name')}"


class Forge:
    """The REST reads a release makes, each refused in the forge's own words when it fails."""

    def __init__(self, repo: str) -> None:
        if shutil.which("gh") is None:
            raise ReleaseRefused("`gh` is not on PATH; a release reads the pull requests it ships from the forge")
        self.repo = repo

    def get(self, endpoint: str, *params: str, paginate: bool = False):
        args = ["gh", "api", "-X", "GET", "-H", "Accept: application/vnd.github+json", endpoint]
        for param in params:
            args += ["-f", param]
        if paginate:
            args += ["--paginate", "--slurp"]
        try:
            result = subprocess.run(args, capture_output=True, text=True)
        except OSError as exc:
            raise ReleaseRefused(f"`gh api {endpoint}` could not run: {exc}") from exc
        if result.returncode != 0:
            raise ReleaseRefused(f"the forge failed `gh api {endpoint}`: {result.stderr.strip()}")
        try:
            body = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise ReleaseRefused(f"the forge answered `gh api {endpoint}` with no JSON: {exc}") from exc
        return [item for page in body for item in page] if paginate else body

    def default_branch(self) -> str:
        return self.get(f"repos/{self.repo}")["default_branch"]

    def merged_pulls(self, branch: str) -> list[dict]:
        """Every pull request merged into `branch`, checked against the forge's own total."""
        listed = self.get(f"repos/{self.repo}/pulls", "state=closed", f"base={branch}", f"per_page={PER_PAGE}", paginate=True)
        merged = [pull for pull in listed if pull.get("merged_at")]
        search = self.get("search/issues", f"q=repo:{self.repo} is:pr is:merged base:{branch}", "per_page=1")
        if search.get("incomplete_results") or search.get("total_count") != len(merged):
            raise ReleaseRefused(
                f"the forge listed {len(merged)} pull requests merged into {branch} but counts "
                f"{search.get('total_count')}; the listing is incomplete"
            )
        return merged

    def files(self, number: int) -> list[dict]:
        """Every file pull request `number` changed, checked against its `changed_files`."""
        expected = self.get(f"repos/{self.repo}/pulls/{number}")["changed_files"]
        if expected > FILE_CAP:
            raise ReleaseRefused(
                f"pull request #{number} changed {expected} files, past the {FILE_CAP} the forge lists; "
                f"whether it is substantive cannot be read"
            )
        files = self.get(f"repos/{self.repo}/pulls/{number}/files", f"per_page={PER_PAGE}", paginate=True)
        if len(files) != expected:
            raise ReleaseRefused(
                f"the forge listed {len(files)} files of pull request #{number}, which changed {expected}; "
                f"the listing is incomplete"
            )
        return files


def landed_pulls(forge: Forge, branch: str, span: Range) -> list[Pull]:
    """The qualifying pull requests landed on `span`, newest merge first. §FS-002-release.1.3"""
    commits = set(span.commits)
    landed = [pull for pull in forge.merged_pulls(branch) if pull.get("merge_commit_sha") in commits]
    selected: list[Pull] = []
    for pull in landed:
        number = pull["number"]
        if not any(is_substantive(entry["filename"]) for entry in forge.files(number)):
            continue
        url = pull.get("html_url", "")
        if url != f"https://github.com/{forge.repo}/pull/{number}":
            raise ReleaseRefused(f"pull request #{number} links to {url}, outside {forge.repo}")
        selected.append(Pull(number, pull["title"], url, pull["merged_at"]))
    if not selected:
        why = (
            f"{len(landed)} landed, none with a substantive path"
            if landed
            else "none landed on it; its commits were pushed without one"
            if span.commits
            else "it holds no commit"
        )
        raise ReleaseRefused(f"no pull request qualifies for the range {span.describe()}: {why}")
    return sorted(selected, key=lambda pull: (pull.merged_at, pull.number), reverse=True)


def release_line(pull: Pull) -> str:
    """`- [<title>](<url>) (PR #N)`, the title flattened and escaped as literal link text. §FS-002-release.1.3"""
    title = TITLE_SPECIAL_RE.sub(r"\\\1", " ".join(pull.title.split()))
    return f"- [{title}]({pull.url}) (PR #{pull.number})"


def release_lines(pulls: Sequence[Pull]) -> list[str]:
    return [release_line(pull) for pull in pulls]
