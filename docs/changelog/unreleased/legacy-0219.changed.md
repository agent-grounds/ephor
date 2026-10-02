- **A workspace missing one of the project's repositories is completed, not
  called done**
  ([§AR-004-forest.1](../../architecture/AR-004-forest.md#1-folds),
  [§FS-004-quick-actions.7](../../functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)).
  A declared repository with no working tree on disk became visible last round
  — every fold names it rather than quietly answering for fewer repositories
  than you have — and what was left undecided was whether it should also
  *fail*. It does not, in any fold over what is there: an exit code routes an
  outcome to whoever acts next, and this one routes nowhere, since retrying
  replays no tree that is not there, the missing one holds none of your change,
  and the condition was as true before the command ran as after. `ephor
  checkout` is where it does fail, because there it is exactly the outcome the
  command was asked to change — so that is the exit code that answers *is this
  workspace whole*. For it to answer that at all, `ephor checkout` on a
  workspace that already exists now folds over the project's layout instead of
  stopping at the directory: repositories already there are reported as already
  there and left untouched, missing ones are made, and one that could not be
  made exits `1`. A whole workspace still says *already checked out* and
  changes nothing.
