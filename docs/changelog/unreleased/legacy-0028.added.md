- **A due sweep leaves a root out at your asking**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
  [§FS-011-command-line.10](../../functional-spec/FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).
  `ephor work run --due --except <root|item>` excludes a work root from one
  sweep with no judgement of ephor's own, for a driver carrying its own
  back-off. It is repeatable, takes a work root on disk or the id of a matter
  whose work lives in one, and names every exclusion that applied in the row
  and under `--json`; one that matched nothing due is silent. A value that
  resolves to neither is refused quoting it, and so is `--except` without
  `--due`. It excludes without narrowing the width the `--act` gate is counted
  over, which is now stated: a sweep reaching four projects that excludes every
  root but one is still a sweep over four, and so is `work run --due --item`.
  (PR #88)
