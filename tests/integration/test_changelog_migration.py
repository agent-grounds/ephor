"""The switch-over moves what was pending into entries, and loses nothing. §FS-002-release.1.2

The copies here are written by hand, as the old `stamp` and the move would leave
them, rather than by a migration under test: what is pinned is how the gate
(§FS-002-release.6) and the release (§FS-002-release.2.2) treat a copy, and
that the exception for one cannot be claimed by anything else.
"""

from changelog_git import (
    ENTRY_DIR,
    POINTER,
    EntryRepoCase,
    changelog_text,
    describe,
    entries_readme,
    output,
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
FOREIGN_NUMBERS = ("104", "105", "106", "107", "108")


class SwitchOverTests(EntryRepoCase):
    def switch_over(self, *, own_entry: str | None = OWN_ENTRY):
        """The tree before, the base it finally lands on, and the switch-over commit itself."""
        repo = self.repo(LEGACY)
        repo.write("docs/guide.md", "# Guide\n\n## Detail\n")
        repo.write("docs/changelog.md", changelog_text(LEGACY + LANDED_LATE))
        final_base = repo.commit("fix: merged while the switch-over was open")
        repo.write("docs/changelog.md", changelog_text(POINTER))
        repo.write(f"{ENTRY_DIR}/README.md", entries_readme())
        for name, body in COPIES.items():
            repo.entry(name, body)
        if own_entry is not None:
            repo.entry("distributed-entries.changed.md", own_entry)
        migration = repo.commit("feat: one file per pending changelog entry")
        repo.forge({repo.base: [], final_base: [107], migration: [149]})
        return repo, final_base, migration

    # --- the gate ---------------------------------------------------------------

    def test_the_switch_over_passes_the_gate_on_its_own_entry(self) -> None:
        repo, final_base, _ = self.switch_over()
        self.assert_exit(repo.gate("--base-rev", final_base, "--pr-number", "149"), 0)

    def test_copies_alone_are_not_an_added_entry(self) -> None:
        repo, final_base, _ = self.switch_over(own_entry=None)
        refused = repo.gate("--base-rev", final_base, "--pr-number", "149")
        self.assert_refused(refused, ENTRY_DIR)
        for number in FOREIGN_NUMBERS:
            # Refused for the missing entry, not by a number check copies are spared.
            self.assertNotIn(f"#{number}", output(refused), describe(refused))

    def test_the_switch_over_s_own_entry_is_held_to_the_number_check(self) -> None:
        repo, final_base, _ = self.switch_over(own_entry="- **Pending entries are files.** (PR #104)\n")
        self.assert_refused(
            repo.gate("--base-rev", final_base, "--pr-number", "149"), "distributed-entries.changed.md", "104", "149"
        )

    def test_a_copy_that_does_not_match_the_base_is_an_ordinary_entry(self) -> None:
        cases = (
            ("reworded", "legacy-0001.fixed.md", "- **Written number, reworded.** [guide](../../guide.md#detail). (PR #104)\n"),
            ("recategorized", "legacy-0001.note.md", COPIES["legacy-0001.fixed.md"]),
            ("renumbered", "legacy-0001.fixed.md", COPIES["legacy-0001.fixed.md"].replace("#104", "#109")),
        )
        for description, name, body in cases:
            with self.subTest(description):
                repo, final_base, _ = self.switch_over()
                (repo.path / ENTRY_DIR / "legacy-0001.fixed.md").unlink()
                repo.entry(name, body)
                repo.commit("feat: the switch-over, with one copy that is not a copy")
                number = "109" if description == "renumbered" else "104"
                self.assert_refused(repo.gate("--base-rev", final_base, "--pr-number", "149"), name, number)

    def test_copies_are_recognized_against_the_base_they_land_on(self) -> None:
        repo, _, _ = self.switch_over()
        # The base it was planned against never had the late bullet.
        self.assert_refused(repo.gate("--base-rev", repo.base, "--pr-number", "149"), "legacy-0006.fixed.md", "107")

    def test_after_the_switch_over_a_copy_of_moved_text_is_an_ordinary_entry(self) -> None:
        repo, _, migration = self.switch_over()
        repo.entry("pretend-legacy.fixed.md", COPIES["legacy-0001.fixed.md"])
        repo.commit("fix: an entry copied from one that moved")
        self.assert_refused(repo.gate("--base-rev", migration, "--pr-number", "150"), "pretend-legacy.fixed.md", "104")

    # --- the release -------------------------------------------------------------

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
