- **Dispatch reads an ordering already made, and a limit bounds what runs**
  ([§FS-005-dispatch.26](../../functional-spec/FS-005-dispatch.md#26-an-ordering-already-made-can-be-read-and-a-limit-bounds-what-runs),
  PR #24). Ephor does not compute a rank — it reads one a project already
  wrote: an optional `"ranking": "<path>"` in the `work` block of site
  configuration, or `--ranking <path>` on `ephor work dispatch` displacing it
  for one run, names a file of item ids, one per line, most important first.
  Ranked items dispatch before every unranked one, in exactly the file's own
  order; everything the file does not name follows in the order it already
  had — the file orders, it never filters. `ephor work dispatch --limit N`
  bounds how many items are actually dispatched (opened, or would-open under
  `--dry-run`), taken from the top of that order; an item skipped for another
  reason — it already has work, it fails `--kind` or `--updated-within`, no
  recipe applies — costs nothing against the bound. A ranking file that is
  absent, empty, or unreadable is not an error: the sweep falls back to
  today's newest-first order, and the reading says which of the three
  happened; an id the file names that matches no item is skipped and named,
  not fatal. The reading says which file it used and how old it is, in prose
  and in `--json` alike. With no ranking configured and no `--limit`,
  behaviour and output are exactly today's.
