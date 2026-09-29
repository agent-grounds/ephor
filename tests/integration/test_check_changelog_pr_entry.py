"""The pull-request changelog gate, at both moments it runs. §FS-002-release.6

The gate asks for a bullet, not for a number: a contributor writing their first
bullet cannot know the number, so the pre-push hook is what refuses a missing
bullet and CI never asks for a number that did not exist yet
(§FS-002-release.1).
"""

import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from subprocess import CompletedProcess
from unittest.mock import patch
from pathlib import Path

from changelog_git import GIT_ENV, ChangelogRepo, load_script, working_directory

gate = load_script("check_changelog_pr_entry.py")

# `GITHUB_EVENT_PATH` is set on every CI leg, and the gate reads it when no
# `--pr-number` is given. Blanking it is what keeps a `--local-pr` case from
# quietly picking up the number of the pull request running the suite.
GATE_ENV = {**GIT_ENV, "GITHUB_EVENT_PATH": ""}

# A section that already carries the numbers of merged pull requests, which is
# what `## Unreleased` looks like in this repository. None of them is ours.
NUMBERED_SECTION = """### Added

- **A thing that already shipped to Unreleased.** (PR #137)
- **A second thing already here.** (PR #138)
- **A third thing, recorded by its url.**
  (https://github.com/agent-grounds/ephor/pull/139)
"""

SECTION_WITH_A_NUMBERLESS_BULLET = """### Added

- **A thing that already shipped to Unreleased.** (PR #137)
- **A thing whose number nobody wrote.**
"""

THE_SIX_SECTIONS = ("Added", "Changed", "Deprecated", "Removed", "Fixed", "Security")


class Outcome:
    def __init__(self, code: int, out: str, err: str) -> None:
        self.code = code
        self.out = out
        self.err = err

    @property
    def text(self) -> str:
        return self.out + self.err

    @property
    def first_error_line(self) -> str:
        for line in self.text.splitlines():
            if line.startswith("error:"):
                return line
        return ""

    def __str__(self) -> str:
        return f"exit={self.code}\n--- stdout ---\n{self.out}--- stderr ---\n{self.err}"


def run_gate(repo: ChangelogRepo, argv: list[str], *, local_pr_number: int | None = None) -> Outcome:
    """Run the gate the way a hook or a CI step does: in the checkout, by argv."""
    out, err = io.StringIO(), io.StringIO()
    with working_directory(repo.path), patch.dict(os.environ, GATE_ENV), patch.object(
        gate, "pr_number_from_current_branch", return_value=local_pr_number
    ):
        with redirect_stdout(out), redirect_stderr(err):
            try:
                code = gate.main(argv)
            except SystemExit as exc:  # argparse refusing an argument the gate does not have
                code = exc.code if isinstance(exc.code, int) else 1
    return Outcome(code, out.getvalue(), err.getvalue())


