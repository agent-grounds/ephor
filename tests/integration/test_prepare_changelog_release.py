"""The release script: collecting, ordering, stamping and consuming the entries. §FS-002-release.2

`stamp`, `prepare`, `notes` and `pending` are run the way the release
workflows run them, against a real history and a forge that answers what each
case says. Where an entry landed decides its order and its number
(§FS-002-release.2.1); stamping writes only the trailing number and never fails
a release (§FS-002-release.2.2); preparing consumes the entries and refuses
before it would lose one (§FS-002-release.2.3). What `notes` writes is still the
inline section (§FS-002-release.3). `pending` is what the scheduled release
holds on while no section is written, and the workflows are read for that hold
(§FS-002-release.2).
"""

import os
import re
import unittest

from changelog_git import (
    CATEGORIES,
    ENTRY_DIR,
    POINTER,
    REPOSITORY_ROOT,
    EntryRepoCase,
    describe,
    list_items,
    output,
    release_section,
    unreleased_section,
)


# What the refusal of an empty directory says to do (§FS-002-release.2.3), matched without case.
WRITE_IT_FIRST = "write the release section first"


class CollectionTests(EntryRepoCase):
    """`prepare` builds today's section shape from the entry files, and consumes them."""

    def test_every_category_is_released_in_order_and_the_entries_are_consumed(self) -> None:
        repo = self.repo()
        for category in reversed(CATEGORIES):
            repo.entry(
                f"example-{category}.{category}.md",
                f"- **The {category} change.**\n  Its continuation line stays with it. (PR #142)\n",
            )
        repo.commit("docs: one entry in every category")
        readme = (repo.path / ENTRY_DIR / "README.md").read_bytes()

        text = self.prepare(repo)

        section = release_section(text)
        headings = re.findall(r"^### (.+)$", section, re.MULTILINE)
        self.assertEqual(headings, [category.capitalize() for category in CATEGORIES], section)
        for category in CATEGORIES:
            self.assertIn(f"- **The {category} change.**\n  Its continuation line stays with it. (PR #142)\n", section)
        self.assertEqual(repo.pending(), [])
        self.assertEqual((repo.path / ENTRY_DIR / "README.md").read_bytes(), readme)
        self.assertEqual(unreleased_section(text).strip(), POINTER)
        self.assertIn("## 2. [0.1.1] — 2026-10-02\n", text)
        older = text.split("## 3. Older releases\n", 1)[1]
        self.assertIn("- [0.1.0](changelog/0.1.0.md) — 2026-01-01: The first release.", older)
        self.assertIn("- [0.0.1](changelog/0.0.1.md) — 2025-12-01: the first release.", older)
        archive = (repo.path / "docs" / "changelog" / "0.1.0.md").read_text()
        self.assertTrue(archive.startswith("# 0.1.0 — 2026-01-01\n"), archive)
        self.assertIn("- **The beginning.**", archive)

    def test_a_category_with_no_entry_is_omitted(self) -> None:
        repo = self.repo()
        repo.entry("only-a-note.note.md", "- **Only a note.**\n")
        repo.commit("docs: only a note")
        section = release_section(self.prepare(repo))
        self.assertEqual(re.findall(r"^### (.+)$", section, re.MULTILINE), ["Note"], section)

    def test_an_entry_of_several_paragraphs_is_released_whole(self) -> None:
        body = (
            "- **A change that needs two paragraphs.** The first one wraps\n"
            "  onto a second line.\n"
            "\n"
            "  **And a second paragraph.** It belongs to the same bullet. (PR #142)\n"
        )
        repo = self.repo()
        # One commit, so the file name orders them: the long entry first.
        repo.entry("a-paragraphs.changed.md", body)
        repo.entry("b-after.changed.md", "- **The entry after it.** (PR #143)\n")
        repo.commit("feat: a long entry and a short one")
        section = release_section(self.prepare(repo))
        self.assertIn(body, section)
        self.assertLess(section.index("And a second paragraph"), section.index("The entry after it"))


