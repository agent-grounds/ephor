"""Compatibility notices: what a published schema lost or changed since the previous tag. §FS-002-release.1.4

Every `assets/*.schema.json` is compared step by step along the range's
first-parent history, from the previous tag through each commit to `HEAD`, so a
field removed and put back before the release is still noticed. Only what is
provably compatible goes unnoticed — a new optional property, a new definition,
and prose (`title`, `description`, `$comment`, `examples`); a removal is noticed
by its JSON Pointer, a changed value with the old and the new and the commit
that changed it, and any other addition as potentially incompatible. A rename
is a removal and an addition, and nothing guesses which two belong together.

This is how a field renamed or removed from the machine form still reaches the
release that ships it (§REQ-002-parity.4).
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Iterator

from release_pulls import Range, ReleaseRefused, git


SCHEMA_RE = re.compile(r"^assets/[^/]+\.schema\.json$")
VERSION_MARKER_RE = re.compile(r"/(v[0-9]+)$")
PROSE = frozenset({"title", "description", "$comment", "examples"})
# Maps whose keys are names a schema declares rather than keywords.
NAME_MAPS = frozenset({"properties", "patternProperties", "$defs", "definitions", "dependentSchemas"})
# Name maps where a new name constrains nothing that was valid before.
FREE_ADDITIONS = frozenset({"$defs", "definitions"})


def compatibility_notices(root: Path, repo: str, span: Range) -> list[str]:
    """One bullet per notice, oldest change first; none on a first release. §FS-002-release.1.4"""
    if span.tag_commit is None:
        return []
    notices: list[str] = []
    previous = span.tag_commit
    for commit in reversed(span.commits):
        changed = git(root, "diff", "--name-only", "--no-renames", previous, commit, "--", "assets/").splitlines()
        for path in sorted(p for p in changed if SCHEMA_RE.match(p)):
            old, new = _schema_at(root, previous, path), _schema_at(root, commit, path)
            if old is None:
                continue
            label = _version_label(new if new is not None else old)
            link = f"[{commit[:12]}](https://github.com/{repo}/commit/{commit})"
            if new is None:
                found = [f"the schema was removed in {link}"]
            else:
                found = [_sentence(change, link) for change in _changes(old, new, "", False, None)]
            for text in found:
                notice = f"- `{path}` ({label}): {text}."
                if notice not in notices:
                    notices.append(notice)
        previous = commit
    return notices


def _schema_at(root: Path, commit: str, path: str) -> Any:
    if not git(root, "ls-tree", "--name-only", commit, "--", path).strip():
        return None
    text = git(root, "show", f"{commit}:{path}")
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise ReleaseRefused(f"{path} at {commit[:12]} is not JSON, so it cannot be compared: {exc}") from exc


def _version_label(schema: Any) -> str:
    marker = VERSION_MARKER_RE.search(schema.get("$id", "")) if isinstance(schema, dict) else None
    return marker.group(1) if marker else "unversioned"


def _changes(old: Any, new: Any, pointer: str, names: bool, required: set[str] | None) -> Iterator[tuple]:
    """(kind, pointer, old, new) for every difference a caller could trip on."""
    if not (isinstance(old, dict) and isinstance(new, dict)):
        if old != new:
            yield ("changed", pointer, old, new)
        return
    map_name = pointer.rsplit("/", 1)[-1] if names else None
    for key in old:
        if not names and key in PROSE:
            continue
        child = f"{pointer}/{_escape(key)}"
        if key not in new:
            yield ("removed", child, old[key], None)
        elif not names and key in NAME_MAPS:
            inner = set(new.get("required", [])) if key == "properties" else None
            yield from _changes(old[key], new[key], child, True, inner)
        else:
            yield from _changes(old[key], new[key], child, False, None)
    for key in new:
        if key in old or (not names and key in PROSE):
            continue
        if names and (map_name in FREE_ADDITIONS or (map_name == "properties" and key not in (required or set()))):
            continue
        yield ("added", f"{pointer}/{_escape(key)}", None, new[key])


def _sentence(change: tuple, link: str) -> str:
    kind, pointer, old, new = change
    where = _code(pointer or "/")
    if kind == "removed":
        return f"{where} was removed in {link}"
    if kind == "added":
        return f"{where} was added as {_code(_json(new))}, potentially incompatible, in {link}"
    return f"{where} changed from {_code(_json(old))} to {_code(_json(new))} in {link}"


def _escape(key: str) -> str:
    """A key as one JSON Pointer reference token (RFC 6901)."""
    return key.replace("~", "~0").replace("/", "~1")


def _json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(", ", ": "))


def _code(text: str) -> str:
    """`text` as inline code, fenced past any backtick run it holds."""
    fence = "`" * (max((len(run) for run in re.findall(r"`+", text)), default=0) + 1)
    pad = " " if text.startswith("`") or text.endswith("`") else ""
    return f"{fence}{pad}{text}{pad}{fence}"
