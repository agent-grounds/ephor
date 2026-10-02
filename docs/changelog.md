# Changelog

Records every notable change to `ephor`. Versions follow semver
([§FS-002-release](functional-spec/FS-002-release.md#fs-002-release-ephor-releases-from-a-tag-with-a-changelog-entry-per-change));
the **latest release is inline** in this file, and **older releases live
one-per-file under `docs/changelog/`** so a reader — human or agent — only
loads the history they ask for.

## 1. Conventions

### 1.1 Sections per release

`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security` — the
Keep-a-Changelog set — then `Note`; omit any with no entries.

### 1.2 Entry style

One bullet per change, present tense, leading with the affected area. Every
pull request adds an entry of its own: one file under
[`changelog/unreleased/`](changelog/unreleased/README.md), named
`<slug>.<category>.md` and holding that one bullet, in the format the
directory's README gives. Nothing is written under `## Unreleased` by hand. The
number is optional, because the release fills it in — end the bullet with
`(PR #12)` if you know it, `(PR #TBD)` if you want a placeholder, or nothing at
all. A number you do write must be your own pull request's
([§FS-002-release.1](functional-spec/FS-002-release.md#1-changelog)).

### 1.3 Progressive discovery

Pending changes are one file each under `docs/changelog/unreleased/`, which
`## Unreleased` points to, and only the most recent release is inline. When a
new release ships, its entries are collected inline under it and their files
deleted, the previous "latest" section moves whole to
`docs/changelog/<version>.md`, and a one-line link is added under
[§3 Older releases](#3-older-releases).

## Unreleased

Pending changes are one file each under
[`changelog/unreleased/`](changelog/unreleased/README.md), whose README says how
to write one; the next release collects them here and deletes the files.

## 2. [0.1.0] — 2026-08-11

First version. Not yet tagged or published — publication is gated on
[§RM-001-forge-interface](roadmap.md#rm-001-forge-interface-put-every-forge-behind-the-interface).

### Added

- Registry engine ported from the `automation` repo's Python `dev/projects`
  tool: `list`, `validate`, `ensure-agents`, and `update`, with the registry
  JSON Schema embedded in the binary and `required_branch_ids` replacing the
  previously hardcoded release-branch check.
- Per-project status feed with pluggable providers, cached under
  `~/.local/state/ephor/`. A failing provider keeps its last-good items marked
  `(stale)` rather than blanking the feed.
- Two-screen TUI (`ephor tui`): a navigator organized per organization, project,
  type, and branch, and a thread screen rendering a item's conversation with
  reactions. Item actions run configured commands in the item's checkout with
  the `EPHOR_*` context exported.
- Gate status on every pull request row — passed, failed, and running job
  counts, totalled across every repository the gate covers, with a per-repo
  breakdown when it spans more than one.
- `grund` tree: [§FS-001-forge-interface](functional-spec/FS-001-forge-interface.md#fs-001-forge-interface-ephor-reaches-every-forge-and-issue-tracker-through-one-provider-interface)
  and [§FS-002-release](functional-spec/FS-002-release.md#fs-002-release-ephor-releases-from-a-tag-with-a-changelog-entry-per-change),
  with [§RM-001-forge-interface](roadmap.md#rm-001-forge-interface-put-every-forge-behind-the-interface)
  sequencing the work that has to land before anything ships.
- Release pipeline: tag-triggered publication, profile-guided release binaries
  per target, a scheduled patch release and an on-demand minor release, and a
  pre-release gate that refuses to publish while the tree still carries
  site-specific configuration.

### Changed

- Renamed from `hub` to `ephor`, including the `EPHOR_*` environment contract,
  the state and secrets directories, and the systemd units.

## 3. Older releases

_None yet._
