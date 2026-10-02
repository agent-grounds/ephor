"""What a pending changelog entry is, and what the shared section it replaced held. §FS-002-release.1.1

The release reads the entries in more than one step: it stamps them, collects
them and consumes them (§FS-002-release.2). The steps agree only if they read an
entry identically, so the reading is written once, here. Git is
`changelog_history`'s; recognizing what the switch-over moved is
`changelog_switchover`'s.

Every caller imports this by adding its own directory to `sys.path` from
`__file__`, which is set whether a script is run as `python scripts/...` or
loaded by path from the test harness.

Nothing here raises: a malformed entry is a problem returned as a sentence that
opens with the file name, so each caller keeps its own error vocabulary and
prefixes the directory it names.
"""

from __future__ import annotations

import posixpath
import re
from pathlib import Path
from typing import NamedTuple, Sequence


# Where the entries are, beside `changelog.md` under `docs/`, as path components.
ENTRY_PARTS = ("changelog", "unreleased")
ENTRY_README = "README.md"
# Released in this order, each under the same word capitalized; `note` is ephor's (§FS-002-release.1.1).
CATEGORIES = ("added", "changed", "deprecated", "removed", "fixed", "security", "note")

# Any lowercase word as a category, so a name with the wrong one is told so
# rather than told it is no entry at all; which categories count is `entry_problem`'s.
ENTRY_NAME_RE = re.compile(r"^(?P<slug>[a-z0-9][a-z0-9-]*)(?:\.(?P<category>[a-z]+))?\.md$")
ENTRY_BULLET_RE = re.compile(r"^- +\S")
CONTINUATION = "  "

UNRELEASED_RE = re.compile(r"^## Unreleased\s*$")
TOP_LEVEL_RE = re.compile(r"^##(?!#)\s+")
CATEGORY_HEADING_RE = re.compile(r"^###(?!#)\s+(?P<name>.+?)\s*$")
SHARED_BULLET_RE = re.compile(r"^\s*-\s")

# The three spellings a written pull request number takes, and the placeholder that is not one.
PR_NUMBER_PATTERNS = (
    re.compile(r"(?i)\bPR\s*#\s*([0-9]+)\b"),
    re.compile(r"(?i)\bpull request\s*#\s*([0-9]+)\b"),
    re.compile(r"/pull/([0-9]+)(?:\b|[/#?)])"),
)
PLACEHOLDER_RE = re.compile(r"(?i)\bPR\s*#\s*TBD\b")
# The parenthesized group an entry ends with, one level of nesting deep, so a
# `([#12](…/pull/12))` link is read whole; an optional full stop may follow it.
TRAILING_GROUP_RE = re.compile(r"\((?:[^()]|\([^()]*\))*\)(?=\.?\s*$)")

LINK_RE = re.compile(r"(?P<prefix>\]\()(?P<destination>[^)#\s][^)#\s]*)(?P<fragment>#[^)\s]*)?\)")


class EntryName(NamedTuple):
    """An entry's file name as the format reads it. §FS-002-release.1.1"""

    slug: str
    category: str | None


class LegacyBullet(NamedTuple):
    """A whole bullet under the shared `## Unreleased`, and the `###` category it sat in."""

    category: str
    lines: tuple[str, ...]


def entry_directory(changelog: Path) -> Path:
    """The entry directory that belongs to `changelog`, so `--changelog` moves it too. §FS-002-release.1"""
    return changelog.parent.joinpath(*ENTRY_PARTS)


def entry_name(name: str) -> EntryName | None:
    """`<slug>.<category>.md` or `<slug>.md`, else `None`. §FS-002-release.1.1"""
    match = ENTRY_NAME_RE.match(name)
    if match is None:
        return None
    return EntryName(match.group("slug"), match.group("category"))


def entry_problem(name: str, text: str | None) -> str | None:
    """What keeps `name`, holding `text`, from being an entry here, or `None`. §FS-002-release.1.1

    `text` is `None` for a file that cannot be read as UTF-8. The answer opens
    with the file name, so a caller prefixes the directory and has the whole
    refusal.
    """
    categories = ", ".join(CATEGORIES)
    parsed = entry_name(name)
    if parsed is None:
        return (
            f"{name}: not an entry; an entry is named `<slug>.<category>.md`, its slug lowercase "
            "letters, digits and hyphens"
        )
    if parsed.category is None:
        return f"{name}: no category; name it `{parsed.slug}.<category>.md`, <category> one of: {categories}"
    if parsed.category not in CATEGORIES:
        return f"{name}: the category `{parsed.category}` is not one of: {categories}"
    if text is None:
        return f"{name}: not UTF-8 text"
    shape = _bullet_shape_problem(entry_lines(text))
    return None if shape is None else f"{name}: not one bullet; {shape}"


def entry_lines(text: str) -> list[str]:
    """An entry's lines as the release prints them: line endings and trailing blank lines dropped."""
    lines = [line.rstrip("\r") for line in text.split("\n")]
    while lines and not lines[-1].strip():
        lines.pop()
    return lines


