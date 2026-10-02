- **A branch you had checked out was still "not linked to a branch"**
  ([§FS-008-attribution.2](../../functional-spec/FS-008-attribution.md#2-two-stages-one-engine)).
  The inbox measured whether an item's workspace was on disk by expanding the
  project's template and looking, and then grouped that same item under the
  branches the registry row happened to name — two answers to one question,
  from two sources. So a row could read `✓` for "checked out" while sitting
  under the heading that says it belongs to no branch, and a workspace
  `ephor checkout` had just made stayed invisible to the grouping the moment
  after it was made, because nothing writes a branch back into the row. On a
  project with fourteen trees on disk and three branches written down, eleven
  branches' worth of pull requests fell into one undifferentiated pile. A
  project's branches are now the row's plus every workspace found under the
  workspace base — a bounded filesystem walk, no git process per directory —
  each named for the directory it was found in so that
  [§AR-004-forest.3](../../architecture/AR-004-forest.md#3-workspace-resolution)
  keeps one answer. The row still has the last word on a branch it also names,
  and a branch only the disk knows cannot widen what the project claims:
  identity is the row's alone ([§FS-008-attribution.1](../../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)).
