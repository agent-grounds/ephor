"""A throwaway git repository carrying `docs/changelog.md` and its pending entries.

Not a test module — `unittest discover -p 'test_*.py'` does not collect it. It is
shared by every changelog test because the gate and the release are two halves
of one rule and both need a real repository rather than loose files: the gate
compares the entries against a base commit (§FS-002-release.6), and the release
orders and attributes them by the history that added them (§FS-002-release.2.1).

The scripts run as the hook and the workflows run them — by argv, in the
checkout, in a process of their own — so nothing here assumes how they are
written inside. The forge is the one thing faked: `gh` on `PATH` is a stub that
answers from a JSON file the test writes and logs every call, so a refused or
rate-limited forge is a case and not an accident of the machine.

Every path handed out is resolved first. The CI matrix runs on macOS, where
`TMPDIR` is a symlink, so the raw `mkdtemp()` string and the path git reports for
the same directory are two different strings; resolving once here keeps any
comparison between them honest. Scratch repositories go wherever `TMPDIR` says.
"""

from __future__ import annotations

import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import NamedTuple


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

ENTRY_DIR = "docs/changelog/unreleased"
CATEGORIES = ("added", "changed", "deprecated", "removed", "fixed", "security", "note")

# What the switch-over leaves under `## Unreleased`: a pointer, and no bullet.
POINTER = "Pending changes are one file each in [changelog/unreleased/](changelog/unreleased/)."

# grund's README part one at the commit §FS-002-release.1.1 pins, byte for byte.
SCHEMA_COMMIT = "16fa32b4c087583254f757e52480fda966da0621"
PORTABLE = Path(__file__).resolve().parent / "fixtures" / "grund-unreleased-portable.txt"

# A repository of our own, told nothing by the machine it runs on: no global or
# system configuration, a fixed identity and date, and no credential prompt.
GIT_ENV = {
    "GIT_AUTHOR_NAME": "Changelog Fixture",
    "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
    "GIT_COMMITTER_NAME": "Changelog Fixture",
    "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
    "GIT_AUTHOR_DATE": "2026-01-02T00:00:00+00:00",
    "GIT_COMMITTER_DATE": "2026-01-02T00:00:00+00:00",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_TERMINAL_PROMPT": "0",
}

# The forge, as far as the gate and the release ask it anything: the branch's
# pull request, and the pull requests a commit belongs to. A response that is a
# string is a refusal, printed the way `gh` prints one. Anything else is refused
# loudly, so a call the tests did not expect shows up in the warning it causes.
GH_STUB = '''
import json, os, re, sys
from pathlib import Path

args = sys.argv[1:]
with open(os.environ["CHANGELOG_FORGE_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps(args) + "\\n")
data = json.loads(Path(os.environ["CHANGELOG_FORGE_DATA"]).read_text(encoding="utf-8"))

if args[:2] == ["pr", "view"]:
    number = data.get("branch_pr")
    if number is None:
        print("no pull requests found for branch", file=sys.stderr)
        sys.exit(1)
    print(number)
    sys.exit(0)

match = re.search(r"commits/([0-9a-f]{40})/pulls", " ".join(args)) if args[:1] == ["api"] else None
if match is not None:
    response = data.get("pulls", {}).get(match.group(1), [])
    if isinstance(response, str):
        print(response, file=sys.stderr)
        sys.exit(1)
    if "--jq" in args:
        print("\\n".join(str(number) for number in response))
    else:
        print(json.dumps([{"number": number} for number in response]))
    sys.exit(0)

print("unexpected forge call: gh " + " ".join(args), file=sys.stderr)
sys.exit(2)
'''


class ListItem(NamedTuple):
    """One item of a YAML block list — a workflow step, a pre-commit hook — as its own keys and its text."""

    keys: dict[str, str]
    text: str


ITEM_KEY_RE = re.compile(r"^(?:- +)?(?P<key>[A-Za-z_][\w-]*):(?:\s+(?P<value>.*))?$")


