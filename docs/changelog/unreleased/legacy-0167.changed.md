- **A branch knows where it is published, and the row says both distances**
  ([§DA-003-upstream-is-the-published-copy](../../decisions/architectural/DA-003-upstream-is-the-published-copy.md#da-003-upstream-is-the-published-copy-a-branchs-upstream-is-its-published-copy-not-its-tracking-config),
  [§AR-004-forest.1](../../architecture/AR-004-forest.md#1-folds)). The upstream of
  a branch, as ephor means it, is its **published copy** — resolved per
  repository from that repository's own `HEAD`, never from the workspace
  directory's name: the recorded `@{upstream}` where it does not name the
  repository's base (tracking that names the base is `branch.autoSetupMerge`
  recording where the branch was *cut*, and read at face value it would hand
  the menu the rebase onto main a second time under another name), else the
  remote's branch of the same name — the shape `worktree add -b` leaves, the
  one bare `git rebase` fails on — else unpushed, which is an answer and not
  an error. One `for-each-ref` per repository reads ref, upstream and both
  distances at once, and the behind-main count is derived from the same fold
  so the two numbers on a row cannot disagree about when they were measured.
  A checked-out branch row now carries both, apart: `· 13 behind · ↓2` is
  thirteen commits behind the project's main branch and two behind what was
  pushed of this branch. A copy that is level, or a branch published nowhere,
  adds no arrow. No offer reads the fact yet — the rebase onto the published
  copy lands separately.
