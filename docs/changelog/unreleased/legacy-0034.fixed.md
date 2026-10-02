- **A branch pushed to a fork is read against its copy there**
  ([§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it)).
  A checkout that fetches from the project and pushes to a fork, wired by
  `remote.pushDefault` or `branch.<name>.pushRemote`, looked for a branch's
  published copy only on the remote it fetches from, so a branch pushed to the
  fork alone read as never pushed: `ephor branches` showed no distance to
  `fork/<branch>`, the rebase onto it was never offered, and
  `ephor rebase --upstream` reported nothing published. The copy is now looked
  for on the remote the branch is pushed to first and on the fetch remote after
  it, the replay onto it fetches the fork too, and the distance from main is
  still counted on the remote the project is fetched from. A repository with no
  push configuration reads exactly as before. (PR #152)
