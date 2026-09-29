"""The release script: promoting `## Unreleased`, and stamping the numbers
contributors could not know. §FS-002-release.2
"""

import io
import os
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest.mock import patch
from pathlib import Path

from changelog_git import GIT_ENV, ChangelogRepo, load_script, working_directory

prepare_changelog_release = load_script("prepare_changelog_release.py")


SAMPLE_CHANGELOG = """# Changelog

Intro.

## Unreleased

### Fixed

- [§FS-distribution.4](functional-spec/FS-distribution.md#4-release-process): rotate release notes automatically.

## 2. [0.2.0] — 2026-05-17

Workspace and agent-entrypoint release. The main user-visible change is workspace aliases.

### Added

- [§FS-workspace](functional-spec/FS-workspace.md#fs-workspace-grund-validates-cross-project-citations-in-a-workspace): validate aliases.

## 3. Older releases

- [0.1.0](changelog/0.1.0.md) — 2026-05-14: first published release and baseline CLI surface.
"""


class PrepareChangelogReleaseTests(unittest.TestCase):
    def write_changelog(self, text: str = SAMPLE_CHANGELOG) -> Path:
        root = Path(self.tempdir.name)
        changelog = root / "docs" / "changelog.md"
        changelog.parent.mkdir(parents=True)
        changelog.write_text(text, encoding="utf-8")
        return changelog

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def test_prepare_promotes_unreleased_and_archives_previous_latest(self) -> None:
        changelog = self.write_changelog()

        prepare_changelog_release.prepare_release(changelog, "0.2.1", "2026-05-18")

        updated = changelog.read_text(encoding="utf-8")
        self.assertIn("## Unreleased\n\n## 2. [0.2.1] — 2026-05-18", updated)
        self.assertIn("rotate release notes automatically.", updated)
        self.assertIn(
            "- [0.2.0](changelog/0.2.0.md) — 2026-05-17: Workspace and agent-entrypoint release.",
            updated,
        )
        self.assertIn(
            "- [0.1.0](changelog/0.1.0.md) — 2026-05-14: first published release and baseline CLI surface.",
            updated,
        )

        archived = changelog.parent / "changelog" / "0.2.0.md"
        self.assertEqual(
            archived.read_text(encoding="utf-8"),
            """# 0.2.0 — 2026-05-17

Workspace and agent-entrypoint release. The main user-visible change is workspace aliases.

### Added

- [§FS-workspace](../functional-spec/FS-workspace.md#fs-workspace-grund-validates-cross-project-citations-in-a-workspace): validate aliases.

""",
        )

    def test_prepare_fails_when_unreleased_has_no_bullets(self) -> None:
        changelog = self.write_changelog(
            """# Changelog

## Unreleased

## 2. [0.2.0] — 2026-05-17

Previous release.

## 3. Older releases
"""
        )

        with self.assertRaisesRegex(prepare_changelog_release.ChangelogError, "no bullet entries"):
            prepare_changelog_release.prepare_release(changelog, "0.2.1", "2026-05-18")

    def test_extract_notes_writes_inline_release_body(self) -> None:
        changelog = self.write_changelog()
        output = changelog.parent / "release-notes.md"

        prepare_changelog_release.extract_notes(changelog, "0.2.0", output)

        notes = output.read_text(encoding="utf-8")
        self.assertIn("Workspace and agent-entrypoint release.", notes)
        self.assertIn("### Added", notes)
        self.assertNotIn("Older releases", notes)


class Outcome:
    def __init__(self, code: int, out: str, err: str) -> None:
        self.code = code
        self.out = out
        self.err = err

    @property
    def text(self) -> str:
        return self.out + self.err

    def __str__(self) -> str:
        return f"exit={self.code}\n--- stdout ---\n{self.out}--- stderr ---\n{self.err}"


def run_stamp(repo: ChangelogRepo) -> Outcome:
    """Run `stamp` the way the release workflow does: in the checkout, by argv."""
    out, err = io.StringIO(), io.StringIO()
    with working_directory(repo.path), patch.dict(os.environ, GIT_ENV):
        with redirect_stdout(out), redirect_stderr(err):
            try:
                code = prepare_changelog_release.main(["--changelog", "docs/changelog.md", "stamp"])
            except SystemExit as exc:  # argparse refusing a subcommand the script does not have
                code = exc.code if isinstance(exc.code, int) else 1
    return Outcome(code, out.getvalue(), err.getvalue())


THE_RELEASED_BULLET = (
    "- **The beginning.** It shipped before the gate existed and carries no\n  number at all."
)


