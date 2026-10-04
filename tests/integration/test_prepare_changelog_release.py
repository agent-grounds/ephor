"""The release writes its own notes from the pull requests it ships. §FS-002-release.1.3

`prepare` and `notes` are run the way the release workflows run them, against a
real history and a forge stand-in that answers what each case says. Nobody
writes an entry first: the first release lists every qualifying pull request
from the root commit, a later one what landed since the previous tag
(§FS-002-release.1.3), and the release rotates the previous section out as it
always has. Everything is read before anything is written, and a release the
forge or the history cannot vouch for is refused with the tree untouched
(§FS-002-release.2.3). What `notes` writes is still the inline section
(§FS-002-release.3).
"""

import re
import unittest

from release_forge import (
    FIRST_CHANGELOG,
    PATHS_SCRIPT,
    PULL_URL,
    REPO,
    REPOSITORY_ROOT,
    ReleaseCase,
    describe,
    line,
    listed_numbers,
    release_section,
)


def with_release(version: str, date: str, *lines: str) -> str:
    """`FIRST_CHANGELOG` with `version` already released inline."""
    section = f"## 2. [{version}] — {date}\n\n" + "".join(f"{text}\n" for text in lines) + "\n"
    return FIRST_CHANGELOG.replace("## 3. Older releases\n", section + "## 3. Older releases\n")


class FirstReleaseTests(ReleaseCase):
    """No tag, no store: the first release lists everything from the root commit."""

    def first_history(self, repo):
        repo.land_merge(1, "feat: the first feature", ("src/one.rs",))
        repo.push(("src/direct.rs",), "chore: a substantive push no pull request landed")
        repo.land_squash(2, "fix: a squashed fix", ("src/two.rs",))
        repo.land_rebase(3, "feat: a rebased change", ("src/three.rs", "src/three_more.rs"))

    def test_the_first_release_lists_every_qualifying_pull_request_from_the_root_commit(self) -> None:
        repo = self.repo()
        self.first_history(repo)

        text = self.prepare(repo)

        lines = (
            line(3, "feat: a rebased change"),
            line(2, "fix: a squashed fix"),
            line(1, "feat: the first feature"),
        )
        self.assertEqual(text, with_release("0.1.0", self.DATE, *lines))
        self.assertFalse((repo.path / "docs" / "changelog").exists(), "a first release archived something")

    def test_notes_publish_the_first_release_s_list(self) -> None:
        repo = self.repo()
        self.first_history(repo)
        section = release_section(self.prepare(repo), "0.1.0")
        notes = repo.root / "release-notes.md"
        self.assert_exit(repo.release("notes", "0.1.0", "--output", str(notes)), 0)
        self.assertEqual(notes.read_text(encoding="utf-8"), section.strip() + "\n")
        self.assertEqual(listed_numbers(notes.read_text(encoding="utf-8")), [3, 2, 1])

    def test_origin_names_the_repository_when_github_repository_is_unset(self) -> None:
        repo = self.repo(origin=f"git@github.com:{REPO}.git", repository_env=False)
        repo.land_squash(1, "feat: found through origin", ("src/one.rs",))
        self.assertEqual(self.released(repo), [1])

    def test_this_repository_s_changelog_takes_its_first_release(self) -> None:
        shipping = (REPOSITORY_ROOT / "docs" / "changelog.md").read_text(encoding="utf-8")
        repo = self.repo(shipping)
        repo.land_squash(1, "feat: the change that ships first", ("src/one.rs",))
        text = self.prepare(repo)
        head, older = shipping.split("## 3. Older releases\n", 1)
        self.assertEqual(
            text,
            head + f"## 2. [0.1.0] — {self.DATE}\n\n" + line(1, "feat: the change that ships first") + "\n\n"
            "## 3. Older releases\n" + older,
        )


