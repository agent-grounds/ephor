- **A branch that trails its own published copy is offered the rebase onto it**
  ([§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it)).
  Somebody else pushing to your branch is a different fact from main moving
  under it, and now it has its own move: a second quick action beside the
  first, naming the ref and the count — `⤴ rebase onto origin/you/ABC-42-retry
  (2 behind)` — and `ephor rebase --upstream`, which excludes `--onto` because
  a per-repository ref has no branch name to give. The replay is a fold with a
  different base in every repository, each branch's own copy read off its
  `HEAD`; a repository that has published nothing is reported as *nothing
  published* and the run still exits `0`, in the same register as one already
  current, never as a refusal. Every other guard is unchanged: uncommitted work
  reported and left alone, a rebase already stopped reported as the conflict it
  is, and a conflict handed over rather than decided. This is the case bare
  `git rebase` cannot do — a branch grown by `git worktree add -b` and pushed
  carries no tracking configuration, and git refuses before it starts. The
  offer is withheld in exactly one place: where the published copy *is* the
  base for every repository under the checkout — a workspace repository parked
  on the main branch and tracking it — because two entries running one
  operation is the duplication the resolution exists to prevent; where the
  repositories disagree, both are offered and the report says what happened to
  each. Recipes and project offers gain a matching `behind_upstream` selector,
  measured from the same fold as `behind` — on a project naming no main
  branch, dispatch measures the fold all the same and nulls only `behind`,
  the same split the rows make, so a `behind_upstream` recipe offered in the
  menu is dispatchable everywhere it is offered.
