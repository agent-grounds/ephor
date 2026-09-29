"""How `## Unreleased` is read: where the section ends, and where its bullets do. §FS-002-release.6

The gate and the release stamper are the two ends of one mechanism — one asks a
pull request for a bullet under `## Unreleased` (§FS-002-release.6), the other
writes onto that same bullet the number its author could not know
(§FS-002-release.2). They agree only if they cut the section and its bullets
identically, so the cut is written once here rather than twice by hand.

Both callers import this by adding their own directory to `sys.path` from
`__file__`, which is set whether the script is run as `python scripts/...` or
loaded by path from the test harness.

Nothing here raises: `unreleased_range` reports a missing section by returning
`None`, so each caller keeps its own error vocabulary and its own message.

The lines may carry their line endings or not — the gate splits without them and
the stamper with them — so every pattern here tolerates a trailing newline.
"""

from __future__ import annotations

import re
from typing import Sequence


UNRELEASED_RE = re.compile(r"^## Unreleased\s*$")
TOP_LEVEL_RE = re.compile(r"^##(?!#)\s+")
BULLET_RE = re.compile(r"^\s*-\s")
CONTINUATION_RE = re.compile(r"^\s+\S")


def unreleased_range(lines: Sequence[str]) -> tuple[int, int] | None:
    """The body of `## Unreleased` as a half-open index range, or `None` if there is no section."""
    start = None
    for index, line in enumerate(lines):
        if UNRELEASED_RE.match(line):
            start = index + 1
            break
    if start is None:
        return None

    end = len(lines)
    for index in range(start, len(lines)):
        if TOP_LEVEL_RE.match(lines[index]):
            end = index
            break
    return start, end


def bullet_blocks(lines: Sequence[str], start: int, end: int) -> list[tuple[int, int]]:
    """Each bullet in `lines[start:end]` as a half-open range: its `- ` line and what continues it."""
    blocks = []
    index = start
    while index < end:
        if not BULLET_RE.match(lines[index]):
            index += 1
            continue
        stop = index + 1
        while stop < end and CONTINUATION_RE.match(lines[stop]) and not BULLET_RE.match(lines[stop]):
            stop += 1
        blocks.append((index, stop))
        index = stop
    return blocks