class OrderTests(EntryRepoCase):
    """Within a category, the oldest-landed entry comes first. §FS-002-release.2.1"""

    def order_in(self, section: str, *labels: str) -> list[str]:
        return sorted(labels, key=section.index)

    def test_entries_are_released_in_first_parent_landing_order_not_file_name_order(self) -> None:
        repo = self.repo()
        # Written first, on a side branch, and merged last.
        repo.git("checkout", "-q", "-b", "late-landing", repo.base)
        repo.entry("a-landed-second.fixed.md", "- **Landed second.**\n")
        repo.commit("fix: written first, landed second")
        repo.git("checkout", "-q", "main")
        repo.entry("z-landed-first.fixed.md", "- **Landed first.**\n")
        repo.commit("fix: written second, landed first")
        repo.git("merge", "-q", "--no-ff", "--no-edit", "late-landing")
        section = release_section(self.prepare(repo))
        self.assertEqual(self.order_in(section, "Landed second", "Landed first"), ["Landed first", "Landed second"])

    def test_entries_that_landed_together_go_by_file_name_and_uncommitted_ones_go_last(self) -> None:
        repo = self.repo()
        repo.entry("z-together.fixed.md", "- **Z landed.**\n")
        repo.entry("b-together.fixed.md", "- **B landed beside it.**\n")
        repo.commit("fix: two entries in one commit")
        repo.entry("a-uncommitted.fixed.md", "- **A is not committed.**\n")
        repo.entry("c-uncommitted.fixed.md", "- **C is not committed.**\n")
        section = release_section(self.prepare(repo))
        labels = ("B landed", "Z landed", "A is not", "C is not")
        self.assertEqual(self.order_in(section, *labels), list(labels), section)

    def test_an_edit_or_a_change_of_category_keeps_where_the_entry_landed(self) -> None:
        repo = self.repo()
        z = repo.entry("z-first.fixed.md", "- **Z landed first.**\n")
        repo.commit("fix: z")
        repo.entry("a-second.changed.md", "- **A landed second.**\n")
        repo.commit("feat: a")
        z.unlink()
        repo.entry("z-first.changed.md", "- **Z landed first, and was reworded and moved since.**\n")
        repo.commit("docs: amend z")
        section = release_section(self.prepare(repo))
        self.assertEqual(self.order_in(section, "A landed second", "Z landed first"), ["Z landed first", "A landed second"])

    def test_without_history_the_release_warns_orders_by_file_name_and_attributes_nothing(self) -> None:
        repo = self.repo()
        repo.entry("z-older.fixed.md", "- **Z landed first.**\n")
        repo.commit("fix: z")
        repo.entry("a-newer.fixed.md", "- **A landed second.**\n")
        head = repo.commit("fix: a")
        # What a depth-1 clone records: history ends at HEAD, which then looks
        # as though it added every file in the tree.
        (repo.path / ".git" / "shallow").write_text(head + "\n")
        repo.forge({head: [999]})

        stamped = repo.release("stamp")
        self.assert_exit(stamped, 0)
        self.assertIn("warning", stamped.stderr.lower(), describe(stamped))
        for name in repo.pending():
            self.assertNotIn("999", repo.read_entry(name))

        prepared = repo.release("prepare", "0.1.1", "--date", "2026-10-02")
        self.assert_exit(prepared, 0)
        self.assertIn("warning", prepared.stderr.lower(), describe(prepared))
        section = release_section(repo.changelog.read_text())
        self.assertEqual(self.order_in(section, "Z landed", "A landed"), ["A landed", "Z landed"])


