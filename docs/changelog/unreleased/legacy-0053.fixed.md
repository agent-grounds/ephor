- **A named work run now gives one first refusal on the command line and work
  screen**
  ([§FS-005-dispatch.30](../../functional-spec/FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work),
  [§FS-015-spend-ceiling.6](../../functional-spec/FS-015-spend-ceiling.md#6-only-the-sweep-is-bound-and-the-persons-key-never-is)).
  Root validity is now decided before live-run safety below both surfaces, so
  an unreadable machine or wrong branch is named first and `--force` still
  lifts only a live-lock refusal. Root-refused groups no longer enter the
  named-run budget lookup: an all-root-refused selection prints no spend
  warning, while a valid group later refused by a live run remains eligible.
  (PR #91)