class LaterReleaseTests(ReleaseCase):
    """A later release lists what followed the previous tag, and rotates that release out."""

    def cut(self, repo, version: str, date: str) -> str:
        text = self.prepare(repo, version, date)
        repo.write("Cargo.toml", f'[package]\nversion = "{version}"\n')
        repo.commit(f"Release v{version}")
        repo.tag(version)
        return text

    def test_a_later_release_lists_only_what_followed_its_tag_and_rotates_the_previous_one(self) -> None:
        repo = self.repo()
        repo.land_squash(1, "feat: shipped in 0.1.0", ("src/one.rs",))
        first = release_section(self.cut(repo, "0.1.0", "2026-10-04"), "0.1.0")
        repo.land_merge(2, "fix: after the tag", ("src/two.rs",))
        repo.land_squash(3, "feat: later still", ("src/three.rs",))

        text = self.cut(repo, "0.1.1", "2026-11-02")

        self.assertEqual(listed_numbers(release_section(text, "0.1.1")), [3, 2])
        archive = (repo.path / "docs" / "changelog" / "0.1.0.md").read_text(encoding="utf-8")
        self.assertTrue(archive.startswith("# 0.1.0 — 2026-10-04\n"), archive)
        self.assertIn(line(1, "feat: shipped in 0.1.0"), archive)
        self.assertEqual(listed_numbers(archive), listed_numbers(first))
        older = text.split("## 3. Older releases\n", 1)[1]
        self.assertRegex(older, r"(?m)^- \[0\.1\.0\]\(changelog/0\.1\.0\.md\) — 2026-10-04")
        self.assertNotIn("_None yet._", older)
        self.assertNotIn("[0.1.0]", text.split("## 3. Older releases\n", 1)[0])

        repo.land_squash(4, "fix: in the third release", ("src/four.rs",))
        third = self.prepare(repo, "0.1.2", "2026-12-01")
        self.assertEqual(listed_numbers(release_section(third, "0.1.2")), [4])
        links = re.findall(r"(?m)^- \[(0\.1\.[01])\]\(changelog/", third.split("## 3. Older releases\n", 1)[1])
        self.assertEqual(links, ["0.1.1", "0.1.0"])


class RangeTests(ReleaseCase):
    """The range is first-parent history between the previous tag and the committed HEAD. §FS-002-release.1.3"""

    def test_only_pull_requests_landed_on_the_range_are_listed(self) -> None:
        repo = self.repo(with_release("0.1.0", "2026-09-01", line(1, "feat: before the tag")))
        repo.land_squash(1, "feat: before the tag", ("src/one.rs",))
        repo.tag("0.1.0")
        # Branched from the root commit and tagged there: reachable after its merge, never first-parent.
        repo.land_merge(2, "feat: merged with a merge commit", ("src/two.rs",), branch_from=repo.root_commit)
        repo.tag("0.0.5", "pr-2")
        repo.land_squash(3, "fix: squashed", ("src/three.rs",))
        repo.git("tag", "v1.0")
        repo.git("tag", "nightly")
        repo.land_rebase(4, "feat: rebased in two commits", ("src/four.rs", "src/four_more.rs"))
        repo.git("checkout", "-q", "-b", "next")
        repo.land_merge(5, "feat: merged into another branch", ("src/five.rs",), into="next")
        repo.git("checkout", "-q", "main")
        repo.git("merge", "-q", "--no-ff", "-m", "Merge pull request #7", "next")
        repo.record(7, "chore: bring next into main", repo.git("rev-parse", "HEAD"), [{"filename": "src/five.rs", "status": "added"}])
        candidate = repo.git("rev-parse", "HEAD")
        repo.land_squash(6, "fix: merged after the candidate", ("src/six.rs",))
        repo.sync_origin()
        repo.git("checkout", "-q", "--detach", candidate)

        self.assertEqual(self.released(repo, "0.1.1"), [7, 4, 3, 2])