class StampTests(EntryRepoCase):
    """The number comes from the commit that added the entry. §FS-002-release.2.2"""

    def test_stamp_writes_the_number_of_the_pull_request_that_added_the_entry(self) -> None:
        repo = self.repo()
        entry = repo.entry("mine.fixed.md", "- **Mine.**\n")
        addition = repo.commit("fix: mine")
        repo.forge({addition: [142]})
        changelog = repo.changelog.read_bytes()
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(entry.read_text(), "- **Mine.** (PR #142)\n")
        self.assertEqual(repo.changelog.read_bytes(), changelog)

    def test_an_edit_or_a_change_of_category_keeps_the_original_pull_request(self) -> None:
        repo = self.repo()
        original = repo.entry("same.fixed.md", "- **Original.**\n  With context.\n")
        addition = repo.commit("fix: original")
        original.unlink()
        moved = repo.entry("same.changed.md", "- **Original, reworded at length and moved.**\n  Other context.\n")
        edit = repo.commit("docs: reword and recategorize")
        repo.forge({addition: [142], edit: [143]})
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(moved.read_text(), "- **Original, reworded at length and moved.**\n  Other context. (PR #142)\n")

    def test_a_merge_landing_is_attributed_to_its_pull_request(self) -> None:
        repo = self.repo()
        entry = repo.entry("merged.fixed.md", "- **Merged, not rebased.**\n")
        side = repo.commit("fix: on the branch")
        repo.git("checkout", "-q", "main")
        repo.git("merge", "-q", "--no-ff", "--no-edit", "contribution")
        landing = repo.git("rev-parse", "HEAD")
        repo.forge({side: [142], landing: [142]})
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(entry.read_text(), "- **Merged, not rebased.** (PR #142)\n")

    def test_a_slug_used_again_after_a_release_starts_a_new_lifetime(self) -> None:
        repo = self.repo()
        repo.entry("same.fixed.md", "- **The first lifetime.** (PR #142)\n")
        first = repo.commit("fix: first lifetime")
        self.prepare(repo)
        repo.commit("release: 0.1.1")
        reused = repo.entry("same.note.md", "- **The second lifetime.**\n")
        second = repo.commit("docs: second lifetime")
        self.assert_exit(repo.gate("--base-rev", f"{second}^", "--pr-number", "143"), 0)
        repo.forge({first: [142], second: [143]})
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(reused.read_text(), "- **The second lifetime.** (PR #143)\n")

    def test_stamp_changes_only_the_trailing_number(self) -> None:
        prose = "- **Examples stay.** The example `(PR #TBD)` and PR #12 are prose."
        cases = (
            ("appended", f"{prose}\n", f"{prose} (PR #142)\n"),
            ("placeholder replaced", f"{prose} (PR #TBD)\n", f"{prose} (PR #142)\n"),
            ("written number kept", f"{prose} (PR #137)\n", f"{prose} (PR #137)\n"),
            (
                "last line of a wrapped entry",
                f"{prose}\n  It wraps. (PR #TBD)\n",
                f"{prose}\n  It wraps. (PR #142)\n",
            ),
            (
                "last paragraph",
                f"{prose}\n\n  **A second paragraph.**\n",
                f"{prose}\n\n  **A second paragraph.** (PR #142)\n",
            ),
        )
        for name, before, after in cases:
            with self.subTest(name):
                repo = self.repo()
                entry = repo.entry("examples.changed.md", before)
                addition = repo.commit("docs: numbers in prose")
                repo.forge({addition: [142]})
                self.assert_exit(repo.release("stamp"), 0)
                self.assertEqual(entry.read_text(), after)

    def test_stamp_warns_and_publishes_when_the_forge_gives_no_single_pull_request(self) -> None:
        for response in ([], [142, 143], "gh: HTTP 403: Resource not accessible by integration", "gh: API rate limit exceeded"):
            with self.subTest(response=response):
                repo = self.repo()
                entry = repo.entry("unresolved.fixed.md", "- **Unresolved.**\n")
                addition = repo.commit("fix: unresolved")
                repo.forge({addition: response})
                result = repo.release("stamp")
                self.assert_exit(result, 0)
                self.assertIn("warning", result.stderr.lower(), describe(result))
                self.assertIn("unresolved.fixed.md", result.stderr, describe(result))
                if isinstance(response, str):
                    self.assertIn(response.removeprefix("gh: "), result.stderr, describe(result))
                self.assertEqual(entry.read_text(), "- **Unresolved.**\n")
                self.assertIn("- **Unresolved.**\n", release_section(self.prepare(repo)))

    def test_stamp_never_fails_on_an_entry_it_cannot_read(self) -> None:
        repo = self.repo()
        broken = repo.path / ENTRY_DIR / "undecodable.fixed.md"
        broken.write_bytes(b"- **\xff\xfe not UTF-8.**\n")
        good = repo.entry("good.fixed.md", "- **Good.**\n")
        addition = repo.commit("fix: two entries, one unreadable")
        repo.forge({addition: [142]})
        result = repo.release("stamp")
        self.assert_exit(result, 0)
        self.assertIn("warning", result.stderr.lower(), describe(result))
        self.assertIn("undecodable.fixed.md", result.stderr, describe(result))
        self.assertEqual(broken.read_bytes(), b"- **\xff\xfe not UTF-8.**\n")
        self.assertEqual(good.read_text(), "- **Good.** (PR #142)\n")

    @unittest.skipIf(hasattr(os, "geteuid") and os.geteuid() == 0, "harness: root writes through file permissions")
    def test_stamp_never_fails_on_an_entry_it_cannot_write(self) -> None:
        repo = self.repo()
        entry = repo.entry("read-only.fixed.md", "- **Read-only.**\n")
        addition = repo.commit("fix: read-only")
        repo.forge({addition: [142]})
        directory = entry.parent
        entry.chmod(0o444)
        directory.chmod(0o555)
        self.addCleanup(entry.chmod, 0o644)
        self.addCleanup(directory.chmod, 0o755)
        result = repo.release("stamp")
        self.assert_exit(result, 0)
        self.assertIn("warning", result.stderr.lower(), describe(result))
        self.assertIn("read-only.fixed.md", result.stderr, describe(result))
        self.assertEqual(entry.read_text(), "- **Read-only.**\n")