class UnreleasedBulletGateTests(unittest.TestCase):
    """The base-relative gate: a bullet is what is asked for, at both moments."""

    def repo(self, unreleased: str, **kwargs) -> ChangelogRepo:
        repo = ChangelogRepo(unreleased, **kwargs)
        self.addCleanup(repo.cleanup)
        return repo

    def branch_that_adds(self, unreleased: str, *, base: str = NUMBERED_SECTION, **kwargs) -> ChangelogRepo:
        repo = self.repo(base, **kwargs)
        repo.write_unreleased(unreleased)
        repo.commit("changelog: the contribution")
        return repo

    def branch_that_adds_no_bullet(self, *, base: str = NUMBERED_SECTION, **kwargs) -> ChangelogRepo:
        repo = self.repo(base, **kwargs)
        repo.write_file("src/thing.rs", "// a change a user will notice, and no bullet for it\n")
        repo.commit("a change with no changelog bullet")
        return repo

    # --- the two arms of the reproducer, inverted ---------------------------

    def test_new_bullet_passes_without_any_number(self) -> None:
        repo = self.branch_that_adds(
            NUMBERED_SECTION + "- **My first contribution.** It does what the issue asked for.\n"
        )
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)

    def test_no_new_bullet_is_refused_even_though_the_section_is_full_of_numbers(self) -> None:
        repo = self.branch_that_adds_no_bullet()
        outcome = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("no new or changed bullet", outcome.text, outcome)

    # --- what counts as adding a bullet ------------------------------------

    def test_reworded_bullet_counts_as_changed(self) -> None:
        repo = self.branch_that_adds(
            SECTION_WITH_A_NUMBERLESS_BULLET.replace(
                "- **A thing whose number nobody wrote.**",
                "- **A thing whose number nobody wrote.** Now it says what it does.",
            ),
            base=SECTION_WITH_A_NUMBERLESS_BULLET,
        )
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)

    def test_added_heading_without_a_bullet_is_refused(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "\n### Security\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("no new or changed bullet", outcome.text, outcome)

    # --- the number clause reads only the lines this pull request adds ------

    def test_number_on_an_added_line_must_be_this_pull_requests_own(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, misnumbered.** (PR #137)\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("PR #137", outcome.text, outcome)
        self.assertIn("142", outcome.text, outcome)

    def test_a_pull_request_url_naming_another_number_is_refused(self) -> None:
        repo = self.branch_that_adds(
            NUMBERED_SECTION + "- **Mine, by url.** (https://github.com/agent-grounds/ephor/pull/137)\n"
        )
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("137", outcome.text, outcome)

    def test_numbers_on_untouched_bullets_are_ignored(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, with no number at all.**\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)
        for untouched in ("137", "138", "139"):
            self.assertNotIn(untouched, outcome.text, outcome)

    def test_pr_tbd_placeholder_is_accepted(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, with a placeholder.** (PR #TBD)\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)

    # --- where CI gets its base --------------------------------------------

    def test_the_base_rev_flag_is_the_base_it_compares_against(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, with no number at all.**\n")
        outcome = run_gate(
            repo, ["--pr-number", "142", "--base-rev", repo.base_sha, "--changelog", "docs/changelog.md"]
        )
        self.assertEqual(outcome.code, 0, outcome)

    def test_a_moved_main_does_not_hand_a_branch_a_bullet_it_did_not_add(self) -> None:
        repo = self.branch_that_adds_no_bullet()
        repo.advance_main(
            NUMBERED_SECTION.replace("A second thing already here.", "A second thing, reworded on main."),
            "changelog: reword a bullet on main",
        )
        outcome = run_gate(
            repo, ["--pr-number", "142", "--base-rev", repo.base_sha, "--changelog", "docs/changelog.md"]
        )
        self.assertEqual(outcome.code, 1, outcome)

    # --- where the local hook cannot see a base ----------------------------

    def test_unresolvable_base_falls_back_to_any_bullet_and_says_which_check_it_ran(self) -> None:
        repo = self.branch_that_adds_no_bullet(with_origin=False)
        outcome = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)
        self.assertIn("could not resolve a base", outcome.text, outcome)
        self.assertIn("origin/main", outcome.text, outcome)
        self.assertIn("has a bullet at all", outcome.text, outcome)
        self.assertIn("not that you added one", outcome.text, outcome)

    def test_unresolvable_base_with_no_bullets_at_all_is_refused(self) -> None:
        repo = self.repo("", with_origin=False)
        repo.write_file("src/thing.rs", "// a change with an empty Unreleased above it\n")
        repo.commit("a change with no changelog bullet anywhere")
        outcome = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("no bullets at all", outcome.text, outcome)
        self.assertIn("SKIP=changelog-pr-entry git push", outcome.text, outcome)

    # --- the messages are the contributor-facing half of this change --------

    def test_refusal_names_the_skip_bypass(self) -> None:
        repo = self.branch_that_adds_no_bullet()
        outcome = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("SKIP=changelog-pr-entry git push", outcome.text, outcome)

    def test_refusal_carries_the_guidance_a_contributor_needs(self) -> None:
        repo = self.branch_that_adds_no_bullet()
        outcome = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        for section in THE_SIX_SECTIONS:
            self.assertIn(section, outcome.text, f"the refusal does not name {section}:\n{outcome}")
        self.assertIn("You do not need the pull request number", outcome.text, outcome)
        self.assertIn("PR #TBD", outcome.text, outcome)
        self.assertIn("conventions are in docs/changelog.md", outcome.text, outcome)

    def test_wrong_number_refusal_says_untouched_numbers_are_not_theirs_to_fix(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, misnumbered.** (PR #137)\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 1, outcome)
        self.assertIn("not yours to fix", outcome.text, outcome)
        self.assertIn("PR #TBD", outcome.text, outcome)

    # --- one predicate, two moments ----------------------------------------

    def test_the_push_and_the_pull_request_reach_one_predicate(self) -> None:
        """`.pre-commit-config.yaml` promises the hook matches the CI gate rather than exceeding it."""
        cases = (
            ("a branch that adds no bullet", None),
            ("a branch that adds a numberless bullet", NUMBERED_SECTION + "- **Mine.**\n"),
        )
        for description, unreleased in cases:
            with self.subTest(description):
                if unreleased is None:
                    repo = self.branch_that_adds_no_bullet()
                else:
                    repo = self.branch_that_adds(unreleased)
                local = run_gate(repo, ["--local-pr", "--changelog", "docs/changelog.md"])
                ci = run_gate(
                    repo, ["--pr-number", "142", "--base-rev", repo.base_sha, "--changelog", "docs/changelog.md"]
                )
                self.assertEqual(local.code, ci.code, f"local:\n{local}\nci:\n{ci}")
                if ci.code != 0:
                    self.assertEqual(local.first_error_line, ci.first_error_line, f"local:\n{local}\nci:\n{ci}")

    # --- behaviour this change does not move -------------------------------

    def test_an_added_bullet_may_carry_its_own_number(self) -> None:
        repo = self.branch_that_adds(NUMBERED_SECTION + "- **Mine, numbered by hand.** (PR #142)\n")
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)

    def test_a_pull_request_url_counts_as_this_pull_requests_number(self) -> None:
        repo = self.branch_that_adds(
            NUMBERED_SECTION + "- **Mine, by url.** (https://github.com/agent-grounds/ephor/pull/142)\n"
        )
        outcome = run_gate(repo, ["--pr-number", "142", "--changelog", "docs/changelog.md"])
        self.assertEqual(outcome.code, 0, outcome)


class PullRequestNumberLookupTests(unittest.TestCase):
    """Where the number comes from when there is one. Unmoved by this change."""

    def test_reads_pull_request_number_from_event_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            event_path = Path(tmp).resolve() / "event.json"
            event_path.write_text(json.dumps({"pull_request": {"number": 15}}), encoding="utf-8")
            self.assertEqual(gate.pr_number_from_event(event_path), 15)

    def test_non_pull_request_event_returns_no_number(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            event_path = Path(tmp).resolve() / "event.json"
            event_path.write_text(json.dumps({"ref": "refs/heads/main"}), encoding="utf-8")
            self.assertIsNone(gate.pr_number_from_event(event_path))

    def test_reads_pull_request_number_from_gh_current_branch(self) -> None:
        with patch.object(
            gate.subprocess,
            "run",
            return_value=CompletedProcess(args=[], returncode=0, stdout="18\n", stderr=""),
        ):
            self.assertEqual(gate.pr_number_from_current_branch(), 18)

    def test_missing_gh_current_branch_pr_returns_no_number(self) -> None:
        with patch.object(
            gate.subprocess,
            "run",
            return_value=CompletedProcess(args=[], returncode=1, stdout="", stderr="no pull requests found"),
        ):
            self.assertIsNone(gate.pr_number_from_current_branch())


if __name__ == "__main__":
    unittest.main()
