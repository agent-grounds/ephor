"""Recognizing an entry the switch-over moved out of the shared section. §FS-002-release.1.2

The change that introduced the entry directory copied every bullet pending
under `## Unreleased` into an entry of its own. A copy is not a new change, and
nothing a contributor writes marks it as one: it is recognized by its category
and its complete bullet matching a bullet the shared section held, allowing
only for the links it moved with and the number the old `stamp` wrote before the
move. The release never stamps one with the switch-over's number
(§FS-002-release.2.2). After the switch-over the shared section holds no bullet,
so nothing written later is recognized as a copy.
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path
from typing import Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_unreleased import (  # noqa: E402
    ENTRY_PARTS,
    PLACEHOLDER_RE,
    entry_lines,
    legacy_bullets,
    pr_numbers,
    rebase_links,
)


# What the old `stamp` took for a number already written, and so left alone.
OLD_NUMBER_WRITTEN_RE = re.compile(r"(?i)\bPR\s*#\s*[0-9]+\b|\bpull request\s*#\s*[0-9]+\b|/pull/[0-9]+")

Comparable = tuple[str, ...]


class SharedSection:
    """The bullets a shared `## Unreleased` held, by category, to recognize copies against. §FS-002-release.1.2"""

    def __init__(self, changelog_text: str | None) -> None:
        self.bullets: Counter[tuple[str, Comparable]] = Counter()
        for bullet in legacy_bullets(changelog_text or ""):
            self.bullets[(bullet.category, comparable(bullet.lines, ()))] += 1

    def __bool__(self) -> bool:
        return bool(self.bullets)

    def match(self, category: str, entry_text: str) -> tuple[str, Comparable] | None:
        """The bullet `entry_text`, filed under `category`, is a copy of, or `None`."""
        entry = comparable(entry_lines(entry_text), ENTRY_PARTS)
        if self.bullets[(category, entry)] > 0:
            return category, entry
        for number in set(pr_numbers("\n".join(entry))):
            for (held_category, held), count in self.bullets.items():
                if count and held_category == category and old_stamped(held, number) == entry:
                    return held_category, held
        return None

    def take(self, category: str, entry_text: str) -> bool:
        """Whether the entry is a copy, consuming the bullet it copies so two entries cannot copy one."""
        found = self.match(category, entry_text)
        if found is None:
            return False
        self.bullets[found] -= 1
        return True


def comparable(lines: Sequence[str], source: Sequence[str]) -> Comparable:
    """A bullet's lines with every link re-expressed from `docs/`, so where it was written does not count."""
    return tuple(rebase_links(line, source, ()) for line in lines)


def old_stamped(lines: Sequence[str], number: int) -> Comparable | None:
    """The bullet as the old `stamp` left it given `number`, or `None` where it wrote nothing. §FS-002-release.1.2

    The old stamper read a bullet only to its first blank line. It left one
    whose first paragraph already named a number, replaced the first
    `PR #TBD` there, and otherwise appended ` (PR #N)` to that paragraph's last
    line — which in a bullet of several paragraphs is not the bullet's last.
    """
    end = next((index for index, line in enumerate(lines) if not line.strip()), len(lines))
    first, rest = list(lines[:end]), list(lines[end:])
    if not first or OLD_NUMBER_WRITTEN_RE.search("\n".join(first)):
        return None
    for index, line in enumerate(first):
        if PLACEHOLDER_RE.search(line):
            first[index] = PLACEHOLDER_RE.sub(f"PR #{number}", line, count=1)
            return tuple(first + rest)
    first[-1] = f"{first[-1]} (PR #{number})"
    return tuple(first + rest)
