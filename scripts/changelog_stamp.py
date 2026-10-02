"""Stamping the pull request numbers the contributors could not know. §FS-002-release.2.2

An entry's number comes from the commit that began its lifetime
(§FS-002-release.2.1): where that commit resolves to exactly one pull request,
the number is written at the entry's end and nowhere else. Everything else — no
pull request, more than one, a forge that refuses, an entry that cannot be read
or written, a history that is not there — leaves the entry exactly as written
and says so in the workflow log, in the failing tool's own words. Stamping never
fails a release; `prepare` is the step that may refuse.

A copy the switch-over moved (§FS-002-release.1.2) began its lifetime in the
switch-over, whose pull request is not where it came from, so it is never
stamped: it keeps the number it moved with or is released once without one.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_history import (  # noqa: E402
    GitError,
    HistoryUnavailable,
    Landing,
    file_at,
    first_parent,
    landings,
    repository_path,
)
from changelog_switchover import SharedSection  # noqa: E402
from changelog_unreleased import (  # noqa: E402
    ENTRY_README,
    entry_directory,
    entry_lines,
    entry_name,
    entry_problem,
    pr_numbers,
    stamped,
    trailing_number,
)


def pull_requests_for_commit(sha: str) -> list[int]:
    """The pull requests a commit belongs to, asked of the forge. §FS-002-release.2.2"""
    result = subprocess.run(
        ["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls", "--jq", ".[].number"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [int(token) for token in result.stdout.split()]


def stamp_entries(changelog: Path) -> None:
    """Write the numbers the contributors did not. Never fails a release. §FS-002-release.2.2"""
    try:
        Stamper(changelog).run()
    except Exception as exc:
        # Never failing a release is a property of this whole step, not of a
        # list of statements inside it: a refused token, a history git will not
        # read, a filesystem that will not take the write.
        warn(f"not stamping the changelog entries: {reason(exc)}")


class Stamper:
    def __init__(self, changelog: Path) -> None:
        self.changelog = changelog
        self.directory = entry_directory(changelog)
        self.shown = self.directory.as_posix()
        self.pulls: dict[str, list[int]] = {}
        self.shared: dict[str, SharedSection] = {}

    def run(self) -> None:
        names = sorted(p.name for p in self.directory.iterdir() if p.name != ENTRY_README) if self.directory.is_dir() else []
        if not names:
            return
        try:
            self.repository_directory = repository_path(self.directory)
            self.repository_changelog = repository_path(self.changelog)
            landed = landings(self.repository_directory)
        except (HistoryUnavailable, GitError) as exc:
            warn(f"not stamping any entry under {self.shown}/: {exc}; nothing is attributed that was not seen")
            return
        for name in names:
            self.stamp(name, landed)

    def stamp(self, name: str, landed: dict[str, Landing]) -> None:
        shown = f"{self.shown}/{name}"
        path = self.directory / name
        try:
            text = path.read_bytes().decode("utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            warn(f"not stamping {shown}: cannot read it: {reason(exc)}")
            return
        problem = entry_problem(name, text)
        if problem is not None:
            warn(f"not stamping {self.shown}/{problem}")
            return
        lines = entry_lines(text)
        if trailing_number(lines) not in (None, "TBD"):
            return  # a number written at the end is left alone, and is not verified here
        landing = landed.get(entry_name(name).slug)
        if landing is None:
            warn(f"not stamping {shown}: no commit on HEAD's first-parent history added it, so nothing attributes it")
            return
        if self.is_switchover_copy(landing):
            if not pr_numbers(text):
                warn(f"releasing {shown} without a number: the switch-over moved it, and it carried none")
            return
        number = self.single_pull_request(shown, landing.commit)
        if number is None:
            return
        try:
            path.write_text("\n".join(stamped(lines, number)) + "\n", encoding="utf-8")
        except OSError as exc:
            warn(f"not stamping {shown} with PR #{number}: cannot write it: {reason(exc)}")

    def is_switchover_copy(self, landing: Landing) -> bool:
        """Whether the entry, as it was added, copies a bullet the addition's parent held. §FS-002-release.2.2"""
        parent = first_parent(landing.commit)
        if parent is None:
            return False
        if parent not in self.shared:
            self.shared[parent] = SharedSection(file_at(parent, self.repository_changelog))
        shared = self.shared[parent]
        added = file_at(landing.commit, f"{self.repository_directory}/{landing.name}") if shared else None
        parsed = entry_name(landing.name)
        return added is not None and parsed is not None and shared.match(parsed.category, added) is not None

    def single_pull_request(self, shown: str, sha: str) -> int | None:
        try:
            if sha not in self.pulls:
                self.pulls[sha] = pull_requests_for_commit(sha)
        except Exception as exc:  # a forge, a token or a rate limit this release will not fail over
            warn(f"not stamping {shown}: cannot ask which pull request {sha[:12]} belongs to: {reason(exc)}")
            return None
        pulls = self.pulls[sha]
        if len(pulls) == 1:
            return pulls[0]
        belongs = "no pull request" if not pulls else "more than one: " + ", ".join(f"#{n}" for n in pulls)
        warn(f"not stamping {shown}: {sha[:12]}, the commit that added it, belongs to {belongs}")
        return None


def warn(message: str) -> None:
    print(f"warning: {message}", file=sys.stderr)


def reason(exc: BaseException) -> str:
    """What went wrong, in the failing tool's own words where it left any. §FS-002-release.2.2

    The warnings go to the workflow log and nowhere else, so that log is the
    whole of the release's report on a stamping failure. `CalledProcessError`
    omits `stderr` from its `str()`, which is what makes a 403, a rate limit and
    an unreachable forge read identically there.
    """
    captured = getattr(exc, "stderr", None)
    if isinstance(captured, bytes):
        captured = captured.decode("utf-8", errors="replace")
    if isinstance(captured, str) and captured.strip():
        return f"{exc}: {captured.strip()}"
    return str(exc)
