- **A branch quoted in an issue's comments is no longer read as the issue's own
  branch**
  ([§FS-008-attribution.2.1](../../functional-spec/FS-008-attribution.md#21-a-matters-branch-is-one-it-names-never-one-its-conversation-quotes)).
  Branch matching looked for every branch name as a substring of the issue's
  title and its whole comment thread, so an issue whose lifecycle record comment
  quoted the checkout of the run that filed it was placed on that run's branch:
  `ephor work lay` and `ephor actions` put the plan for issue *N* inside another
  issue's finished checkout instead of minting `fix/issue-<N>`, and a ranked lay
  that checks the path refused it every turn. A matter's branch now comes only
  from what the matter says of itself — the branch the forge recorded, a ticket
  key in its id or title, or a branch its title names — so a quoted path or
  ticket key in a comment places it on no branch and a `branch` template mints
  its own. The inbox groups an issue the same way, so one that only a comment
  tied to a branch is now listed with no branch. (PR #150)
