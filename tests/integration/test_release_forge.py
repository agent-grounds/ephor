"""The release fixture's own `git` outlasts git's transient on macOS, and nothing else. §FS-002-release.1.3

The tests of §FS-002-release.1.3 build their histories through `ReleaseRepo`
in `release_forge.py`, hundreds of `git` processes at a time. On the hosted
macOS runner, now and then one of them fails to write a loose object. git prints
`error: unable to create temporary file: Invalid argument` and exits 128, and
the test died at the fixture's `git` call before the release ever ran (#202).
The fixture retries that one transient, at most three attempts in all, says on
stderr that it did, and raises every other failure after one call, exactly as
it always has.

Linux never fails that way, so a `git` stand-in written first on the fixture's
`PATH`, beside the `gh` stand-in, makes it fail on purpose. Every call passes
through to the real `git` except the ones a case arms, which write nothing,
print what git printed on CI, and exit 128. Nothing here is random and nothing
is timed. Where the pause between attempts is taken with `time.sleep`, the
suite does not wait it out.
"""

import io
import json
import shutil
import sys
import unittest
from contextlib import redirect_stderr
from unittest import mock

from release_forge import ReleaseCase, ReleaseRepo


TRANSIENT = "error: unable to create temporary file: Invalid argument"
# What git 2.55.0 printed on the runner when commit 109's commit object could not be written (run 37717956180).
COMMIT_TRANSIENT = f"{TRANSIENT}\nfatal: failed to write commit object\n"
# And when `git add -A` could not write the blob of docs/gen/b200.md (run 37721124392).
ADD_TRANSIENT = (
    f"{TRANSIENT}\n"
    "error: docs/gen/b200.md: failed to insert into database\n"
    "error: unable to index file 'docs/gen/b200.md'\n"
    "fatal: adding files failed\n"
)
# The same object store refusing for good: the same exit and the same last line, not the transient.
REFUSED = "error: insufficient permission for adding an object to repository database .git/objects"
NOT_TRANSIENT = f"{REFUSED}\nfatal: failed to write commit object\n"

GIT_STAND_IN = r'''
import json, os, sys

args = sys.argv[1:]
with open(FAULTS, encoding="utf-8") as armed_file:
    armed = json.load(armed_file)
fault = None
if armed and args[:1] == [armed["subcommand"]] and armed["stderr"]:
    fault = armed["stderr"].pop(0)
    with open(FAULTS, "w", encoding="utf-8") as armed_file:
        json.dump(armed, armed_file)
with open(CALLS, "a", encoding="utf-8") as log:
    log.write(json.dumps({"args": args, "faulted": fault is not None}) + "\n")
if fault is None:
    os.execv(REAL_GIT, [REAL_GIT, *args])
sys.stderr.write(fault)
sys.exit(128)
'''


class GitStandIn:
    """The `git` the fixture finds first on its `PATH`: the real one, except for the calls a case arms."""

    def __init__(self, repo: ReleaseRepo) -> None:
        real = shutil.which("git")
        assert real is not None, "harness: git is not on PATH"
        self.faults = repo.root / "git-faults.json"
        self.log = repo.root / "git-calls.log"
        self.faults.write_text("null", encoding="utf-8")
        self.log.touch()
        stand_in = repo.bin / "git"
        stand_in.write_text(
            f"#!{sys.executable}\nREAL_GIT = {real!r}\nFAULTS = {str(self.faults)!r}\nCALLS = {str(self.log)!r}\n"
            + GIT_STAND_IN,
            encoding="utf-8",
        )
        stand_in.chmod(0o755)
        repo.git("version")
        assert self.calls("version"), "harness: the fixture's `git` does not resolve to the stand-in on its PATH"

    def arm(self, subcommand: str, *stderr: str) -> None:
        """Fail the next calls of `git <subcommand>`, one per `stderr`, each printing it and exiting 128."""
        self.faults.write_text(json.dumps({"subcommand": subcommand, "stderr": list(stderr)}), encoding="utf-8")

    def entries(self, subcommand: str) -> list[dict]:
        entries = [json.loads(text) for text in self.log.read_text(encoding="utf-8").splitlines() if text]
        return [entry for entry in entries if entry["args"][:1] == [subcommand]]

    def calls(self, subcommand: str) -> list[list[str]]:
        """The arguments of every `git <subcommand>` the fixture ran, in order."""
        return [entry["args"] for entry in self.entries(subcommand)]

    def faulted(self, subcommand: str) -> list[bool]:
        """For every `git <subcommand>` the fixture ran, in order, whether the stand-in failed it."""
        return [entry["faulted"] for entry in self.entries(subcommand)]