def list_items(text: str, key: str) -> list[ListItem]:
    """Every item of every `<key>:` block list in `text`, in file order, with comment lines dropped.

    Line-based, because the Python CI sets up carries no YAML parser. An item
    opens at a `- ` at the list's own indentation and runs to the next one, or
    to the first line not indented past `<key>:`. Only the item's own lines are
    read as keys, so a `run: |` body that happens to say `id:` is text.
    """
    lines = [line for line in text.splitlines() if line.strip() and not line.lstrip().startswith("#")]
    opener = re.compile(rf"^(?P<indent> *){re.escape(key)}:\s*$")
    items: list[list[str]] = []
    index = 0
    while index < len(lines):
        match = opener.match(lines[index])
        index += 1
        if match is None:
            continue
        outer = len(match.group("indent"))
        indent_of_items: int | None = None
        while index < len(lines):
            line = lines[index]
            indent = len(line) - len(line.lstrip(" "))
            opens = line.lstrip().startswith("- ") and indent >= outer and indent_of_items in (None, indent)
            if not opens and (indent <= outer or indent_of_items is None):
                break
            if opens:
                indent_of_items = indent
                items.append([])
            items[-1].append(line)
            index += 1

    found = []
    for item in items:
        own = len(item[0]) - len(item[0].lstrip(" "))
        keys: dict[str, str] = {}
        for line in item:
            if len(line) - len(line.lstrip(" ")) not in (own, own + 2):
                continue
            match = ITEM_KEY_RE.match(line.strip())
            if match is not None:
                keys.setdefault(match.group("key"), (match.group("value") or "").strip())
        found.append(ListItem(keys, "\n".join(item)))
    return found


def load_script(name: str):
    """Load a `scripts/` module by path, for the few cases that read a function directly."""
    path = REPOSITORY_ROOT / "scripts" / name
    spec = importlib.util.spec_from_file_location(name.removesuffix(".py"), path)
    module = importlib.util.module_from_spec(spec)
    assert spec is not None and spec.loader is not None
    spec.loader.exec_module(module)
    return module


def changelog_text(unreleased: str) -> str:
    """A changelog shaped like this repository's: Unreleased, one release, the rest rotated out."""
    body = unreleased.rstrip("\n")
    return (
        "# Changelog\n"
        "\n"
        "## Unreleased\n"
        "\n"
        f"{body}\n"
        "\n"
        "## 2. [0.1.0] — 2026-01-01\n"
        "\n"
        "The first release.\n"
        "\n"
        "### Added\n"
        "\n"
        "- **The beginning.** It shipped before the gate existed and carries no\n"
        "  number at all.\n"
        "\n"
        "## 3. Older releases\n"
        "\n"
        "- [0.0.1](changelog/0.0.1.md) — 2025-12-01: the first release.\n"
    )


def entries_readme() -> str:
    """The directory's README: grund's part one unchanged, and a part two of the fixture's own."""
    return PORTABLE.read_text(encoding="utf-8") + "## Part two: how this fixture uses it\n"


