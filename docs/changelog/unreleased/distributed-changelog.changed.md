- **A pending changelog entry is a file of its own.** A pull request no longer
  adds its bullet under `## Unreleased` in `docs/changelog.md`, where any two
  open pull requests edited the same lines and the second to land conflicted.
  It adds one file, `docs/changelog/unreleased/<slug>.<category>.md`, holding
  that bullet in the format grund uses
  ([§FS-002-release.1.1](../../functional-spec/FS-002-release.md#11-one-file-per-pending-change-in-grunds-format)), so two pull requests that each
  record a change merge or rebase in either order. The pre-push hook and CI ask
  for an entry whose slug the base did not have, and refuse one that is
  malformed, renames an entry already there, or names another pull request
  ([§FS-002-release.6](../../functional-spec/FS-002-release.md#6-the-changelog-gate-runs-before-the-pull-request-exists)). The release stamps each entry's number from
  the commit that added it, collects the entries under their categories in the
  order they landed, and deletes the files
  ([§FS-002-release.2](../../functional-spec/FS-002-release.md#2-cutting-a-release)). What was pending under the shared section moved
  into `legacy-NNNN` entries, each with every number it already carried. (PR #153)
