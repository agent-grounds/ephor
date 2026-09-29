"""A throwaway git repository carrying a `docs/changelog.md`.

Not a test module — `unittest discover -p 'test_*.py'` does not collect it. It is
shared by `test_check_changelog_pr_entry.py` and `test_prepare_changelog_release.py`
because the gate and the stamper are two halves of one rule and both need a real
repository rather than a loose file: the gate diffs the changelog against a base
commit, and the stamper blames its bullets. §FS-002-release.6

Every path handed out is resolved first. The CI matrix runs on macOS, where
`TMPDIR` is a symlink, so the raw `mkdtemp()` string and the path git reports for
the same directory are two different strings; resolving once here keeps any
comparison between them honest.
"""

from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import tempfile
from contextlib import contextmanager
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

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


def load_script(name: str):
    """Load a `scripts/` module by path, the way these tests have always done."""
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
        "### Added\n"
        "\n"
        "- **The beginning.** It shipped before the gate existed and carries no\n"
        "  number at all.\n"
        "\n"
        "## 3. Older releases\n"
        "\n"
        "- [0.0.1](changelog/0.0.1.md) — 2025-12-01: the first release.\n"
    )


class ChangelogRepo:
    """A contributor's checkout: a base commit, and a branch to add a bullet on.

    With `with_origin`, the base commit is pushed to a bare `origin` so
    `origin/main` resolves — what a real clone has. Without it, the repository has
    no remote and no `main` at all, which is the fork clone that has never fetched
    this repository and the case the local gate has to degrade for.
    """

    def __init__(self, unreleased: str, *, with_origin: bool = True, branch: str = "feat/contribution") -> None:
        self.root = Path(tempfile.mkdtemp(prefix="ephor-changelog-")).resolve()
        self.path = self.root / "checkout"
        self.git("init", "-q", "-b", "main" if with_origin else branch, str(self.path), cwd=self.root)
        self.changelog = self.path / "docs" / "changelog.md"
        self.changelog.parent.mkdir(parents=True)
        self.write_unreleased(unreleased)
        self.base_sha = self.commit("changelog: the tree before the contribution")
        self.upstream: Path | None = None
        if with_origin:
            self.upstream = self.root / "upstream.git"
            self.git("init", "-q", "--bare", "-b", "main", str(self.upstream), cwd=self.root)
            self.git("remote", "add", "origin", str(self.upstream))
            self.git("push", "-q", "origin", "main")
            self.git("checkout", "-q", "-b", branch)

    def git(self, *args: str, cwd: Path | None = None) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=str(cwd if cwd is not None else self.path),
            env={**os.environ, **GIT_ENV},
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
        return result.stdout

    def write_unreleased(self, unreleased: str) -> None:
        self.changelog.write_text(changelog_text(unreleased), encoding="utf-8")

    def write_file(self, relative: str, text: str) -> None:
        target = self.path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def commit(self, message: str) -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD").strip()

    def advance_main(self, unreleased: str, message: str) -> str:
        """Move main and `origin/main` on, the way a merge does while a branch is open."""
        branch = self.git("rev-parse", "--abbrev-ref", "HEAD").strip()
        self.git("checkout", "-q", "main")
        self.write_unreleased(unreleased)
        sha = self.commit(message)
        self.git("push", "-q", "origin", "main")
        self.git("checkout", "-q", branch)
        return sha

    def read(self) -> str:
        return self.changelog.read_text(encoding="utf-8")

    def cleanup(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


@contextmanager
def working_directory(path: Path):
    previous = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(previous)
