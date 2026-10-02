- **The operations board enumerates the work roots, not the ledger**
  ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).
  Rows used to derive from ledger entries — operations about items ephor
  itself dispatched, about a third of what the section claims — so a plan
  written by hand, a project's own planning tickets, and a run started in
  another terminal on a root ephor never wrote into were invisible. The
  board now enumerates `*.rhei.md` under every work root: the configured
  `work.root`, resolved at each project's checkout and again in every
  branch workspace on disk, with aliased spellings of one directory
  collapsing to one row. Such an operation has no matter behind it by
  construction, so `Enter` opens its plan, titled in the plan's own
  heading. The walk runs when rows are built — the board opened, a refresh
  landing, the glance seeing something move — never on the bare 2-second
  tick, which keeps statting only what the last walk found
  ([§FS-005-dispatch.15.1](../../functional-spec/FS-005-dispatch.md#151-the-board-keeps-itself-current));
  measured against a real tree of fourteen projects and two dozen branch
  workspaces, the walk costs under a millisecond warm and the tick's gate
  stays in the tens of microseconds.