def _bullet_shape_problem(lines: Sequence[str]) -> str | None:
    """One `- ` line, then lines indented two spaces, with blank lines only between paragraphs. §FS-002-release.1.1"""
    if not lines:
        return "it is empty"
    if ENTRY_BULLET_RE.match(lines[0]) is None:
        return "its first line does not open a bullet with `- `"
    for number, line in enumerate(lines[1:], start=2):
        if not line.strip():
            continue  # a paragraph break; what follows it is held to the indent like any line
        if not line.startswith(CONTINUATION):
            return f"line {number} is not indented two spaces under the bullet"
    return None


def unreleased_range(lines: Sequence[str]) -> tuple[int, int] | None:
    """The body of `## Unreleased` as a half-open index range, or `None` if there is no section."""
    start = next((index + 1 for index, line in enumerate(lines) if UNRELEASED_RE.match(line)), None)
    if start is None:
        return None
    end = next((index for index in range(start, len(lines)) if TOP_LEVEL_RE.match(lines[index])), len(lines))
    return start, end


def stray_bullets(lines: Sequence[str]) -> list[int]:
    """Indexes of bullets written under the `## Unreleased` pointer, which holds none. §FS-002-release.2.3"""
    section = unreleased_range(lines)
    if section is None:
        return []
    return [index for index in range(*section) if SHARED_BULLET_RE.match(lines[index])]


def legacy_bullets(text: str) -> list[LegacyBullet]:
    """Every bullet under a shared `## Unreleased`, whole, in order, with its category. §FS-002-release.1.2

    A bullet runs on through its indented lines and, past a blank line, through
    any further indented paragraph, as Markdown reads it. The category is the
    `###` heading above it, lowercased, so a section that repeats a heading
    keeps each bullet in the one it sat in.
    """
    lines = [line.rstrip("\r") for line in text.split("\n")]
    section = unreleased_range(lines)
    if section is None:
        return []
    start, end = section
    bullets: list[LegacyBullet] = []
    category = ""
    index = start
    while index < end:
        heading = CATEGORY_HEADING_RE.match(lines[index])
        if heading is not None:
            category = heading.group("name").lower()
        if not lines[index].startswith("- "):
            index += 1
            continue
        stop = index + 1
        while stop < end:
            if lines[stop].startswith(CONTINUATION) and lines[stop].strip():
                stop += 1
                continue
            following = next((ahead for ahead in range(stop, end) if lines[ahead].strip()), end)
            if following < end and following > stop and lines[following].startswith(CONTINUATION):
                stop = following  # blank lines, then an indented paragraph of the same bullet
                continue
            break
        bullets.append(LegacyBullet(category, tuple(lines[index:stop])))
        index = stop
    return bullets


def pr_numbers(text: str) -> list[int]:
    """Every pull request number `text` writes, in any of the three spellings. §FS-002-release.2.2"""
    return [int(match) for pattern in PR_NUMBER_PATTERNS for match in pattern.findall(text)]


def trailing_number(lines: Sequence[str]) -> str | None:
    """The number an entry ends with — digits, `TBD`, or `None` for none. §FS-002-release.2.2

    Only the parenthesized group the last line ends with is read, so a number
    or a placeholder in the prose before it is prose.
    """
    group = TRAILING_GROUP_RE.search(lines[-1]) if lines else None
    if group is None:
        return None
    written = pr_numbers(group.group(0))
    if written:
        return str(written[-1])
    return "TBD" if PLACEHOLDER_RE.search(group.group(0)) else None


def stamped(lines: Sequence[str], number: int) -> list[str]:
    """The entry with `(PR #number)` written at its end and nowhere else. §FS-002-release.2.2

    A trailing `(PR #TBD)` is replaced; otherwise the number is appended to the
    last line. The caller has already left alone an entry that ends with a number.
    """
    result = list(lines)
    last = result[-1]
    group = TRAILING_GROUP_RE.search(last)
    if group is not None and PLACEHOLDER_RE.search(group.group(0)):
        replaced = PLACEHOLDER_RE.sub(f"PR #{number}", group.group(0), count=1)
        result[-1] = last[: group.start()] + replaced + last[group.end() :]
    else:
        result[-1] = f"{last.rstrip()} (PR #{number})"
    return result


def rebase_links(line: str, source: Sequence[str], target: Sequence[str]) -> str:
    """Each relative link written in `source` rewritten to reach the same target from `target`. §FS-002-release.2.3

    Both are directories as components under `docs/`: an entry is written in
    `changelog/unreleased`, the inline release is read in `docs/` itself, and an
    archive in `changelog`. An anchor, an absolute path and a URL mean the same
    thing anywhere; every relative destination — one that already climbs out
    with `../` included — is resolved against where it was written and
    re-expressed from where it lands, and its fragment goes with it.
    """

    def rewrite(match: re.Match[str]) -> str:
        destination = match.group("destination")
        if destination.startswith(("/", "<")) or re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", destination):
            return match.group(0)
        fragment = match.group("fragment") or ""
        return f"{match.group('prefix')}{_rebased(destination, source, target)}{fragment})"

    return LINK_RE.sub(rewrite, line)


def _rebased(destination: str, source: Sequence[str], target: Sequence[str]) -> str:
    # Rooted at placeholders well above `docs/`, so a link that climbs out of it keeps its `..`s.
    root = "/_/_/_/_/_/_/_/_/docs"
    resolved = posixpath.normpath(posixpath.join(root, *source, destination))
    rebased = posixpath.relpath(resolved, posixpath.join(root, *target))
    return f"{rebased}/" if destination.endswith("/") and rebased != "." else rebased