class RefusalTests(EntryRepoCase):
    """`prepare` refuses with nothing consumed and nothing written. §FS-002-release.2.3"""

    def assert_refused_untouched(self, repo, *details: str) -> None:
        before = repo.snapshot()
        self.assert_refused(repo.release("prepare", "0.1.1", "--date", "2026-10-02"), *details)
        self.assertEqual(repo.snapshot(), before)

    def test_a_malformed_entry_refuses_before_anything_is_written(self) -> None:
        repo = self.repo()
        repo.entry("good.fixed.md")
        repo.entry("two-bullets.fixed.md", "- **One.**\n- **Two.**\n")
        repo.commit("fix: a malformed entry beside a good one")
        self.assert_refused_untouched(repo, "two-bullets.fixed.md")

    def test_a_bullet_left_under_the_shared_section_refuses_before_anything_is_written(self) -> None:
        repo = self.repo()
        repo.entry("good.fixed.md")
        repo.write(
            "docs/changelog.md",
            repo.changelog.read_text().replace("## Unreleased\n", "## Unreleased\n\n### Fixed\n\n- **Written the old way.**\n"),
        )
        repo.commit("fix: an entry written the old way")
        self.assert_refused_untouched(repo, "Unreleased")

    def test_an_existing_archive_refuses_before_anything_is_consumed(self) -> None:
        repo = self.repo()
        repo.entry("good.fixed.md")
        repo.write("docs/changelog/0.1.0.md", "# 0.1.0 — already archived\n")
        repo.commit("fix: and an archive that is already there")
        self.assert_refused_untouched(repo, "archive already exists")

    def test_the_readme_alone_is_nothing_to_release(self) -> None:
        repo = self.repo()
        self.assert_refused_untouched(repo, ENTRY_DIR)
        refusal = output(repo.release("prepare", "0.1.1", "--date", "2026-10-02"))
        self.assertIn(WRITE_IT_FIRST, refusal.lower(), "the refusal does not say to write the section first:\n" + refusal)

    def test_a_bad_version_or_date_refuses_before_anything_is_written(self) -> None:
        """Unmoved by this change: the arguments are validated first."""
        for argv in (("prepare", "one"), ("prepare", "0.1.1", "--date", "tomorrow")):
            with self.subTest(argv=argv):
                repo = self.repo()
                repo.entry("good.fixed.md")
                repo.commit("fix: good")
                before = repo.snapshot()
                self.assert_refused(repo.release(*argv), "must look like")
                self.assertEqual(repo.snapshot(), before)


