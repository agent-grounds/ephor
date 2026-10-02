"""The pull-request changelog gate, at both moments it runs. §FS-002-release.6

The gate asks for an entry file the base did not have, not for a number: a
contributor writing their first entry cannot know the number, so the pre-push
hook is what refuses a missing entry and CI never asks for a number that did not
exist yet (§FS-002-release.1). What an entry is — its name, its category, one
bullet — is §FS-002-release.1.1, and the gate holds every entry a change touches
to it.
"""

import json
import re
import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

from changelog_git import ENTRY_DIR, EntryRepoCase, describe, load_script, output

gate = load_script("check_changelog_pr_entry.py")

BYPASS = "SKIP=changelog-pr-entry git push"

# A refusal names a file to add, not only the directory: a concrete
# `<slug>.<category>.md` under it, in one of the categories it may take.
SUGGESTED_FILE = re.compile(
    r"docs/changelog/unreleased/[a-z0-9][a-z0-9-]*\.(added|changed|deprecated|removed|fixed|security|note)\.md"
)


class EntryGateTests(EntryRepoCase):
    """The base-relative gate: an added entry is what is asked for, at both moments."""

    def ci(self, repo, *extra: str) -> CompletedProcess:
        return repo.gate("--base-rev", repo.base, "--pr-number", "142", *extra)

    def branch_with_no_entry(self, **kwargs):
        repo = self.repo(**kwargs)
        repo.write("src/thing.rs", "// a change a user will notice, and no entry for it\n")
        repo.commit("a change with no changelog entry")
        return repo

    # --- what counts as adding an entry -------------------------------------

    def test_an_added_entry_passes_at_both_moments_with_or_without_its_number(self) -> None:
        for ending in ("", " (PR #TBD)", " (PR #142)", " ([#142](https://github.com/agent-grounds/ephor/pull/142))"):
            with self.subTest(ending=ending):
                repo = self.repo()
                repo.entry("refresh-unreachable.fixed.md", f"- **`ephor refresh` keeps unreachable projects.**{ending}\n")
                repo.commit("fix: keep unreachable projects")
                self.assert_exit(repo.gate("--local-pr"), 0)
                self.assert_exit(self.ci(repo), 0)

    def test_an_entry_of_several_paragraphs_is_one_entry(self) -> None:
        repo = self.repo()
        repo.entry(
            "paragraphs.changed.md",
            "- **A change that needs two paragraphs.** The first one wraps\n"
            "  onto a second line.\n"
            "\n"
            "  **And a second paragraph.** It belongs to the same bullet.\n",
        )
        repo.commit("feat: a long entry")
        self.assert_exit(self.ci(repo), 0)

    def test_a_branch_that_adds_no_entry_is_refused_naming_the_directory_and_a_file(self) -> None:
        repo = self.branch_with_no_entry()
        local = repo.gate("--local-pr")
        self.assert_refused(local, ENTRY_DIR, "PR #TBD", BYPASS)
        self.assertRegex(output(local), SUGGESTED_FILE)
        ci = self.ci(repo)
        self.assert_refused(ci, ENTRY_DIR)
        self.assertRegex(output(ci), SUGGESTED_FILE)

    def test_the_readme_alone_is_not_an_entry(self) -> None:
        repo = self.repo()
        repo.write(f"{ENTRY_DIR}/README.md", repo.read_entry("README.md") + "- A line that looks like a bullet.\n")
        repo.commit("docs: explain the entries")
        self.assert_refused(self.ci(repo), ENTRY_DIR)

    def test_a_bullet_under_the_shared_section_is_not_an_entry(self) -> None:
        repo = self.repo()
        repo.write(
            "docs/changelog.md",
            repo.changelog.read_text().replace("## Unreleased\n", "## Unreleased\n\n### Fixed\n\n- **Written the old way.**\n"),
        )
        repo.commit("fix: an entry written the old way")
        self.assert_refused(self.ci(repo), ENTRY_DIR)
        self.assert_refused(repo.gate("--local-pr"), ENTRY_DIR)

    def test_an_edit_or_a_change_of_category_alone_does_not_count(self) -> None:
        def edit(path: Path) -> None:
            path.write_text("- **Already here, reworded.**\n")

        def recategorize(path: Path) -> None:
            path.rename(path.with_name("existing.changed.md"))

        def both(path: Path) -> None:
            # Rewritten as well as moved: git sees no rename, the slug still does.
            path.unlink()
            path.with_name("existing.changed.md").write_text("- **Something else entirely, at length.**\n")

        for name, change in (("edit", edit), ("category", recategorize), ("category and edit", both)):
            with self.subTest(change=name):
                repo = self.repo()
                existing = repo.entry("existing.fixed.md", "- **Already here.**\n")
                base = repo.commit("fix: an entry that already landed")
                change(existing)
                repo.commit("docs: amend that entry")
                self.assert_refused(repo.gate("--base-rev", base, "--pr-number", "142"), ENTRY_DIR)

                # The amendment is not what was refused, the missing entry was.
                repo.entry("mine.fixed.md", "- **My own change.**\n")
                repo.commit("fix: my own change")
                self.assert_exit(repo.gate("--base-rev", base, "--pr-number", "142"), 0)

    def test_a_change_of_slug_is_refused_even_beside_a_new_entry(self) -> None:
        repo = self.repo()
        kept = repo.entry("kept-for-life.fixed.md", "- **Already here.**\n")
        base = repo.commit("fix: an entry that already landed")
        kept.rename(kept.with_name("renamed-later.fixed.md"))
        repo.entry("mine.fixed.md", "- **My own change.**\n")
        repo.commit("fix: mine, and a rename")
        refused = repo.gate("--base-rev", base, "--pr-number", "142")
        self.assert_refused(refused, "kept-for-life", "renamed-later")

    def test_a_slug_used_twice_is_refused(self) -> None:
        with self.subTest("both added by this change"):
            repo = self.repo()
            repo.entry("same.fixed.md")
            repo.entry("same.added.md")
            repo.commit("fix: one slug, two categories")
            self.assert_refused(self.ci(repo), "same.fixed.md", "same.added.md")
        with self.subTest("one already at the base"):
            repo = self.repo()
            repo.entry("same.fixed.md")
            base = repo.commit("fix: an entry that already landed")
            repo.entry("same.added.md")
            repo.commit("feat: the same slug again")
            self.assert_refused(repo.gate("--base-rev", base, "--pr-number", "142"), "same.added.md")

    def test_a_malformed_entry_is_refused_even_beside_a_good_one(self) -> None:
        cases = (
            ("Upper-case.fixed.md", "- **The slug is not lowercase.**\n"),
            ("under_score.fixed.md", "- **The slug has an underscore.**\n"),
            ("no-category.md", "- **This repository requires a category.**\n"),
            ("unknown.other.md", "- **`other` is not a category.**\n"),
            ("plain-text.fixed.txt", "- **Not Markdown.**\n"),
            ("empty.fixed.md", ""),
            ("heading.fixed.md", "### Fixed\n\n- **A heading is not a bullet.**\n"),
            ("no-bullet.fixed.md", "A paragraph, not a bullet.\n"),
            ("two-bullets.fixed.md", "- **One.**\n- **Two.**\n"),
            ("one-space.fixed.md", "- **Wrapped.**\n with a one-space continuation.\n"),
            ("loose-paragraph.fixed.md", "- **A bullet.**\n\nThen a paragraph that is not indented.\n"),
        )
        for name, body in cases:
            with self.subTest(name=name):
                repo = self.repo()
                repo.entry("mine.fixed.md", "- **My own change.**\n")
                repo.entry(name, body)
                repo.commit("fix: mine, and a malformed entry")
                self.assert_refused(self.ci(repo), name)

    def test_an_entry_this_change_edits_is_held_to_the_format_too(self) -> None:
        repo = self.repo()
        existing = repo.entry("existing.fixed.md", "- **Already here.**\n")
        base = repo.commit("fix: an entry that already landed")
        existing.write_text("- **Already here.**\n- **And a second bullet in the same file.**\n")
        repo.entry("mine.fixed.md", "- **My own change.**\n")
        repo.commit("fix: mine, and a broken edit")
        self.assert_refused(repo.gate("--base-rev", base, "--pr-number", "142"), "existing.fixed.md")

    # --- the number clause reads only what this change writes ----------------

    def test_a_number_this_change_writes_must_be_its_own(self) -> None:
        for written in (
            "(PR #137)",
            "(pull request #137)",
            "([original](https://github.com/agent-grounds/ephor/pull/137))",
        ):
            with self.subTest(written=written):
                repo = self.repo()
                repo.entry("misnumbered.fixed.md", f"- **Mine, misnumbered.** {written}\n")
                repo.commit("fix: misnumbered")
                self.assert_refused(self.ci(repo), "misnumbered.fixed.md", "137", "142")

    def test_a_number_an_entry_already_carried_is_not_this_change_s_to_fix(self) -> None:
        repo = self.repo()
        carried = repo.entry("carried.fixed.md", "- **Landed with its number.** (PR #137)\n")
        repo.entry("untouched.added.md", "- **Landed with its number too.** (PR #138)\n")
        base = repo.commit("feat: entries that already landed")
        carried.unlink()
        repo.entry("carried.changed.md", "- **Landed with its number, clarified.** (PR #137)\n")
        repo.entry("mine.fixed.md", "- **My own change.**\n")
        repo.commit("fix: mine, and a clarification")
        result = repo.gate("--base-rev", base, "--pr-number", "142")
        self.assert_exit(result, 0)
        self.assertNotIn("138", output(result), describe(result))

    def test_a_foreign_number_written_into_an_existing_entry_is_refused(self) -> None:
        repo = self.repo()
        existing = repo.entry("existing.fixed.md", "- **Already here.**\n")
        base = repo.commit("fix: an entry that already landed")
        existing.write_text("- **Already here.** (PR #137)\n")
        repo.entry("mine.fixed.md", "- **My own change.**\n")
        repo.commit("fix: mine, and a number on someone else's")
        self.assert_refused(repo.gate("--base-rev", base, "--pr-number", "142"), "existing.fixed.md", "137")

    # --- where the base comes from --------------------------------------------

    def test_the_base_rev_flag_is_the_base_it_compares_against(self) -> None:
        repo = self.repo()
        repo.entry("mine.fixed.md")
        head = repo.commit("fix: mine")
        self.assert_exit(repo.gate("--base-rev", repo.base, "--pr-number", "142"), 0)
        self.assert_refused(repo.gate("--base-rev", head, "--pr-number", "142"), ENTRY_DIR)

    def test_a_moved_main_does_not_hand_a_branch_an_entry_it_did_not_add(self) -> None:
        repo = self.branch_with_no_entry()
        repo.git("checkout", "-q", "main")
        repo.entry("theirs.fixed.md", "- **Someone else's change.**\n")
        repo.commit("fix: theirs, merged while this branch was open")
        repo.git("checkout", "-q", "contribution")
        self.assert_refused(repo.gate("--local-pr"), ENTRY_DIR)
        self.assert_refused(self.ci(repo), ENTRY_DIR)

    def test_local_bases_are_tried_origin_then_upstream_then_main(self) -> None:
        for preferred in ("origin/main", "upstream/main", "main"):
            with self.subTest(preferred=preferred):
                repo = self.repo()
                if preferred != "main":
                    repo.git("update-ref", f"refs/remotes/{preferred}", repo.base)
                repo.entry("mine.fixed.md")
                head = repo.commit("fix: mine")
                if preferred != "main":
                    # Local main already holds the entry: reading it instead of
                    # the preferred base would find nothing added.
                    repo.git("update-ref", "refs/heads/main", head)
                self.assert_exit(repo.gate("--local-pr"), 0)

    # --- where the local hook cannot see a base -------------------------------

    def test_with_no_base_a_well_formed_entry_passes_and_says_what_was_not_checked(self) -> None:
        repo = self.repo(with_base=False)
        repo.entry("mine.note.md", "- **A note.**\n")
        repo.commit("docs: a note")
        result = repo.gate("--local-pr")
        self.assert_exit(result, 0)
        for detail in ("could not resolve a base", "origin/main", "upstream/main", ENTRY_DIR, "not that you added one"):
            self.assertIn(detail, output(result), describe(result))

    def test_with_no_base_a_missing_or_malformed_entry_is_refused(self) -> None:
        for body in (None, "A paragraph, not a bullet.\n"):
            with self.subTest(body=body):
                repo = self.repo(with_base=False)
                if body is not None:
                    repo.entry("broken.fixed.md", body)
                repo.write("src/thing.rs", "// a change\n")
                repo.commit("a change")
                self.assert_refused(repo.gate("--local-pr"), ENTRY_DIR, BYPASS)

    # --- one predicate, two moments ---------------------------------------------

    def test_the_push_and_the_pull_request_reach_one_predicate(self) -> None:
        """`.pre-commit-config.yaml` promises the hook matches the CI gate rather than exceeding it."""
        cases = (
            ("adds no entry", None, 1),
            ("adds a numberless entry", "- **Mine.**\n", 0),
            ("adds an entry naming another pull request", "- **Mine.** (PR #137)\n", 1),
        )
        for description, body, expected in cases:
            with self.subTest(description):
                repo = self.repo()
                if body is None:
                    repo.write("src/thing.rs", "// a change\n")
                else:
                    repo.entry("mine.fixed.md", body)
                repo.commit("the contribution")
                repo.forge(branch_pr=142)
                event = repo.root / "event.json"
                event.write_text(json.dumps({"pull_request": {"number": 142}}))
                local = repo.gate("--local-pr")
                ci = repo.gate("--base-rev", repo.base, "--event-path", str(event))
                self.assert_exit(local, expected)
                self.assert_exit(ci, expected)
                if expected:
                    self.assertEqual(first_error_line(local), first_error_line(ci), f"{describe(local)}\n{describe(ci)}")

    def test_the_changelog_flag_moves_the_entry_directory_with_it(self) -> None:
        repo = self.repo()
        repo.write("elsewhere/changelog.md", repo.changelog.read_text())
        repo.write("elsewhere/changelog/unreleased/mine.fixed.md", "- **Mine.**\n")
        repo.commit("fix: an entry beside another changelog")
        self.assert_exit(repo.gate("--changelog", "elsewhere/changelog.md", "--base-rev", repo.base), 0)
        self.assert_refused(repo.gate("--base-rev", repo.base), ENTRY_DIR)


def first_error_line(result: CompletedProcess) -> str:
    return next((line for line in output(result).splitlines() if line.startswith("error:")), "")


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
