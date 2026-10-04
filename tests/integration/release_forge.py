"""A throwaway repository and a forge stand-in, for the release that lists pull requests.

Not a test module — `unittest discover -p 'test_*.py'` does not collect it. It is
shared by every release test, because the release reads two things a loose file
cannot fake: the first-parent history its range is drawn from, and the pull
requests the forge says landed on it (§FS-002-release.1.3).

The script runs as the workflows run it — by argv, in the checkout, in a process
of its own — so nothing here assumes how it is written inside. The forge is the
one thing faked: `gh` on `PATH` is a stand-in that answers `gh api` the way
GitHub's REST API does, from a JSON file the test writes, and logs every call.
It serves `repos/O/R`, the pull request listing and a single pull request, a
pull request's files, the pull requests of a commit, and `search/issues`, with
GitHub's page sizes (30 by default, 100 at most) and its 3,000-file cap. It
takes `--paginate` (pages printed one after another, as `gh` prints them),
`--slurp`, `--include`, `-X`, `-H`, and `-f`/`-F` parameters. Anything else —
GraphQL, `--jq`, a call it does not know — fails loudly with exit 2, so a call
the tests did not expect shows up as the refusal it causes.

Every path handed out is resolved first, because on macOS `TMPDIR` is a symlink
and git reports the resolved path. Scratch repositories go wherever `TMPDIR` says.
"""

from __future__ import annotations

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
SCRIPT = REPOSITORY_ROOT / "scripts" / "prepare_changelog_release.py"
PATHS_SCRIPT = REPOSITORY_ROOT / "scripts" / "release_paths.py"

REPO = "agent-grounds/ephor-fixture"
ORIGIN = f"https://github.com/{REPO}.git"
PULL_URL = f"https://github.com/{REPO}/pull/{{number}}"
COMMIT_URL = f"https://github.com/{REPO}/commit/{{sha}}"

# A repository of our own, told nothing by the machine it runs on.
GIT_ENV = {
    "GIT_AUTHOR_NAME": "Release Fixture",
    "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
    "GIT_COMMITTER_NAME": "Release Fixture",
    "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
    "GIT_AUTHOR_DATE": "2026-01-02T00:00:00+00:00",
    "GIT_COMMITTER_DATE": "2026-01-02T00:00:00+00:00",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_SYSTEM": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_TERMINAL_PROMPT": "0",
}
# What the machine running the tests may carry and the release must not read.
SCRUBBED = ("GH_TOKEN", "GITHUB_TOKEN", "GH_HOST", "GH_REPO", "GH_ENTERPRISE_TOKEN", "GITHUB_REPOSITORY")

# A changelog before its first release: the conventions, an empty slot, no older release.
FIRST_CHANGELOG = (
    "# Changelog\n"
    "\n"
    "Records every notable change to the fixture.\n"
    "\n"
    "## 1. Conventions\n"
    "\n"
    "Each release lists the pull requests it ships, newest first.\n"
    "\n"
    "## 3. Older releases\n"
    "\n"
    "_None yet._\n"
)