class PendingTests(EntryRepoCase):
    """`pending` counts what `prepare` would read, so the hold and the refusal agree. §FS-002-release.2"""

    def assert_pending(self, repo, expected: str) -> None:
        before = repo.snapshot()
        result = repo.release("pending")
        self.assert_exit(result, 0)
        self.assertEqual(result.stdout.strip(), expected, describe(result))
        self.assertEqual(repo.snapshot(), before, "pending wrote to the tree")

    def test_the_readme_alone_is_nothing_pending(self) -> None:
        self.assert_pending(self.repo(), "0")

    def test_every_entry_is_counted(self) -> None:
        repo = self.repo()
        for name in ("one.fixed.md", "two.added.md", "three.note.md"):
            repo.entry(name)
        repo.commit("docs: three entries")
        self.assert_pending(repo, "3")

    def test_a_malformed_entry_is_pending_so_the_release_goes_on_and_prepare_names_it(self) -> None:
        repo = self.repo()
        repo.entry("good.fixed.md")
        repo.entry("two-bullets.fixed.md", "- **One.**\n- **Two.**\n")
        repo.entry("no-category.md")
        repo.commit("docs: one good entry and two malformed ones")
        self.assert_pending(repo, "3")

    def test_pending_and_the_empty_refusal_agree_on_what_counts(self) -> None:
        cases = {
            "the readme alone": (),
            "one entry": (("good.fixed.md", "- **Good.**\n"),),
            "one malformed entry": (("two-bullets.fixed.md", "- **One.**\n- **Two.**\n"),),
            "a file named as no entry": (("notes.txt", "- **Not an entry.**\n"),),
        }
        for case, files in cases.items():
            with self.subTest(case):
                repo = self.repo()
                for name, body in files:
                    repo.entry(name, body)
                counted = repo.release("pending")
                self.assert_exit(counted, 0)
                refused = repo.release("prepare", "0.1.1", "--date", "2026-10-02")
                held = counted.stdout.strip() == "0"
                self.assertEqual(held, WRITE_IT_FIRST in output(refused).lower(), describe(counted) + describe(refused))


