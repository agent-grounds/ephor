"""The Python fixtures, re-run as a git hook runs them, pass and write nothing they did not make. §REQ-003-test-isolation.2

The pre-commit hook runs this suite, and git exports `GIT_DIR` and
`GIT_INDEX_FILE` to it. Every subprocess a fixture starts inherits them
(§REQ-003-test-isolation.3.3), so `ReleaseRepo`'s `git init` and `git remote
add origin` land in the repository being committed to, and the second fixture
to try it is refused with `remote origin already exists`. This case stands a
decoy in for that repository - a main checkout with a linked worktree, named to
a child `python -m unittest` exactly as a hook committing from the worktree
names it - and reads it back (§REQ-003-test-isolation.1).

It is marked `expectedFailure` until #233 is fixed: it runs on every pass of the
suite and fails today, and the change that makes it pass turns it into an
unexpected success, which fails the suite until the marker is removed.
"""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Two fixture repositories in one child, so a second `git remote add origin` meets the first.
FIXTURES = (
    "test_prepare_changelog_release.FirstReleaseTests.test_origin_names_the_repository_when_github_repository_is_unset",
    "test_prepare_changelog_release.FirstReleaseTests.test_notes_publish_the_first_release_s_list",
)


def unhooked_env() -> dict:
    """This process's environment with every `GIT_*` taken off, so the decoy is read on purpose."""
    return {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}


def decoy_git(where: Path, *args: str) -> str:
    result = subprocess.run(["git", "-C", str(where), *args], env=unhooked_env(), capture_output=True, text=True)
    if result.returncode != 0:
        raise AssertionError(f"git {args} on the decoy at {where} failed: {result.stderr}")
    return result.stdout


def decoy_state(main: Path, wt: Path) -> list:
    """Everything §REQ-003-test-isolation.1 measures, read without a hook's variables."""
    return [
        decoy_git(main, "config", "--local", "--list"),
        decoy_git(main, "for-each-ref", "--format=%(refname) %(objectname)"),
        decoy_git(main, "symbolic-ref", "HEAD"),
        decoy_git(wt, "symbolic-ref", "HEAD"),
        decoy_git(wt, "ls-files", "--stage"),
    ]


class UnderAWorktreeHookTests(unittest.TestCase):
    maxDiff = None

    @unittest.expectedFailure
    def test_the_fixtures_pass_under_a_worktree_hook_and_leave_its_repository_alone(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ephor-hook-decoy-") as temp:
            main = Path(temp).resolve() / "main"
            wt = main.parent / "wt"
            main.mkdir()
            decoy_git(main, "init", "-q", "-b", "main")
            (main / "README.md").write_text("decoy\n", encoding="utf-8")
            decoy_git(main, "add", "README.md")
            decoy_git(main, "-c", "user.name=decoy", "-c", "user.email=decoy@example.invalid",
                      "commit", "-q", "-m", "decoy")
            decoy_git(main, "worktree", "add", "-q", "-b", "topic", "../wt")
            (wt / "change.txt").write_text("the commit under test\n", encoding="utf-8")
            decoy_git(wt, "add", "change.txt")
            before = decoy_state(main, wt)

            # What git exports to a hook run from a linked worktree: both absolute.
            git_dir = main / ".git" / "worktrees" / "wt"
            env = {**unhooked_env(), "GIT_DIR": str(git_dir), "GIT_INDEX_FILE": str(git_dir / "index")}
            child = subprocess.run([sys.executable, "-m", "unittest", *FIXTURES],
                                   cwd=HERE, env=env, capture_output=True, text=True)
            after = decoy_state(main, wt)

            self.assertIn(f"Ran {len(FIXTURES)} tests", child.stderr, child.stderr)
            self.assertEqual(before, after,
                             f"a fixture's git wrote into {main}, a repository no test named "
                             "(§REQ-003-test-isolation.1)")
            self.assertEqual(child.returncode, 0,
                             "the fixtures fail under a worktree hook's GIT_DIR and GIT_INDEX_FILE, and pass "
                             f"without them (§REQ-003-test-isolation.2):\n{child.stderr}")


if __name__ == "__main__":
    unittest.main()
