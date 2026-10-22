"""The Python fixtures, re-run as a git hook runs them, pass and write nothing they did not make. §REQ-003-test-isolation.2

The pre-commit hook runs this suite, and git exports `GIT_DIR` and
`GIT_INDEX_FILE` to it. Every subprocess a fixture starts inherits them
(§REQ-003-test-isolation.3.3), so `ReleaseRepo`'s `git init` and `git remote
add origin` land in the repository being committed to, and the second fixture
to try it is refused with `remote origin already exists`. This case stands a
decoy in for that repository - a main checkout with a linked worktree, named to
a child `python -m unittest` exactly as a hook committing from the worktree
names it - and reads it back (§REQ-003-test-isolation.1).

The decoy is read, and the child handed it, through `git_env.environ`, the one
place the Python fixtures take git's repository variables off what they start
(§REQ-003-test-isolation.3.4).
"""

import ast
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from git_env import environ

HERE = Path(__file__).resolve().parent

# Two fixture repositories in one child, so a second `git remote add origin` meets the first.
FIXTURES = (
    "test_prepare_changelog_release.FirstReleaseTests.test_origin_names_the_repository_when_github_repository_is_unset",
    "test_prepare_changelog_release.FirstReleaseTests.test_notes_publish_the_first_release_s_list",
)


def decoy_git(where: Path, *args: str) -> str:
    result = subprocess.run(["git", "-C", str(where), *args], env=environ(), capture_output=True, text=True)
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
            env = environ(GIT_DIR=str(git_dir), GIT_INDEX_FILE=str(git_dir / "index"))
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


# Started by the Rust suite rather than by a fixture here, which hands them an
# environment it has already taken the variables off.
DRIVEN_FROM_RUST = ("standing_git.py", "standing_tui.py")
SPAWNS = ("run", "Popen", "call", "check_call", "check_output")


class OnePlaceTests(unittest.TestCase):
    """§REQ-003-test-isolation.3.4: every subprocess a fixture starts is handed an environment from `git_env`."""

    def test_every_subprocess_is_handed_an_environment_and_none_is_built_from_the_whole_of_os_environ(self) -> None:
        offenders = []
        for path in sorted(HERE.glob("*.py")):
            if path.name == "git_env.py" or path.name in DRIVEN_FROM_RUST:
                continue
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
                        and isinstance(node.func.value, ast.Name) and node.func.value.id == "subprocess"
                        and node.func.attr in SPAWNS
                        and not any(keyword.arg == "env" for keyword in node.keywords)):
                    offenders.append(f"{path.name}:{node.lineno} subprocess.{node.func.attr} inherits the environment")
                if self.whole_environ(node):
                    offenders.append(f"{path.name}:{node.lineno} builds an environment from all of os.environ")
        self.assertEqual(offenders, [],
                         "a fixture's subprocess would inherit a hook's GIT_DIR (§REQ-003-test-isolation.3.3); "
                         "hand it `git_env.environ(...)` instead")

    @staticmethod
    def whole_environ(node: ast.AST) -> bool:
        """`os.environ` taken whole: `.copy()`, `.items()`, `{**os.environ}` or `dict(os.environ)` - not one key read from it."""
        def is_environ(expression: ast.AST) -> bool:
            return (isinstance(expression, ast.Attribute) and expression.attr == "environ"
                    and isinstance(expression.value, ast.Name) and expression.value.id == "os")
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and is_environ(node.func.value):
            return node.func.attr in ("copy", "items")
        if isinstance(node, ast.Dict):
            return any(key is None and is_environ(value) for key, value in zip(node.keys, node.values))
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "dict":
            return any(is_environ(argument) for argument in node.args)
        return False


if __name__ == "__main__":
    unittest.main()
