- **A base nobody could resolve no longer turns tracking config into a
  publication**
  ([§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it),
  [§DA-003-upstream-is-the-published-copy](../../decisions/architectural/DA-003-upstream-is-the-published-copy.md#da-003-upstream-is-the-published-copy-a-branchs-upstream-is-its-published-copy-not-its-tracking-config)).
  The rule that a recorded upstream naming the base publishes nothing was
  written as *the base, where one resolved* — so in a repository where nothing
  names a base (no row main, no project main, no `refs/remotes/<remote>/HEAD`;
  the `~/c/g/master/master` shape) a branch cut from `origin/main` and never
  pushed reported `origin/main` as its published copy: the row showed a `↓N`
  that was really the distance to the base, and `--upstream` would have
  replayed onto the very ref the resolution exists to keep it off. It fails
  closed now — an unresolved base cannot clear the record of naming it, and
  only a pushed copy of the branch's own name counts. Two smaller lies in the
  same neighborhood: a replay distance that could not be measured was reported
  as *Already on top of* (unreachable today, but None is not zero anywhere
  else either — it is a refusal now that names the ref), and a declared
  repository with no working tree on disk was silently absent from the
  rebase's answer, which now names it per repository, gating nothing
  ([§AR-004-forest.1](../../architecture/AR-004-forest.md#1-folds)).