class EntryRepo:
    """A contributor's checkout: a base commit on `main`, and a branch to add entries on.

    `unreleased=None` is a tree after the switch-over — the pointer under
    `## Unreleased` and the entry directory with its README. Any other text is
    a tree before it, with that text as the shared section and no directory.
    Without `with_base` there is no `main` and no remote at all, which is the
    fork clone that has never fetched this repository and the case the local
    gate has to degrade for.
    """

    def __init__(self, unreleased: str | None = None, *, with_base: bool = True) -> None:
        self.root = Path(tempfile.mkdtemp(prefix="ephor-changelog-")).resolve()
        self.path = self.root / "checkout"
        self.path.mkdir()
        self.forge_data = self.root / "forge.json"
        self.forge_log = self.root / "forge.log"
        self.forge_log.touch()
        stub = self.root / "bin" / "gh"
        stub.parent.mkdir()
        stub.write_text(f"#!{sys.executable}\n{GH_STUB}", encoding="utf-8")
        stub.chmod(0o755)
        self.env = {
            **os.environ,
            **GIT_ENV,
            # Set on every CI leg, and read by the gate when no `--pr-number` is
            # given: blanking it keeps the suite's own pull request out of a case.
            "GITHUB_EVENT_PATH": "",
            "PATH": f"{stub.parent}{os.pathsep}{os.environ.get('PATH', '')}",
            "CHANGELOG_FORGE_DATA": str(self.forge_data),
            "CHANGELOG_FORGE_LOG": str(self.forge_log),
        }
        self.forge()

        self.git("init", "-q", "-b", "main" if with_base else "contribution")
        self.changelog = self.write("docs/changelog.md", changelog_text(POINTER if unreleased is None else unreleased))
        if unreleased is None:
            self.write(f"{ENTRY_DIR}/README.md", entries_readme())
        self.base = self.commit("changelog: the tree before the contribution")
        if with_base:
            self.git("checkout", "-q", "-b", "contribution")

    def forge(self, pulls: dict[str, object] | None = None, branch_pr: int | None = None) -> None:
        """What the forge answers from now on: `pulls` by commit, and the branch's own pull request."""
        self.forge_data.write_text(json.dumps({"pulls": pulls or {}, "branch_pr": branch_pr}), encoding="utf-8")

    def git(self, *args: str) -> str:
        result = subprocess.run(["git", *args], cwd=self.path, env=self.env, capture_output=True, text=True)
        if result.returncode != 0:
            raise AssertionError(f"fixture: git {' '.join(args)} failed: {result.stderr.strip()}")
        return result.stdout.strip()

    def commit(self, message: str) -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD")

    def write(self, relative: str, text: str) -> Path:
        target = self.path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        return target

    def entry(self, name: str, body: str = "- **A user-visible change.**\n") -> Path:
        return self.write(f"{ENTRY_DIR}/{name}", body)

    def read_entry(self, name: str) -> str:
        return (self.path / ENTRY_DIR / name).read_text(encoding="utf-8")

    def run_script(self, script: str, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(REPOSITORY_ROOT / "scripts" / script), *args],
            cwd=self.path,
            env=self.env,
            capture_output=True,
            text=True,
        )

    def gate(self, *args: str) -> subprocess.CompletedProcess:
        return self.run_script("check_changelog_pr_entry.py", *args)

    def release(self, *args: str) -> subprocess.CompletedProcess:
        return self.run_script("prepare_changelog_release.py", *args)

    def pending(self) -> list[str]:
        """The entry files waiting for a release, by name; the README is not one."""
        directory = self.path / ENTRY_DIR
        return sorted(p.name for p in directory.iterdir() if p.name != "README.md") if directory.is_dir() else []

    def snapshot(self) -> dict[str, bytes]:
        """Every file under `docs/`, so a refusal can be shown to have written nothing."""
        return {str(p.relative_to(self.path)): p.read_bytes() for p in (self.path / "docs").rglob("*") if p.is_file()}

    def cleanup(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


def output(result: subprocess.CompletedProcess) -> str:
    return result.stdout + result.stderr


def describe(result: subprocess.CompletedProcess) -> str:
    return f"exit={result.returncode}\n--- stdout ---\n{result.stdout}--- stderr ---\n{result.stderr}"


def release_section(text: str, version: str = "0.1.1") -> str:
    """The body of the numbered release `version`, up to the next top-level heading."""
    heading = re.search(rf"^## [0-9]+\. \[{re.escape(version)}\] — [0-9-]+\n", text, re.MULTILINE)
    assert heading is not None, f"no release section for {version}:\n{text}"
    rest = text[heading.end() :]
    following = re.search(r"^## ", rest, re.MULTILINE)
    return rest[: following.start()] if following else rest


def unreleased_section(text: str) -> str:
    return text.split("## Unreleased\n", 1)[1].split("\n## ", 1)[0]


class EntryRepoCase(unittest.TestCase):
    """Assertions on a script's exit and words, shared by every changelog test."""

    def repo(self, unreleased: str | None = None, **kwargs) -> EntryRepo:
        repo = EntryRepo(unreleased, **kwargs)
        self.addCleanup(repo.cleanup)
        return repo

    def assert_exit(self, result: subprocess.CompletedProcess, expected: int) -> None:
        self.assertEqual(result.returncode, expected, describe(result))

    def assert_refused(self, result: subprocess.CompletedProcess, *details: str) -> None:
        self.assert_exit(result, 1)
        for detail in details:
            self.assertIn(detail, output(result), describe(result))

    def prepare(self, repo: EntryRepo, version: str = "0.1.1") -> str:
        """Prepare a release that is expected to succeed, and return the changelog it left."""
        self.assert_exit(repo.release("prepare", version, "--date", "2026-10-02"), 0)
        return repo.changelog.read_text(encoding="utf-8")

