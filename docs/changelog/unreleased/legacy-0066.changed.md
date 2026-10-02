- **A scope selector is honoured or refused, never ignored**
  ([§FS-011-command-line.9](../../functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)). `--workspace`, `--tag` and `--org` were parsed by
  every verb and read by three, so `ephor status --org foundation` and `--org
  graal` printed the same site-wide table and `ephor work dispatch --org X
  --dry-run` proposed the other organization's work. Every variant of the
  command tree is now classified: `list`, `status`, `feed`, `refresh`,
  `mark-read`, `branches`, `tui`, `work list`, `work dispatch`, `work sync`,
  `work run`, `validate`, `ensure-agents` and `update` scope their reading by
  the selectors, and every other verb refuses each one it was given, naming
  itself and the flag and exiting 2 — the code an empty selection and every
  other configuration refusal already took — and under `--json` saying so as an
  outcome on standard output like any other refusal. Because the feed-side
  verbs pick their projects from `status.json` while the selectors name
  registry rows, a selector is resolved against the registry and intersected
  with what the site watches; a selection that comes out empty is refused
  rather than printed as an empty table. The screen scopes with them too: a
  `tui` opened under a selector shows those projects and its `r` key now
  fetches only them. **Callers who passed a selector that was silently ignored
  now get an error, and that is the point.** (PR #58)
