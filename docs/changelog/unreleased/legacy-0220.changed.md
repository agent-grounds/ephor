- **A refresh landing places the project that landed, not every project**
  ([§FS-001-forge-interface.7](../../functional-spec/FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it),
  [§FS-008-attribution.2](../../functional-spec/FS-008-attribution.md#2-two-stages-one-engine)).
  Items arriving mid-refresh are filed under their branches as they land, and
  that was done by re-running the whole site's placement pass on every arrival
  — so a refresh over N projects paid the whole matching pass N times, and the
  cheapest project's landing paid for the most expensive project's items. The
  pass is scoped now, one implementation parameterised by what it answers for
  rather than two that could file a row differently mid-scan and at the end: a
  landing re-places its own project and leaves the rest of the site standing,
  while the reload at the end of the run still places everything. Measured on a
  seven-project cache — 228 items, 27 branch workspaces on the largest project
  — a refresh drops from 2,009 placements to 228, about 15 ms of matching to
  about 2 ms.
