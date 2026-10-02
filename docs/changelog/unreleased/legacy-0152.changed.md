- `ephor work` — `list` (the ledger, with each ticket's state read back out of
  its plan), `dispatch`, `ask`, `sync`, `run`, `forget`, and `states`. Every one that
  writes answers `--dry-run`, and `dispatch` is the sweep: every item in every
  project that matches a recipe and has no work yet, bounded by
  `--updated-within`. `run` names the plans ephor opened rather than the
  directory holding them, so a runtime project kept in the same checkout for
  other work is not swept in.