class StampUnreleasedNumbersTests(unittest.TestCase):
    """The release stamps what the contributor did not write, and never fails. §FS-002-release.2"""

    def repo(self, unreleased: str) -> ChangelogRepo:
        repo = ChangelogRepo(unreleased, with_origin=False, branch="main")
        self.addCleanup(repo.cleanup)
        return repo

    def resolver(self, pulls: dict[str, list[int]]):
        """The one seam that reaches the forge, stubbed. No network call is made."""
        return patch.object(
            prepare_changelog_release,
            "pull_requests_for_commit",
            side_effect=lambda sha: pulls.get(sha, []),
        )

    def test_stamp_writes_the_number_when_every_line_agrees(self) -> None:
        repo = self.repo(
            "### Fixed\n"
            "\n"
            "- **A bullet one commit wrote.** It says what a user will notice, over\n"
            "  two lines, and nobody wrote its number.\n"
        )
        with self.resolver({repo.base_sha: [142]}):
            outcome = run_stamp(repo)
        self.assertEqual(outcome.code, 0, outcome)
        text = repo.read()
        self.assertIn("(PR #142)", text, outcome)
        self.assertIn("two lines, and nobody wrote its number.", text, outcome)

    def test_stamp_leaves_a_bullet_whose_lines_disagree_and_warns(self) -> None:
        first = (
            "### Fixed\n"
            "\n"
            "- **A bullet two commits wrote.** Its first line came with the bullet\n"
            "  and its second line was added later.\n"
        )
        repo = self.repo(first)
        repo.write_unreleased(first.replace("was added later.", "was rewritten later."))
        later = repo.commit("changelog: reword the second line of the bullet")
        with self.resolver({repo.base_sha: [137], later: [138]}):
            outcome = run_stamp(repo)
        self.assertEqual(outcome.code, 0, outcome)
        text = repo.read()
        self.assertNotIn("PR #137", text, outcome)
        self.assertNotIn("PR #138", text, outcome)
        self.assertIn("A bullet two commits wrote", outcome.text, outcome)

    def test_stamp_replaces_pr_tbd_in_place(self) -> None:
        repo = self.repo("### Fixed\n\n- **A bullet with a placeholder.** (PR #TBD)\n")
        with self.resolver({repo.base_sha: [142]}):
            outcome = run_stamp(repo)
        self.assertEqual(outcome.code, 0, outcome)
        text = repo.read()
        self.assertIn("(PR #142)", text, outcome)
        self.assertNotIn("TBD", text, outcome)
        self.assertEqual(text.count("PR #142"), 1, outcome)

    def test_stamp_leaves_an_existing_number_alone(self) -> None:
        repo = self.repo("### Fixed\n\n- **A bullet whose author knew the number.** (PR #137)\n")
        with self.resolver({repo.base_sha: [142]}):
            outcome = run_stamp(repo)
        self.assertEqual(outcome.code, 0, outcome)
        text = repo.read()
        self.assertIn("(PR #137)", text, outcome)
        self.assertNotIn("142", text, outcome)

    def test_stamp_ignores_released_sections(self) -> None:
        repo = self.repo("### Fixed\n\n- **A bullet of our own.** (PR #137)\n")
        with self.resolver({repo.base_sha: [142]}):
            outcome = run_stamp(repo)
        self.assertEqual(outcome.code, 0, outcome)
        text = repo.read()
        self.assertIn(THE_RELEASED_BULLET, text, outcome)
        self.assertNotIn("142", text, outcome)

    def test_stamp_exits_zero_when_the_forge_call_fails(self) -> None:
        """Never failing a release has to be true of a broken token, not only of an ambiguous bullet."""
        causes = (
            ("gh is not on PATH", FileNotFoundError("gh")),
            ("the token is refused", prepare_changelog_release.ChangelogError("gh api: HTTP 401")),
            ("the api errors", subprocess.CalledProcessError(1, ["gh", "api"], stderr="rate limit")),
        )
        for description, failure in causes:
            with self.subTest(description):
                repo = self.repo("### Fixed\n\n- **A bullet with no number.**\n")
                before = repo.read()
                with patch.object(
                    prepare_changelog_release, "pull_requests_for_commit", side_effect=failure
                ):
                    outcome = run_stamp(repo)
                self.assertEqual(outcome.code, 0, outcome)
                self.assertEqual(repo.read(), before, outcome)
                self.assertIn("warning", outcome.text.lower(), outcome)

    def test_stamp_exits_zero_when_the_changelog_cannot_be_read(self) -> None:
        """Never failing a release has to be true of the changelog too, not only of the forge."""
        repo = self.repo("### Fixed\n\n- **A bullet with no number.**\n")
        repo.changelog.write_bytes(b"# Changelog\n\n## Unreleased\n\n- \xff\xfe not utf-8.\n")
        before = repo.changelog.read_bytes()

        outcome = run_stamp(repo)

        self.assertEqual(outcome.code, 0, outcome)
        self.assertEqual(repo.changelog.read_bytes(), before, outcome)
        self.assertIn("warning", outcome.text.lower(), outcome)

    def test_stamp_exits_zero_when_the_changelog_cannot_be_written(self) -> None:
        """The write is inside the never-fails boundary as much as the read is."""
        repo = self.repo("### Fixed\n\n- **A bullet with no number.**\n")
        before = repo.read()
        refuse = patch.object(
            prepare_changelog_release,
            "_write_lines",
            side_effect=PermissionError("read-only file system"),
        )
        with self.resolver({repo.base_sha: [142]}), refuse:
            outcome = run_stamp(repo)

        self.assertEqual(outcome.code, 0, outcome)
        self.assertEqual(repo.read(), before, outcome)
        self.assertIn("warning", outcome.text.lower(), outcome)

    def test_the_warning_carries_the_failing_tool_s_own_words(self) -> None:
        """The workflow log is the whole report, so a 403 may not read like a timeout."""
        repo = self.repo("### Fixed\n\n- **A bullet with no number.**\n")
        refused = subprocess.CalledProcessError(
            1, ["gh", "api"], stderr="gh: Resource not accessible by integration (HTTP 403)\n"
        )
        with patch.object(
            prepare_changelog_release, "pull_requests_for_commit", side_effect=refused
        ):
            outcome = run_stamp(repo)

        self.assertEqual(outcome.code, 0, outcome)
        self.assertIn("Resource not accessible by integration (HTTP 403)", outcome.text, outcome)


if __name__ == "__main__":
    unittest.main()