class WriteUpTests(EntryRepoCase):
    """The section written before a release, in one pull request of its own. §FS-002-release.1

    These pass on the code as it stands, by design: ordering and stamping do not
    change. They pin what §FS-002-release.2.1 and §FS-002-release.2.2 now say
    about a write-up's entries, which all land in the one commit.
    """

    WRITE_UP = 170

    def write_up(self, repo, entries: dict[str, str]) -> str:
        for name, body in entries.items():
            repo.entry(name, body)
        addition = repo.commit("docs: write the release section")
        repo.forge({addition: [self.WRITE_UP]})
        return addition

    def test_a_write_up_s_entries_keep_their_own_numbers_and_go_by_file_name(self) -> None:
        repo = self.repo()
        repo.entry("z-landed-before.fixed.md", "- **Landed with its change, the old way.** (PR #150)\n")
        earlier = repo.commit("fix: an entry written with its change")
        repo.write("src/lib.rs", "// a change that wrote no entry\n")
        repo.commit("fix: no entry")
        entries = {
            "b-refresh-keeps-unreachable.fixed.md": (
                "- **`ephor refresh` keeps a project whose remote is unreachable.** It stays in\n"
                "  the feed, marked stale. (PR #160)\n"
            ),
            "a-stale-feed.fixed.md": "- **A stale feed says so.** (PR #161)\n",
            "clean-verb.added.md": "- **`ephor clean` gives an idle checkout's build output back.** (PR #154)\n",
        }
        self.write_up(repo, entries)
        repo.forge({earlier: [150]})

        self.assert_exit(repo.release("stamp"), 0)
        for name, body in entries.items():
            with self.subTest(stamped=name):
                self.assertEqual(repo.read_entry(name), body)

        section = release_section(self.prepare(repo))
        for body in entries.values():
            self.assertIn(body, section)
        order = sorted(("Landed with its change", "A stale feed", "`ephor refresh` keeps"), key=section.index)
        self.assertEqual(order, ["Landed with its change", "A stale feed", "`ephor refresh` keeps"], section)
        self.assertNotIn(f"PR #{self.WRITE_UP}", section)

    def test_an_entry_the_write_up_leaves_unnumbered_takes_the_write_up_s_number(self) -> None:
        repo = self.repo()
        self.write_up(
            repo,
            {
                "numbered.fixed.md": "- **Numbered.** (PR #160)\n",
                "unnumbered.fixed.md": "- **Unnumbered.**\n",
            },
        )
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(repo.read_entry("numbered.fixed.md"), "- **Numbered.** (PR #160)\n")
        self.assertEqual(repo.read_entry("unnumbered.fixed.md"), f"- **Unnumbered.** (PR #{self.WRITE_UP})\n")

    def test_a_changed_slug_starts_a_new_lifetime_where_it_changed(self) -> None:
        repo = self.repo()
        old = repo.entry("a-old-slug.fixed.md", "- **Renamed by the write-up.**\n")
        first = repo.commit("fix: an entry under its first slug")
        repo.entry("m-between.fixed.md", "- **Landed between.** (PR #151)\n")
        repo.commit("fix: another entry")
        old.unlink()
        renamed = self.write_up(repo, {"b-new-slug.fixed.md": "- **Renamed by the write-up.**\n"})
        repo.forge({first: [150], renamed: [self.WRITE_UP]})
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(repo.read_entry("b-new-slug.fixed.md"), f"- **Renamed by the write-up.** (PR #{self.WRITE_UP})\n")
        section = release_section(self.prepare(repo))
        self.assertLess(section.index("Landed between"), section.index("Renamed by the write-up"), section)


class LinkAndNotesTests(EntryRepoCase):
    """A link means the same thing in its entry, inline, in the notes and in an archive."""

    def test_links_keep_their_targets_through_two_releases(self) -> None:
        repo = self.repo()
        repo.write("docs/guide.md", "# Guide\n\n## Detail\n")
        repo.write("docs/images/example.png", "fixture\n")
        repo.entry(
            "linked.fixed.md",
            "- **Linked.** [guide](../../guide.md#detail), ![image](../../images/example.png),\n"
            "  [readme](../../../README.md), [web](https://example.invalid/page#part). (PR #142)\n",
        )
        repo.commit("fix: an entry with links")

        first = self.prepare(repo)
        inline = release_section(first)
        for link in ("[guide](guide.md#detail)", "![image](images/example.png)", "[readme](../README.md)"):
            self.assertIn(link, inline)
        self.assertIn("[web](https://example.invalid/page#part)", inline)

        notes = repo.root / "notes.md"
        self.assert_exit(repo.release("notes", "0.1.1", "--output", str(notes)), 0)
        self.assertEqual(notes.read_text().strip(), inline.strip())

        repo.commit("release: 0.1.1")
        repo.entry("next.note.md", "- **The next release.**\n")
        repo.commit("docs: the next release")
        second = self.prepare(repo, "0.1.2")
        archive = (repo.path / "docs" / "changelog" / "0.1.1.md").read_text()
        for link in ("[guide](../guide.md#detail)", "![image](../images/example.png)", "[readme](../../README.md)"):
            self.assertIn(link, archive)
        self.assertIn("[web](https://example.invalid/page#part)", archive)
        older = second.split("## 3. Older releases\n", 1)[1]
        self.assertIn("[0.1.1](changelog/0.1.1.md)", older)
        self.assertIn("[0.1.0](changelog/0.1.0.md)", older)
        self.assertEqual(unreleased_section(second).strip(), POINTER)
        self.assertEqual(repo.pending(), [])

    def test_notes_are_the_inline_section_of_the_release(self) -> None:
        """Unmoved by this change (§FS-002-release.3)."""
        repo = self.repo()
        notes = repo.root / "notes.md"
        self.assert_exit(repo.release("notes", "0.1.0", "--output", str(notes)), 0)
        text = notes.read_text()
        self.assertIn("### Added", text)
        self.assertIn("The beginning.", text)
        self.assertNotIn("Older releases", text)
        self.assertNotIn("Unreleased", text)


