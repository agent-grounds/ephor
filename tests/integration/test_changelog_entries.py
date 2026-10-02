"""Pending changelog entries, one file each, from both ends. §FS-002-release.1

Three things no single script shows. The format is grund's, byte for byte
(§FS-002-release.1.1). This repository's own tree has made the move: its shared
section is a pointer, and every entry waiting in it is one a release collects.
And the reason for all of it — two pull requests that each record a change
integrate in either order, merged or rebased, and both entries then pass the
gate (§FS-002-release.6), are stamped with their own numbers and are released
(§FS-002-release.2).
"""

import hashlib
import shutil
import unittest

from changelog_git import (
    CATEGORIES,
    ENTRY_DIR,
    PORTABLE,
    REPOSITORY_ROOT,
    SCHEMA_COMMIT,
    EntryRepoCase,
    release_section,
    unreleased_section,
)


README = REPOSITORY_ROOT / ENTRY_DIR / "README.md"
PART_TWO = b"## Part two:"


class FormatTests(unittest.TestCase):
    """The format is not ephor's own, and the copy of it does not drift."""

    def test_the_readme_carries_grund_s_part_one_byte_for_byte(self) -> None:
        self.assertTrue(README.is_file(), f"{ENTRY_DIR}/README.md is missing")
        readme = README.read_bytes()
        self.assertIn(PART_TWO, readme)
        part_one = readme.split(PART_TWO, 1)[0]
        self.assertEqual(part_one, PORTABLE.read_bytes(), f"part one differs from grund {SCHEMA_COMMIT}")

    def test_the_pinned_part_one_is_the_one_grund_published(self) -> None:
        # `docs/changelog/unreleased/README.md` at grund 16fa32b4c087583254f757e52480fda966da0621,
        # everything above `## Part two:`. Moving to a newer grund changes this on purpose.
        digest = hashlib.sha256(PORTABLE.read_bytes()).hexdigest()
        self.assertEqual(digest, "29280a72551cfbb0123837a97f3482b2afb5763192d04a62ed597adf8536f3b4")

    def test_part_two_names_ephor_s_categories_in_release_order(self) -> None:
        self.assertTrue(README.is_file(), f"{ENTRY_DIR}/README.md is missing")
        part_two = README.read_text(encoding="utf-8").split(PART_TWO.decode(), 1)[1]
        positions = [part_two.find(f"`{category}`") for category in CATEGORIES]
        self.assertNotIn(-1, positions, f"part two does not name every category: {CATEGORIES}")
        self.assertEqual(positions, sorted(positions), "part two names the categories out of release order")


class ThisRepositoryTests(EntryRepoCase):
    """The shipping tree, not a fixture: the move has happened here."""

    def test_the_shared_section_is_a_pointer_that_holds_no_bullet(self) -> None:
        section = unreleased_section((REPOSITORY_ROOT / "docs" / "changelog.md").read_text(encoding="utf-8"))
        bullets = [line for line in section.splitlines() if line.startswith("- ")]
        self.assertFalse(bullets, f"## Unreleased still holds {len(bullets)} bullet(s), the first: {bullets[:1]}")
        self.assertIn("changelog/unreleased/", section)

    def test_every_entry_waiting_here_is_one_a_release_collects(self) -> None:
        self.assertTrue(README.is_file(), f"{ENTRY_DIR}/README.md is missing")
        entries = sorted(p.name for p in README.parent.iterdir() if p.name != "README.md")
        if not entries:
            return  # just after a release there is nothing waiting, and nothing to collect

        repo = self.repo()
        shutil.rmtree(repo.path / "docs")
        shutil.copytree(REPOSITORY_ROOT / "docs" / "changelog", repo.path / "docs" / "changelog")
        shutil.copy2(REPOSITORY_ROOT / "docs" / "changelog.md", repo.path / "docs" / "changelog.md")
        repo.commit("the shipping changelog and its entries")

        released = release_section(self.prepare(repo, "99.0.0"), "99.0.0")
        published = [line for line in released.splitlines() if line.startswith("- ")]
        self.assertEqual(len(published), len(entries), "a release of this tree did not publish one bullet per entry")
        self.assertEqual(repo.pending(), [])
        self.assertTrue((repo.path / ENTRY_DIR / "README.md").is_file())


class IntegrationTests(EntryRepoCase):
    """Two pull requests, each adding one entry, in every order they can land."""

    def land(self, operation: str, first: str, second: str) -> None:
        repo = self.repo()
        numbers = {"alpha": 142, "beta": 143}
        verdicts = {}
        for label, number in numbers.items():
            repo.git("checkout", "-q", "-b", f"probe-{label}", repo.base)
            repo.entry(f"probe-{label}.fixed.md", f"- **The {label} change is recorded.**\n")
            repo.commit(f"fix: {label}")
            verdicts[label] = repo.gate("--base-rev", repo.base, "--pr-number", str(number))

        if operation == "merge":
            repo.git("checkout", "-q", "-b", "candidate", repo.base)
            repo.git("merge", "-q", "--no-ff", "--no-edit", f"probe-{first}")
            repo.git("merge", "-q", "--no-ff", "--no-edit", f"probe-{second}")
        else:
            repo.git("checkout", "-q", "-b", "candidate", f"probe-{second}")
            repo.git("rebase", "-q", f"probe-{first}")

        # What the conflict probe established: no conflict, both entries kept.
        self.assertEqual(repo.git("diff", "--name-only", "--diff-filter=U"), "")
        self.assertEqual(repo.git("diff", repo.base, "--", "docs/changelog.md"), "")
        self.assertEqual(repo.pending(), ["probe-alpha.fixed.md", "probe-beta.fixed.md"])

        # What it could not: each entry passes the gate, is stamped with its own
        # number, and is released, in the order it landed.
        for label in numbers:
            with self.subTest(stage="gate", entry=label):
                self.assert_exit(verdicts[label], 0)
        pulls = {}
        for line in repo.git("log", "--all", "--format=%H %s").splitlines():
            sha, subject = line.split(" ", 1)
            for label, number in numbers.items():
                if subject in (f"fix: {label}", f"Merge branch 'probe-{label}' into candidate"):
                    pulls[sha] = [number]
        repo.forge(pulls)
        self.assert_exit(repo.release("stamp"), 0)
        for label, number in numbers.items():
            with self.subTest(stage="stamp", entry=label):
                self.assertEqual(
                    repo.read_entry(f"probe-{label}.fixed.md"), f"- **The {label} change is recorded.** (PR #{number})\n"
                )
        section = release_section(self.prepare(repo))
        for label, number in numbers.items():
            self.assertIn(f"- **The {label} change is recorded.** (PR #{number})\n", section)
        self.assertLess(section.index(f"The {first} change"), section.index(f"The {second} change"))
        self.assertEqual(repo.pending(), [])

    def test_merge_alpha_then_beta(self) -> None:
        self.land("merge", "alpha", "beta")

    def test_merge_beta_then_alpha(self) -> None:
        self.land("merge", "beta", "alpha")

    def test_rebase_beta_onto_alpha(self) -> None:
        self.land("rebase", "alpha", "beta")

    def test_rebase_alpha_onto_beta(self) -> None:
        self.land("rebase", "beta", "alpha")


if __name__ == "__main__":
    unittest.main()