class SelectionTests(ReleaseCase):
    """Which pull requests qualify, in what order, and how each line is written. §FS-002-release.1.3"""

    def test_newest_merge_first_and_the_higher_number_first_within_a_second(self) -> None:
        repo = self.repo()
        repo.land_squash(10, "feat: oldest", ("src/a.rs",), merged_at="2026-03-01T10:00:00Z")
        repo.land_squash(11, "feat: tied, lower number", ("src/b.rs",), merged_at="2026-03-02T10:00:00Z")
        repo.land_squash(12, "feat: tied, higher number", ("src/c.rs",), merged_at="2026-03-02T10:00:00Z")
        repo.land_squash(9, "feat: newest, opened first", ("src/d.rs",), merged_at="2026-03-03T10:00:00Z")
        self.assertEqual(self.released(repo), [9, 12, 11, 10])

    def test_a_title_is_written_as_literal_link_text(self) -> None:
        repo = self.repo()
        repo.land_squash(5, "fix: `a` [b] *c* _d_ <e> \\ f\r\n  next  ", ("src/a.rs",))
        section = release_section(self.prepare(repo), "0.1.0")
        expected = r"- [fix: \`a\` \[b\] \*c\* \_d\_ \<e\> \\ f next](" + PULL_URL.format(number=5) + ") (PR #5)"
        self.assertEqual(section.strip(), expected)

    def test_paths_are_judged_by_the_shared_predicate_and_renames_by_where_they_went(self) -> None:
        repo = self.repo()
        skipped = {
            20: ("docs/guide.md",),
            21: ("docs/images/a.png",),
            22: (".github/workflows/ci.yml",),
            23: (".agents/rhei/x.yaml",),
            24: (".agent-grounds/fissile.toml",),
            25: ("tests/e2e/cases/E2E-001.rs",),
            26: ("src/notes.md",),
            27: ("LICENSE", "AGENTS.md", "CLAUDE.md", "grund.toml"),
            28: (("rename", "src/old.rs", "docs/old.rs"),),
        }
        listed = {
            30: ("docs/guide.md", "src/mixed.rs"),
            31: (("rename", "docs/moved.txt", "src/moved.rs"),),
            32: ("tests/integration/test_x.py",),
            33: ("scripts/x.py",),
            34: ("docsy/not-docs.rs",),
            35: ("sub/LICENSE",),
        }
        for number, paths in {**skipped, **listed}.items():
            repo.land_squash(number, f"change {number}", paths)
        self.assertEqual(sorted(self.released(repo)), sorted(listed))

    def test_the_predicate_is_one_script_the_schedule_can_call(self) -> None:
        repo = self.repo()
        paths = ["docs/a.md", "src/lib.rs", ".github/x.yml", "README.md", "LICENSE", "sub/LICENSE", "tests/e2e/a.rs", "assets/x.json"]
        result = repo.run(PATHS_SCRIPT, stdin="\n".join(paths) + "\n")
        self.assert_exit(result, 0)
        self.assertEqual(result.stdout.splitlines(), ["src/lib.rs", "sub/LICENSE", "assets/x.json"], describe(result))

    def test_unmerged_pull_requests_are_never_listed(self) -> None:
        repo = self.repo()
        repo.land_squash(1, "feat: merged", ("src/a.rs",))
        repo.record(2, "feat: closed without merging", "0" * 40, [{"filename": "src/b.rs", "status": "added"}], merged=False)
        repo.record(3, "feat: still open", "1" * 40, [{"filename": "src/c.rs", "status": "added"}], state="open", merged=False)
        self.assertEqual(self.released(repo), [1])


class CompletenessTests(ReleaseCase):
    """Every page is read, and a listing that stops short is refused. §FS-002-release.1.3"""

    def many(self, repo, count: int) -> None:
        for number in range(1, count + 1):
            repo.land_squash(number, f"feat: change {number}", (f"src/gen/p{number}.rs",))

    def test_every_page_of_merged_pull_requests_is_read(self) -> None:
        repo = self.repo()
        self.many(repo, 130)
        self.assertEqual(self.released(repo), list(range(130, 0, -1)))

    def test_a_pull_request_listing_that_stops_short_is_refused(self) -> None:
        repo = self.repo()
        self.many(repo, 130)
        repo.forge_extra["truncate_pulls_after"] = 100
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.0", lambda: repo.forge_extra.pop("truncate_pulls_after"))

    def files_beyond_the_first_pages(self, repo, docs: int) -> None:
        """#1 changes `docs` documentation files and one source file, listed last; #2 only the docs."""
        repo.land_squash(1, "feat: the source change is on the last page", [f"docs/gen/a{i}.md" for i in range(docs)] + ["src/last.rs"])
        repo.land_squash(2, "docs: only documentation", [f"docs/gen/b{i}.md" for i in range(docs)])

    def test_every_page_of_a_pull_request_s_files_is_read(self) -> None:
        repo = self.repo()
        self.files_beyond_the_first_pages(repo, 250)
        self.assertEqual(self.released(repo), [1])

    def test_a_file_listing_short_of_changed_files_is_refused(self) -> None:
        repo = self.repo()
        self.files_beyond_the_first_pages(repo, 250)
        repo.forge_extra["truncate_files"] = {"1": 200}
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.0", lambda: repo.forge_extra.pop("truncate_files"))

    def test_a_pull_request_past_the_file_cap_is_refused(self) -> None:
        repo = self.repo()
        repo.land_squash(1, "feat: too many files to list", [f"docs/gen/c{i}.md" for i in range(3000)] + ["src/last.rs"])
        repo.sync_origin()

        def within_the_cap() -> None:
            # The forge's word only: the same pull request, one documentation file fewer.
            pull = repo.pulls[0]
            pull["files"] = pull["files"][1:]
            pull["changed_files"] = len(pull["files"])

        self.assert_refused_until(repo, "0.1.0", within_the_cap)