class FixtureGitTests(ReleaseCase):
    """`ReleaseRepo.git` retries git's transient `unable to create temporary file`, and only that. #202"""

    def setUp(self) -> None:
        # The pause between attempts, where `time.sleep` takes it, is not this suite's to wait out.
        pause = mock.patch("time.sleep")
        pause.start()
        self.addCleanup(pause.stop)

    def fixture(self) -> tuple[ReleaseRepo, GitStandIn, str]:
        repo = self.repo()
        return repo, GitStandIn(repo), repo.git("rev-parse", "HEAD")

    def gained(self, repo: ReleaseRepo, before: str) -> list[str]:
        """The commits `HEAD` gained since `before`, newest first."""
        return repo.git("rev-list", f"{before}..HEAD").splitlines()

    def assert_retry_said(self, said: str, args: list[str]) -> None:
        """One retry, one line on stderr, naming the `git` call it repeats."""
        lines = [text for text in said.splitlines() if text.strip()]
        self.assertEqual(len(lines), 1, f"one retry should say so in one line on stderr; it said:\n{said}")
        for arg in args:
            self.assertIn(arg, lines[0], f"the retry's line does not name the git argument {arg!r}")

    def test_a_commit_that_meets_the_transient_once_still_lands(self) -> None:
        repo, git, before = self.fixture()
        git.arm("commit", COMMIT_TRANSIENT)

        with redirect_stderr(io.StringIO()) as said:
            landing = repo.land_squash(109, "feat: change 109", ("src/gen/p109.rs",))

        self.assertEqual(self.gained(repo, before), [landing.sha])
        self.assertEqual(repo.git("log", "-1", "--format=%s", landing.sha), "feat: change 109 (#109)")
        self.assertEqual(repo.git("ls-tree", "-r", "--name-only", landing.sha, "--", "src/gen"), "src/gen/p109.rs")
        self.assertEqual(git.faulted("commit"), [True, False])
        self.assert_retry_said(said.getvalue(), git.calls("commit")[0])

    def test_a_commit_whose_add_meets_the_transient_once_still_lands_every_file(self) -> None:
        repo, git, before = self.fixture()
        paths = [f"docs/gen/b{i}.md" for i in range(198, 203)]
        git.arm("add", ADD_TRANSIENT)

        with redirect_stderr(io.StringIO()) as said:
            landing = repo.land_squash(2, "docs: only documentation", paths)

        self.assertEqual(self.gained(repo, before), [landing.sha])
        self.assertEqual(repo.git("ls-tree", "-r", "--name-only", landing.sha, "--", "docs/gen").splitlines(), paths)
        self.assertEqual(git.faulted("add"), [True, False])
        self.assert_retry_said(said.getvalue(), git.calls("add")[0])

    def test_any_other_failure_is_raised_after_one_call_as_it_always_was(self) -> None:
        repo, git, before = self.fixture()
        repo.write("src/lib.rs", "// a change the object store refuses\n")
        git.arm("commit", NOT_TRANSIENT, NOT_TRANSIENT, NOT_TRANSIENT)

        with self.assertRaises(AssertionError) as raised:
            repo.commit("feat: refused")

        self.assertEqual(len(git.calls("commit")), 1, "a failure that is not the transient was tried again")
        self.assertEqual(
            str(raised.exception),
            f"fixture: git commit -q --allow-empty -m feat: refused failed: {NOT_TRANSIENT.strip()}",
        )
        self.assertEqual(repo.git("rev-parse", "HEAD"), before)

    def test_a_transient_that_outlasts_the_bound_is_raised_with_every_attempt_s_words(self) -> None:
        repo, git, before = self.fixture()
        git.arm("commit", *[COMMIT_TRANSIENT] * 10)

        with redirect_stderr(io.StringIO()), self.assertRaises(AssertionError) as raised:
            repo.land_squash(109, "feat: change 109", ("src/gen/p109.rs",))

        attempts = len(git.calls("commit"))
        self.assertLessEqual(attempts, 3, "the transient was tried more than three times in all")
        message = str(raised.exception)
        self.assertTrue(message.startswith("fixture: git commit -q --allow-empty -m feat: change 109 (#109) failed"), message)
        self.assertEqual(message.count(TRANSIENT), attempts, f"the raise does not carry every attempt's stderr:\n{message}")
        self.assertEqual(repo.git("rev-parse", "HEAD"), before)

    def test_a_retry_that_fails_otherwise_is_raised_at_once_with_both_attempts_words(self) -> None:
        repo, git, before = self.fixture()
        repo.write("src/lib.rs", "// a change the object store refuses on the retry\n")
        git.arm("commit", COMMIT_TRANSIENT, NOT_TRANSIENT, COMMIT_TRANSIENT)

        with redirect_stderr(io.StringIO()), self.assertRaises(AssertionError) as raised:
            repo.commit("feat: refused on the retry")

        self.assertEqual(
            len(git.calls("commit")), 2, "the transient, then a different failure, is two calls and the second is raised"
        )
        message = str(raised.exception)
        self.assertIn(TRANSIENT, message, "the raise dropped the first attempt's stderr")
        self.assertIn(REFUSED, message, "the raise hides the failure that ended the retry")
        self.assertEqual(repo.git("rev-parse", "HEAD"), before)


if __name__ == "__main__":
    unittest.main()
