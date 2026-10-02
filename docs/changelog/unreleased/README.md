# Unreleased changelog entries

Every change that has merged and is not yet released is one file in this directory. Two pull requests that each record a change add two files, so they merge in either order without touching a line in common. The release collects the files into the changelog and deletes them. This README stays.

## Part one: the format

This part is shared. A repository that takes the format copies everything above part two byte for byte and writes its own part two.

1. **One file per pending change**, in `docs/changelog/unreleased/`. This `README.md` describes the format and is not an entry.
2. **The file name** is `<slug>.<category>.md`, or `<slug>.md` in a changelog without sections.
   - The slug is lowercase letters, digits and hyphens. It is unique in the directory and stays with the entry for life. The branch name is a good default.
   - The category is one lowercase word. It is published under the same word capitalized, so `fixed` becomes `### Fixed`.
3. **One bullet per file**, exactly as published. It starts with `- `. It may wrap, with continuation lines indented two spaces.
4. **Links** are relative to the file, as a formatter writes them. The release rewrites them for where the entry lands.
5. **The number.** An entry may end with `(PR #N)` naming its own pull request, with `(PR #TBD)`, or with neither. A release writes the number at the end of the entry and nowhere else.
6. **The release** groups entries by category and puts the oldest-landed first, breaking ties by file name. It then deletes the files.

## Part two: how this repository uses it

The rules are [§FS-002-release.1.1](../../functional-spec/FS-002-release.md#11-one-file-per-pending-change-in-grunds-format), and a pull request that adds no entry of its own is refused before the push and again in CI ([§FS-002-release.6](../../functional-spec/FS-002-release.md#6-the-changelog-gate-runs-before-the-pull-request-exists)).

- **The category is required** and is one of `added`, `changed`, `deprecated`, `removed`, `fixed`, `security` or `note`. Sections are released in that order, each under the same word capitalized, and a category with no entry is omitted. `note` is this repository's own: its changelog has always carried a `### Note` section.
- **The slug carries the entry's lifetime.** The release orders an entry, and stamps its number, by the commit on `main` that added its slug ([§FS-002-release.2.1](../../functional-spec/FS-002-release.md#21-an-entry-is-ordered-and-attributed-by-where-it-landed)). Rewording an entry or moving it to another category keeps both; changing its slug is refused. Editing an entry that is already here is not a pull request's own entry.
- **A bullet may run to several paragraphs.** A blank line followed by more text indented two spaces continues the same bullet, as Markdown reads it. Here this repository reads part one more widely than grund does: bullets this changelog already held run to several paragraphs, and an entry moved from it says what it said.
- **The number** is optional and, when written, is this pull request's own. The release stamps `(PR #N)` at the end of an entry from the commit that added it, replacing a trailing `(PR #TBD)`; a number already written at the end is left alone, and one in the prose is prose ([§FS-002-release.2.2](../../functional-spec/FS-002-release.md#22-the-release-stamps-only-the-trailing-number-and-stamping-never-fails-it)). Where the forge cannot name exactly one pull request, the entry is released as written and the release log says why.

For a branch named `fix/refresh-unreachable` that fixes a bug, the entry is `docs/changelog/unreleased/fix-refresh-unreachable.fixed.md`:

```markdown
- **`ephor refresh` keeps a project whose remote is unreachable.** It stays in
  the feed, marked stale, instead of disappearing from it.
```

### The switch-over

The entries named `legacy-NNNN.<category>.md` were moved here, once, from the shared `## Unreleased` section that held every pending change before this directory did ([§FS-002-release.1.2](../../functional-spec/FS-002-release.md#12-the-switch-over-moves-what-was-pending-and-loses-nothing)). Each keeps its category, its whole text and every number it carried, with its links rewritten to resolve from here, and the numbers in the names keep the order the section had. One that still carries no number is released once without one — never with the number of the change that moved it. A moved entry is recognized by matching the section it came from, not by its name, so a new entry gains nothing by being named `legacy-…`.