GH_STUB = r'''
import json, os, re, sys
from pathlib import Path
from urllib.parse import parse_qsl, urlsplit

args = sys.argv[1:]
with open(os.environ["RELEASE_FORGE_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps(args) + "\n")
data = json.loads(Path(os.environ["RELEASE_FORGE_DATA"]).read_text(encoding="utf-8"))

def die(message, code=2):
    print(message, file=sys.stderr)
    sys.exit(code)

if args[:1] != ["api"]:
    die("unexpected forge call: gh " + " ".join(args))
endpoint, params, paginate, slurp, include = None, {}, False, False, False
rest = args[1:]
while rest:
    arg = rest.pop(0)
    if arg == "--paginate":
        paginate = True
    elif arg == "--slurp":
        slurp = True
    elif arg in ("-i", "--include"):
        include = True
    elif arg in ("-X", "--method"):
        if rest.pop(0).upper() != "GET":
            die("unexpected forge call: a write: gh " + " ".join(args))
    elif arg in ("-H", "--header", "--hostname"):
        rest.pop(0)
    elif arg in ("-f", "-F", "--field", "--raw-field"):
        key, _, value = rest.pop(0).partition("=")
        params[key] = value
    elif arg.startswith(("--method=", "--header=", "--hostname=")):
        pass
    elif arg.startswith("-"):
        die("unexpected forge call: unsupported flag " + arg + ": gh " + " ".join(args))
    elif endpoint is None:
        endpoint = arg
    else:
        die("unexpected forge call: gh " + " ".join(args))
if endpoint is None or endpoint == "graphql":
    die("unexpected forge call: gh " + " ".join(args))
split = urlsplit(endpoint)
path = split.path.lstrip("/")
params = {**dict(parse_qsl(split.query)), **params}
for pattern, message in data.get("fail", {}).items():
    if re.search(pattern, path):
        die("gh: " + message, 1)

repo = data["repo"]
pulls = data["pulls"]
by_number = {pull["number"]: pull for pull in pulls}

def listed(pull):
    return {key: value for key, value in pull.items() if key not in ("files", "changed_files")}

def page_of(items, page, per_page):
    return items[(page - 1) * per_page : page * per_page]

def per_page_of():
    return max(1, min(int(params.get("per_page", 30)), 100))

def collection():
    """The items a listing endpoint serves, before paging, or None for a single object."""
    if path == f"repos/{repo}":
        return None, {"full_name": repo, "default_branch": data["default_branch"], "html_url": f"https://github.com/{repo}"}
    if path == f"repos/{repo}/pulls":
        state = params.get("state", "open")
        items = [p for p in pulls if state == "all" or p["state"] == state]
        if "base" in params:
            items = [p for p in items if p["base"]["ref"] == params["base"]]
        key = {"updated": lambda p: (p.get("merged_at") or "", p["number"])}.get(params.get("sort", "created"), lambda p: p["number"])
        items = sorted(items, key=key, reverse=params.get("direction", "desc") == "desc")
        cut = data.get("truncate_pulls_after")
        return [listed(p) for p in (items if cut is None else items[:cut])], None
    match = re.fullmatch(rf"repos/{re.escape(repo)}/pulls/(\d+)", path)
    if match:
        pull = by_number.get(int(match.group(1)))
        if pull is None:
            die("gh: Not Found (HTTP 404)", 1)
        return None, pull and {**listed(pull), "changed_files": pull["changed_files"]}
    match = re.fullmatch(rf"repos/{re.escape(repo)}/pulls/(\d+)/files", path)
    if match:
        pull = by_number.get(int(match.group(1)))
        if pull is None:
            die("gh: Not Found (HTTP 404)", 1)
        cut = data.get("truncate_files", {}).get(str(pull["number"]), 3000)
        return pull["files"][: min(cut, 3000)], None
    match = re.fullmatch(rf"repos/{re.escape(repo)}/commits/([0-9a-f]{{40}})/pulls", path)
    if match:
        return [listed(p) for p in pulls if p.get("merge_commit_sha") == match.group(1)], None
    if path == "search/issues":
        terms = params.get("q", "").split()
        if f"repo:{repo}" not in terms or not ({"is:pr", "type:pr"} & set(terms)):
            die("unexpected forge call: a search outside this repository's pull requests: gh " + " ".join(args))
        items = list(pulls)
        if "is:merged" in terms:
            items = [p for p in items if p.get("merged_at")]
        elif "is:closed" in terms:
            items = [p for p in items if p["state"] == "closed"]
        for term in terms:
            if term.startswith("base:"):
                items = [p for p in items if p["base"]["ref"] == term[5:]]
        items = sorted(items, key=lambda p: p["number"], reverse=True)
        total = data.get("search_total", len(items))
        cut = data.get("truncate_pulls_after")
        return ("search", total, [listed(p) for p in (items if cut is None else items[:cut])]), None
    die("unexpected forge call: gh " + " ".join(args))

items, single = collection()
if single is not None:
    print(json.dumps(single))
    sys.exit(0)
search = isinstance(items, tuple)
if search:
    _, total, items = items
per_page = per_page_of()
page = int(params.get("page", 1))
pages = []
while True:
    chunk = page_of(items, page, per_page)
    body = {"total_count": total, "incomplete_results": False, "items": chunk} if search else chunk
    more = page * per_page < len(items)
    pages.append((body, more, page))
    if not paginate or not more:
        break
    page += 1
if slurp:
    print(json.dumps([body for body, _, _ in pages]))
    sys.exit(0)
for body, more, number in pages:
    if include:
        print("HTTP/2.0 200 OK")
        if more:
            query = "&".join(f"{k}={v}" for k, v in {**params, "page": number + 1}.items())
            print(f'Link: <https://api.github.com/{path}?{query}>; rel="next"')
        print()
    sys.stdout.write(json.dumps(body))
sys.stdout.write("\n")
'''


