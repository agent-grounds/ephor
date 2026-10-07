# Changelog

Records every notable change to `ephor`. Versions follow semver
([§FS-002-release](functional-spec/FS-002-release.md#fs-002-release-ephor-releases-from-a-tag-and-the-release-writes-its-changelog-from-the-pull-requests-it-ships));
the **latest release is inline** in this file, and **older releases live
one-per-file under `docs/changelog/`** so a reader — human or agent — only
loads the history they ask for.

## Unreleased

- Refuse non-string forge command overrides before invocation and explain literal executable strings and wrappers for fixed arguments (PR #187)

## 1. Conventions

### 1.1 The release writes its own section

Nobody writes this file before a release, by hand or by agent, and no change
edits it. A release lists the pull requests merged on `main` since the previous
`vX.Y.Z` tag — every one before the first release — that touched more than
documentation and CI, one line each, newest first:
`- [<title>](<url>) (PR #N)`. The title is the pull request's own, so the
title a change merges with is its line in the release
([§FS-002-release.1.3](functional-spec/FS-002-release.md#13-the-release-lists-the-pull-requests-merged-since-the-previous-tag-and-every-one-of-them)).

### 1.2 Compatibility notices

From the second release on, a published schema that lost or changed a field
since the previous tag is noticed under `### Compatibility notices`, after the
list: a removed property by its JSON Pointer, a changed constraint with its old
and new value and the commit that changed it
([§FS-002-release.1.4](functional-spec/FS-002-release.md#14-compatibility-notices-from-the-previous-tag-onwards)).

### 1.3 Progressive discovery

Only the most recent release is inline. When a new release ships, the previous
one moves whole to `docs/changelog/<version>.md`, and a one-line link is added
under [§3 Older releases](#3-older-releases).

## 3. Older releases

_None yet._