class RefusalTests(ReleaseCase):
    """`prepare` names the reason and writes nothing; without the reason it releases. §FS-002-release.2.3"""

    def ready(self, changelog: str = FIRST_CHANGELOG):
        repo = self.repo(changelog)
        repo.land_squash(1, "feat: would be released", ("src/one.rs",))
        repo.sync_origin()
        return repo

    def tagged(self):
        """`v0.1.0` released and tagged; nothing landed since."""
        repo = self.repo(with_release("0.1.0", "2026-09-01", line(1, "feat: shipped")))
        repo.land_squash(1, "feat: shipped", ("src/one.rs",))
        repo.tag("0.1.0")
        return repo

    def qualifying(self, repo, number: int = 90):
        """Undo for a range with nothing to release: land one pull request that qualifies."""
        return lambda: repo.land_squash(number, "feat: a change that qualifies", (f"src/q{number}.rs",))

    def test_no_forge_on_path(self) -> None:
        repo = self.ready()
        self.assert_refused_until(repo, "0.1.0", lambda: None, "gh", env=repo.without_gh())

    def test_a_failing_forge_response_in_its_own_words(self) -> None:
        repo = self.ready()
        repo.forge_extra["fail"] = {r"pulls/1/files$": "HTTP 502: Bad Gateway"}
        self.assert_refused_until(repo, "0.1.0", lambda: repo.forge_extra.pop("fail"), "HTTP 502: Bad Gateway")

    def test_a_shallow_history(self) -> None:
        repo = self.ready()
        shallow = repo.path / ".git" / "shallow"
        shallow.write_text(repo.git("rev-parse", "HEAD") + "\n")
        self.assert_refused_until(repo, "0.1.0", shallow.unlink, "shallow")

    def test_a_head_off_the_default_branch_s_first_parent_history(self) -> None:
        repo = self.ready()
        repo.git("checkout", "-q", "-b", "elsewhere")
        repo.push(("src/elsewhere.rs",), "feat: not on main")
        self.assert_refused_until(repo, "0.1.0", lambda: repo.git("checkout", "-q", "main"))

    def test_a_version_not_past_the_previous_tag(self) -> None:
        for version in ("0.1.0", "0.0.9"):
            with self.subTest(version=version):
                repo = self.tagged()
                repo.land_squash(2, "feat: next", ("src/two.rs",))
                repo.sync_origin()
                self.assert_refused_until(repo, version, lambda: None, version, then="0.1.1")

    def test_a_malformed_version_or_date(self) -> None:
        """A guard, unmoved by this change: the arguments are validated first."""
        repo = self.ready()
        for argv in (("prepare", "one", "--date", self.DATE), ("prepare", "0.1.0", "--date", "tomorrow")):
            with self.subTest(argv=argv):
                before = repo.snapshot()
                result = repo.release(*argv)
                self.assertNotEqual(result.returncode, 0, describe(result))
                self.assertIn("must look like", result.stderr, describe(result))
                self.assertEqual(repo.snapshot(), before)

    def test_a_changelog_without_its_layout(self) -> None:
        repo = self.ready(FIRST_CHANGELOG.replace("## 3. Older releases\n\n_None yet._\n", ""))
        self.assert_refused_until(repo, "0.1.0", lambda: repo.changelog.write_text(FIRST_CHANGELOG), "Older releases")

    def test_an_archive_that_already_exists(self) -> None:
        repo = self.tagged()
        archive = repo.write("docs/changelog/0.1.0.md", "# 0.1.0 — already archived\n")
        repo.land_squash(2, "feat: next", ("src/two.rs",))
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.1", archive.unlink, "already exists")

    def test_an_empty_range(self) -> None:
        repo = self.tagged()
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.1", self.qualifying(repo), "pull request")

    def test_a_range_of_documentation_only_pull_requests(self) -> None:
        repo = self.repo()
        repo.land_squash(1, "docs: only documentation", ("docs/guide.md",))
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.0", self.qualifying(repo), "pull request")

    def test_a_range_of_pushes_no_pull_request_landed(self) -> None:
        repo = self.repo()
        repo.push(("src/direct.rs",))
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.0", self.qualifying(repo), "pull request")

    def test_a_pull_request_url_outside_this_repository(self) -> None:
        repo = self.repo()
        repo.land_squash(1, "feat: linked elsewhere", ("src/one.rs",), url="https://github.com/someone/else/pull/1")
        repo.sync_origin()
        self.assert_refused_until(repo, "0.1.0", lambda: repo.pulls[0].update(html_url=PULL_URL.format(number=1)))


if __name__ == "__main__":
    unittest.main()