class Landing(NamedTuple):
    """A pull request as the fixture landed it: its number and the commit it landed as."""

    number: int
    sha: str


class ReleaseRepo:
    """A checkout of `main` whose pull requests the forge stand-in knows about."""

    def __init__(self, changelog: str = FIRST_CHANGELOG, *, origin: str = ORIGIN, repository_env: bool = True) -> None:
        self.root = Path(tempfile.mkdtemp(prefix="ephor-release-")).resolve()
        self.path = self.root / "checkout"
        self.path.mkdir()
        self.forge_data = self.root / "forge.json"
        self.forge_log = self.root / "forge.log"
        self.forge_log.touch()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        stub = self.bin / "gh"
        stub.write_text(f"#!{sys.executable}\n{GH_STUB}", encoding="utf-8")
        stub.chmod(0o755)
        base = {key: value for key, value in os.environ.items() if key not in SCRUBBED}
        self.env = {
            **base,
            **GIT_ENV,
            "PATH": f"{self.bin}{os.pathsep}{os.environ.get('PATH', '')}",
            "RELEASE_FORGE_DATA": str(self.forge_data),
            "RELEASE_FORGE_LOG": str(self.forge_log),
        }
        if repository_env:
            self.env["GITHUB_REPOSITORY"] = REPO
        self.pulls: list[dict] = []
        self.forge_extra: dict = {}
        self.clock = 0

        self.git("init", "-q", "-b", "main")
        self.git("remote", "add", "origin", origin)
        self.changelog = self.write("docs/changelog.md", changelog)
        self.write("src/lib.rs", "// the fixture's first source file\n")
        self.root_commit = self.commit("chore: the first commit, pushed without a pull request")

    # --- git -----------------------------------------------------------------

    def git(self, *args: str) -> str:
        result = subprocess.run(["git", *args], cwd=self.path, env=self.env, capture_output=True, text=True)
        if result.returncode != 0:
            raise AssertionError(f"fixture: git {' '.join(args)} failed: {result.stderr.strip()}")
        return result.stdout.strip()

    def write(self, relative: str, text: str) -> Path:
        target = self.path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        return target

    def commit(self, message: str) -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def touch(self, paths, tag: str) -> list[dict]:
        """Change each path in the tree and return the forge's file entries for it.

        A path is a string, changed or added; `("rename", old, new)`, moved; or
        `("keep", path)`, already written by the test and recorded as it stands.
        """
        entries = []
        for item in paths:
            if isinstance(item, tuple) and item[0] == "keep":
                existed = self.git("ls-files", "--", item[1]) != ""
                entries.append({"filename": item[1], "status": "modified" if existed else "added"})
            elif isinstance(item, tuple):
                _, old, new = item
                if not (self.path / old).exists():
                    self.write(old, f"// {old}\n")
                    self.commit(f"chore: add {old} to rename later")
                (self.path / new).parent.mkdir(parents=True, exist_ok=True)
                self.git("mv", old, new)
                entries.append({"filename": new, "previous_filename": old, "status": "renamed"})
            else:
                existed = (self.path / item).exists()
                self.write(item, f"// {item}: {tag}\n")
                entries.append({"filename": item, "status": "modified" if existed else "added"})
        return entries

    def push(self, paths, message: str = "chore: pushed straight to main") -> str:
        """A commit on main that no pull request landed."""
        self.touch(paths, message)
        return self.commit(message)

    def tag(self, version: str, at: str = "HEAD") -> None:
        self.git("tag", f"v{version}", at)

    def sync_origin(self, branch: str = "main") -> None:
        """What a fetch leaves: `origin/<branch>` at the local branch, and `origin/HEAD` naming it."""
        self.git("update-ref", f"refs/remotes/origin/{branch}", branch)
        self.git("symbolic-ref", "refs/remotes/origin/HEAD", f"refs/remotes/origin/{branch}")

    # --- landings ------------------------------------------------------------

    def _merged_at(self, merged_at: str | None) -> str:
        if merged_at is not None:
            return merged_at
        self.clock += 1
        return f"2026-03-01T{self.clock // 3600:02d}:{self.clock // 60 % 60:02d}:{self.clock % 60:02d}Z"

    def record(self, number: int, title: str, sha: str | None, files: list[dict], *, base: str = "main",
               merged_at: str | None = None, state: str = "closed", merged: bool = True, url: str | None = None,
               changed_files: int | None = None) -> Landing:
        self.pulls.append(
            {
                "number": number,
                "title": title,
                "html_url": url or PULL_URL.format(number=number),
                "state": state,
                "merged_at": self._merged_at(merged_at) if merged else None,
                "merge_commit_sha": sha,
                "base": {"ref": base},
                "files": files,
                "changed_files": len(files) if changed_files is None else changed_files,
            }
        )
        return Landing(number, sha or "")

    def land_merge(self, number: int, title: str, paths=("src/lib.rs",), *, into: str = "main",
                   branch_from: str | None = None, **pull) -> Landing:
        """A pull request merged with a merge commit; it lands as that merge commit."""
        self.git("checkout", "-q", "-b", f"pr-{number}", branch_from or into)
        files = self.touch(paths, title)
        self.commit(title)
        self.git("checkout", "-q", into)
        self.git("merge", "-q", "--no-ff", "-m", f"Merge pull request #{number}", f"pr-{number}")
        return self.record(number, title, self.git("rev-parse", "HEAD"), files, base=into, **pull)

    def land_squash(self, number: int, title: str, paths=("src/lib.rs",), **pull) -> Landing:
        """A pull request squashed into one commit on main; it lands as that commit."""
        files = self.touch(paths, title)
        return self.record(number, title, self.commit(f"{title} (#{number})"), files, **pull)

    def land_rebase(self, number: int, title: str, paths=("src/lib.rs", "src/more.rs"), **pull) -> Landing:
        """A pull request rebased onto main, one commit per path; it lands as the last one."""
        files = []
        for index, item in enumerate(paths):
            files += self.touch([item], f"{title} {index}")
            sha = self.commit(f"{title}, part {index + 1}")
        return self.record(number, title, sha, files, **pull)

    # --- the forge and the script --------------------------------------------

    def write_forge(self) -> None:
        self.forge_data.write_text(
            json.dumps({"repo": REPO, "default_branch": "main", "pulls": self.pulls, **self.forge_extra}),
            encoding="utf-8",
        )

    def run(self, script: Path, *args: str, stdin: str | None = None, env: dict | None = None) -> subprocess.CompletedProcess:
        self.write_forge()
        return subprocess.run(
            [sys.executable, str(script), *args],
            cwd=self.path,
            env=env or self.env,
            input=stdin,
            capture_output=True,
            text=True,
        )

    def release(self, *args: str, env: dict | None = None) -> subprocess.CompletedProcess:
        return self.run(SCRIPT, *args, env=env)

    def without_gh(self) -> dict:
        """The environment with no `gh` anywhere on `PATH`: only `git` is reachable."""
        bare = self.root / "bare-bin"
        bare.mkdir(exist_ok=True)
        git = shutil.which("git")
        assert git is not None, "harness: git is not on PATH"
        if not (bare / "git").exists():
            (bare / "git").symlink_to(git)
        return {**self.env, "PATH": str(bare)}

    def forge_calls(self) -> list[list[str]]:
        return [json.loads(line) for line in self.forge_log.read_text(encoding="utf-8").splitlines() if line]

    def snapshot(self) -> dict[str, bytes]:
        """Every file in the working tree, so a refusal can be shown to have written nothing."""
        return {
            str(p.relative_to(self.path)): p.read_bytes()
            for p in self.path.rglob("*")
            if p.is_file() and ".git" not in p.relative_to(self.path).parts
        }

    def cleanup(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


def output(result: subprocess.CompletedProcess) -> str:
    return result.stdout + result.stderr


def describe(result: subprocess.CompletedProcess) -> str:
    return f"exit={result.returncode}\n--- stdout ---\n{result.stdout}--- stderr ---\n{result.stderr}"


def line(number: int, title: str) -> str:
    """One release line as §FS-002-release.1.3 writes it, for a title that needs no escaping."""
    return f"- [{title}]({PULL_URL.format(number=number)}) (PR #{number})"


def release_section(text: str, version: str) -> str:
    """The body of the numbered release `version`, up to the next top-level heading."""
    heading = re.search(rf"^## [0-9]+\. \[{re.escape(version)}\] — [0-9-]+\n", text, re.MULTILINE)
    assert heading is not None, f"no release section for {version}:\n{text}"
    rest = text[heading.end() :]
    following = re.search(r"^## ", rest, re.MULTILINE)
    return rest[: following.start()] if following else rest


def listed_numbers(section: str) -> list[int]:
    """The pull request numbers of a release's lines, in the order they are written."""
    return [int(n) for n in re.findall(r"^- \[.*\]\(\S+\) \(PR #(\d+)\)$", section, re.MULTILINE)]


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


class ReleaseCase(unittest.TestCase):
    """Assertions on the release script's exit and words, shared by every release test."""

    DATE = "2026-10-04"

    def repo(self, *args, **kwargs) -> ReleaseRepo:
        repo = ReleaseRepo(*args, **kwargs)
        self.addCleanup(repo.cleanup)
        return repo

    def assert_exit(self, result: subprocess.CompletedProcess, expected: int) -> None:
        self.assertEqual(result.returncode, expected, describe(result))

    def prepare(self, repo: ReleaseRepo, version: str = "0.1.0", date: str | None = None) -> str:
        """Prepare a release that is expected to succeed, and return the changelog it left."""
        repo.sync_origin()
        self.assert_exit(repo.release("prepare", version, "--date", date or self.DATE), 0)
        return repo.changelog.read_text(encoding="utf-8")

    def assert_refused_untouched(self, repo: ReleaseRepo, version: str, *details: str, env: dict | None = None) -> None:
        """`prepare` exits non-zero, says why, and leaves every file as it was. §FS-002-release.2.3"""
        before = repo.snapshot()
        result = repo.release("prepare", version, "--date", self.DATE, env=env)
        self.assertNotEqual(result.returncode, 0, describe(result))
        for detail in details:
            self.assertIn(detail.lower(), output(result).lower(), describe(result))
        self.assertEqual(repo.snapshot(), before, "a refused release wrote to the tree:\n" + describe(result))

    def assert_refused_until(self, repo: ReleaseRepo, version: str, undo, *details: str, env: dict | None = None,
                             then: str | None = None) -> None:
        """Refused, untouched, for the reason the case sets up: once `undo` removes it, the release goes through.

        The second half is what makes the refusal specific. A script that refuses
        everything — one that cannot release without hand-written entries, say —
        passes the first half and fails here.
        """
        self.assert_refused_untouched(repo, version, *details, env=env)
        undo()
        self.prepare(repo, then or version)

    def released(self, repo: ReleaseRepo, version: str = "0.1.0", date: str | None = None) -> list[int]:
        """Prepare `version` and return the pull request numbers its section lists, in order."""
        return listed_numbers(release_section(self.prepare(repo, version, date), version))
