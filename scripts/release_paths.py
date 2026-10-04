#!/usr/bin/env python3
"""Which paths a release counts as substantive. §FS-002-release.1.3

One predicate, called by both the release's pull request selection and the
schedule's gate on observable changes (§FS-002-release.2), so the two cannot
disagree about a path. Documentation, CI and the toolchain's own configuration
do not move the published binary or the crate: a path under `docs/`,
`.github/`, `.agents/`, `.agent-grounds/` or `tests/e2e/`, one ending in `.md`,
and the root `LICENSE`, `AGENTS.md`, `CLAUDE.md` and `grund.toml` are not
substantive; every other path is.

As a command it reads one path per line on standard input and prints the
substantive ones, in the order they came.
"""

from __future__ import annotations

import re
import sys
from typing import Iterable


NOT_SUBSTANTIVE_RE = re.compile(
    r"^(?:docs|\.github|\.agents|\.agent-grounds|tests/e2e)/"
    r"|\.md$"
    r"|^(?:LICENSE|AGENTS\.md|CLAUDE\.md|grund\.toml)$"
)


def is_substantive(path: str) -> bool:
    """Whether a repository-relative path is one a release ships. §FS-002-release.1.3"""
    return NOT_SUBSTANTIVE_RE.search(path) is None


def substantive(paths: Iterable[str]) -> list[str]:
    return [path for path in paths if is_substantive(path)]


def main() -> int:
    paths = [line.strip() for line in sys.stdin.read().splitlines() if line.strip()]
    for path in substantive(paths):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
