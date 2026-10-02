- **A gate can be restarted from the inbox, in two shapes**
  ([§FS-004-quick-actions.9](../../functional-spec/FS-004-quick-actions.md#9-a-gate-is-offered-the-restart-in-two-shapes),
  [§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)).
  Reading what failed answered the question a red gate asks; it never answered
  the one you usually have about it, which is *was that even me*. A runner
  died, a mirror was unreachable, the same flake landed again — nothing about
  the change is wrong and what it needs is another run, and finding which
  button to click for that took a browser tab and five minutes. A gate now
  carries `⟳ restart what failed` and `⟳ restart the whole gate`. They are two
  entries because they are two diagnoses: one job died on infrastructure, or
  the merge commit itself is suspect and the greens are as untrustworthy as
  the reds. A red gate gets both; a gate that is not red keeps only the whole-
  gate entry, which is the one that still has something to do there; an item
  with no gate gets neither. Restarting everything asks first, and both run
  beneath the screen as jobs
  ([§FS-005-dispatch.17](../../functional-spec/FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)):
  a restart asks nothing and the gate answers minutes later.
  On **GitHub** it is native and per-job — the pull request's head commit is
  resolved, its workflow runs are re-run, and `--failed` is GitHub's own
  *rerun failed jobs*; a check that is not a workflow run is named rather than
  silently skipped. For a forge reached through an **extension**, `restart` is
  a new protocol subcommand carrying the scope as the caller's word, answered
  with what was actually asked for — a count, or the forge's own sentence
  where a whole-gate start is executed asynchronously and no count exists.
  Omitting the count is an answer; a zero there would read as *nothing needed
  restarting*. `ephor restart --scope failed|all` is that same ask from the
  command line, for the sources that answer it — GitHub's entries reach `gh`
  directly, exactly as `see the CI failures` always has.
