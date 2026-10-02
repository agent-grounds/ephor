"""The switch-over moves what was pending into entries, and loses nothing. §FS-002-release.1.2

The copies here are written by hand, as the old `stamp` and the move would leave
them, rather than by a migration under test: what is pinned is how the release
treats a copy (§FS-002-release.2.2).
"""

from changelog_git import (
    ENTRY_DIR,
    POINTER,
    EntryRepoCase,
    changelog_text,
    describe,
    entries_readme,
    release_section,
)


# The shared section at the switch-over base, in this repository's shape: a
# category may appear more than once, a bullet may wrap or run to several
# paragraphs, and most carry no number.
LEGACY = """### Fixed

- **Written number.** [guide](guide.md#detail). (PR #104)

- **Recoverable number.**
  The old stamper resolves this continuation.

### Note

- **Unknown origin.** [guide](guide.md#detail).
  Every word survives although the forge knows nothing of it.

### Changed

- **Several paragraphs.** The first one wraps
  onto a second line.

  **A second paragraph.** It belongs to the same bullet.

### Fixed

- **Another fixed entry.** (PR #105)
"""

# Merged while the switch-over was open, so only the final base has it.
LANDED_LATE = "\n- **Landed during planning.** (PR #107)\n"

# Each bullet as its own entry after the old `stamp` ran: it appended `(PR #106)`
# and, at the end of the first paragraph where it stops reading, `(PR #108)`.
# Links are rewritten to resolve from the entry directory.
COPIES = {
    "legacy-0001.fixed.md": "- **Written number.** [guide](../../guide.md#detail). (PR #104)\n",
    "legacy-0002.fixed.md": "- **Recoverable number.**\n  The old stamper resolves this continuation. (PR #106)\n",
    "legacy-0003.note.md": (
        "- **Unknown origin.** [guide](../../guide.md#detail).\n"
        "  Every word survives although the forge knows nothing of it.\n"
    ),
    "legacy-0004.changed.md": (
        "- **Several paragraphs.** The first one wraps\n"
        "  onto a second line. (PR #108)\n"
        "\n"
        "  **A second paragraph.** It belongs to the same bullet.\n"
    ),
    "legacy-0005.fixed.md": "- **Another fixed entry.** (PR #105)\n",
    "legacy-0006.fixed.md": "- **Landed during planning.** (PR #107)\n",
}

OWN_ENTRY = "- **Pending changelog entries are one file each.**\n"


class SwitchOverTests(EntryRepoCase):
    def switch_over(self):
        """The tree before, the base it finally lands on, and the switch-over commit itself."""
        repo = self.repo(LEGACY)
        repo.write("docs/guide.md", "# Guide\n\n## Detail\n")
        repo.write("docs/changelog.md", changelog_text(LEGACY + LANDED_LATE))
        final_base = repo.commit("fix: merged while the switch-over was open")
        repo.write("docs/changelog.md", changelog_text(POINTER))
        repo.write(f"{ENTRY_DIR}/README.md", entries_readme())
        for name, body in COPIES.items():
            repo.entry(name, body)
        repo.entry("distributed-entries.changed.md", OWN_ENTRY)
        migration = repo.commit("feat: one file per pending changelog entry")
        repo.forge({repo.base: [], final_base: [107], migration: [149]})
        return repo, final_base, migration

    def test_the_release_publishes_every_moved_bullet_and_none_with_the_switch_over_s_number(self) -> None:
        repo, _, _ = self.switch_over()
        stamped = repo.release("stamp")
        self.assert_exit(stamped, 0)
        self.assertIn("legacy-0003.note.md", stamped.stderr, describe(stamped))
        self.assertEqual(repo.read_entry("legacy-0003.note.md"), COPIES["legacy-0003.note.md"])
        self.assertEqual(repo.read_entry("distributed-entries.changed.md"), OWN_ENTRY.replace("**\n", "** (PR #149)\n"))

        section = release_section(self.prepare(repo))
        for body in COPIES.values():
            self.assertIn(body.replace("../../guide.md", "guide.md"), section)
        self.assertIn("### Note\n", section)
        fixed = section.split("### Fixed\n", 1)[1].split("\n### ", 1)[0]
        order = ("Written number", "Recoverable number", "Another fixed entry", "Landed during planning")
        self.assertEqual(sorted(order, key=fixed.index), list(order), fixed)
        self.assertEqual(section.count("PR #149"), 1, section)
        self.assertEqual(repo.pending(), [])

    def test_an_edited_copy_never_takes_the_number_of_the_switch_over_or_of_its_editor(self) -> None:
        repo, _, migration = self.switch_over()
        edited = COPIES["legacy-0003.note.md"].replace("Every word survives", "Its meaning survives")
        repo.entry("legacy-0003.note.md", edited)
        edit = repo.commit("docs: clarify a moved entry")
        repo.forge({migration: [149], edit: [150]})
        stamped = repo.release("stamp")
        self.assert_exit(stamped, 0)
        self.assertIn("legacy-0003.note.md", stamped.stderr, describe(stamped))
        self.assertEqual(repo.read_entry("legacy-0003.note.md"), edited)

        section = release_section(self.prepare(repo))
        self.assertIn("Its meaning survives although the forge knows nothing of it.\n", section)
        self.assertNotIn("PR #150", section)
        self.assertEqual(section.count("PR #149"), 1, section)

    def test_an_edited_and_recategorized_copy_keeps_the_number_it_moved_with(self) -> None:
        repo, _, migration = self.switch_over()
        (repo.path / ENTRY_DIR / "legacy-0002.fixed.md").unlink()
        moved = COPIES["legacy-0002.fixed.md"].replace("resolves this", "resolved this")
        repo.entry("legacy-0002.changed.md", moved)
        edit = repo.commit("docs: move a moved entry")
        repo.forge({migration: [149], edit: [150]})
        self.assert_exit(repo.release("stamp"), 0)
        self.assertEqual(repo.read_entry("legacy-0002.changed.md"), moved)
        section = release_section(self.prepare(repo))
        self.assertIn(moved, section)
        self.assertNotIn("PR #150", section)
