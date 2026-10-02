- **What a project can do is a ladder, computed once and consulted everywhere**
  ([§AR-005-capabilities](../../architecture/AR-005-capabilities.md#ar-005-capabilities-availability-is-computed-once-and-consulted-everywhere),
  [§FS-006-project-interface.10](../../functional-spec/FS-006-project-interface.md#10-capability-rung-by-rung),
  manual §7.5). Whether something could run was decided in a dozen places, each
  with its own sentence or with none: a menu entry that could not run said only
  `(unavailable)` and made you press it to find out why, the inbox's `R` key
  handed the terminal to a runner that was not installed, and `ephor work run`
  had the one good sentence about it. There is a `CapabilitySet` per project
  now — eight rungs, each held or missing with the sentence saying why — and
  offering is filtering on it while refusing is rendering it. You see the same
  words wherever you meet the limit, and you see them where the feature would
  have been rather than after choosing it. It is resolved at load, after every
  refresh, and after a checkout, from `stat` calls and configuration only. And
  because a table speaks for the moment it was resolved, the executor
  re-checks the two things a command leans on — its directory, and its script
  where the binding names one — at invocation, so a directory deleted since
  then fails as the world rather than as a stale answer.
