- **An item could be filed under a branch that merely resembled its own.** The
  inbox asked each branch in turn which rows matched it, so a pull request on
  `you/ABC-42-retry` was taken by `you/ABC-42` if that branch came first — both
  carry the ticket key, and near-miss names are common once branches are found
  on disk rather than written down. Each item is now placed once against the
  whole branch list, with the branch the forge recorded winning over any that
  only resembles it, and the count on a branch row is the group beneath it.
