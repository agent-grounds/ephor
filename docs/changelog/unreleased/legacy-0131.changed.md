- **A project is a forest, and every git-facing feature folds over it**
  ([§AR-004-forest](../../architecture/AR-004-forest.md#ar-004-forest-git-is-the-substrate-and-a-project-is-a-forest-folded-over)).
  Repositories were a list of relative path strings that four places rebuilt
  and three folds collapsed into a bare number. They are a `Forest` now — an
  ordered set under a root, declared by the registry row and probed where it
  declares nothing — and staleness, rebase, and checkout fold over it, keeping
  the per-repository answer: a branch row that is `5 behind` can say it was
  `ce 2, ee 3`. Three consequences you can see. **Probing counts each
  repository once**: a checkout with `docs/` and `src/` under a single
  repository reported itself four times behind, because every subdirectory of
  a working tree answers "yes" to being one; a repository is its own toplevel
  now. **A declared repository that is not on disk is named** rather than
  silently dropped from the fold. And **`$EPHOR_REPOS`** hands a summoned
  command the same repository list, in the same order, so the shipped landing
  example folds over ephor's forest instead of probing its own
  ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)).
  Under it, one resolver answers where a branch is checked out — the inbox's
  grouping, the action menu, dispatch, and the CLI had three implementations
  of that question and now share one
  ([§AR-004-forest.3](../../architecture/AR-004-forest.md#3-workspace-resolution)).