class WorkflowTests(unittest.TestCase):
    """The workflows give the release the history it reads, and commit what it consumed."""

    def test_release_workflows_read_full_history_stamp_first_and_commit_the_entry_directory(self) -> None:
        """Unmoved by this change, and what ordering and attribution now depend on (§FS-002-release.2)."""
        for name in ("auto-bump.yml", "release-minor.yml"):
            with self.subTest(workflow=name):
                text = (REPOSITORY_ROOT / ".github" / "workflows" / name).read_text()
                self.assertIn("fetch-depth: 0", text)
                stamp = text.index("prepare_changelog_release.py stamp")
                prepare = text.index("prepare_changelog_release.py prepare")
                self.assertLess(stamp, prepare)
                staged = [line for line in text.splitlines() if line.strip().startswith("git add")]
                self.assertTrue(
                    any(re.search(r"\sdocs/changelog(\s|$)", line) for line in staged),
                    f"{name} does not stage docs/changelog/, so consumed entries would stay: {staged}",
                )

    @staticmethod
    def steps(name: str) -> list:
        return list_items((REPOSITORY_ROOT / ".github" / "workflows" / name).read_text(encoding="utf-8"), "steps")

    def hold(self, steps: list) -> int:
        """The index of the `Auto bump` step that asks `pending`, failing the test where there is none."""
        asking = [index for index, step in enumerate(steps) if "prepare_changelog_release.py pending" in step.text]
        self.assertEqual(len(asking), 1, "Auto bump does not ask `prepare_changelog_release.py pending` in one step")
        return asking[0]

    def test_auto_bump_asks_pending_before_it_stamps_and_holds_with_a_notice(self) -> None:
        steps = self.steps("auto-bump.yml")
        hold = self.hold(steps)
        stamping = [index for index, step in enumerate(steps) if "prepare_changelog_release.py stamp" in step.text]
        self.assertTrue(stamping and hold < stamping[0], "Auto bump asks `pending` only after it stamps")
        self.assertTrue(steps[hold].keys.get("id"), "the hold has no `id:`, so no later step can read it")
        self.assertIn("::notice::", steps[hold].text, "Auto bump holds without a notice saying why")

    def test_every_auto_bump_step_after_the_hold_carries_it_the_dev_advance_included(self) -> None:
        steps = self.steps("auto-bump.yml")
        hold = self.hold(steps)
        reads = f"steps.{steps[hold].keys.get('id')}."
        unheld = [step.keys.get("name") or step.keys.get("uses") for step in steps[hold + 1 :] if reads not in step.keys.get("if", "")]
        self.assertEqual(unheld, [], f"these steps run while the release is held; their `if:` does not read `{reads}`")
        conditions = {step.keys.get("name"): step.keys.get("if") for step in steps}
        self.assertEqual(
            conditions.get("Advance main to the next dev version"),
            conditions.get("Publish release commit and dispatch release"),
            "the dev-version advance does not carry the release's own condition",
        )

    def test_release_minor_is_not_held(self) -> None:
        """A guard, and it passes today: a release a person asks for reaches `prepare`'s refusal instead."""
        asking = [step.keys.get("name") for step in self.steps("release-minor.yml") if "prepare_changelog_release.py pending" in step.text]
        self.assertEqual(asking, [], "Release minor holds on `pending`; it should reach prepare's refusal")


if __name__ == "__main__":
    unittest.main()
