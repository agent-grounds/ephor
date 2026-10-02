- **A repository parked on the base counts toward the rebase onto main alone**
  ([§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it)).
  The copy-is-the-base duplication guard was all-or-nothing across the forest
  while the copy-side count summed across it, so a workspace with one
  repository on the change's branch and two parked on `master` tracking it —
  the ordinary graal shape — showed both menu entries carrying the identical
  number and doing the identical thing to the identical repositories. A
  repository whose published copy is its base is now left out of the
  copy-side sum entirely: its one distance belongs to the base count, the
  copy entry counts and names only the repositories that trail a copy of
  their own, and a checkout of nothing but parked repositories measures
  nothing there and is offered only the rebase onto main. The standing fold
  behind all of this also stopped re-deriving its own facts — the base is
  resolved once and carried on the per-repository answer, one `for-each-ref`
  reads branch, upstream and both distances at once, and presence on disk is
  a path test rather than a subprocess
  ([§AR-004-forest.1](../../architecture/AR-004-forest.md#1-folds)) — which cuts a
  refresh landing on the inbox from ~8 git subprocesses per repository to ~3.
