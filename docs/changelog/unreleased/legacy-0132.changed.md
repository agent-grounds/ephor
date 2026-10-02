- **Running work is a summons too**
  ([§AR-002-summons](../../architecture/AR-002-summons.md#ar-002-summons-one-executor-runs-everything-ephor-asks-of-the-world),
  [§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
  `ephor work run` and the inbox's `R` key built the runner invocation twice,
  in two places, with two ideas about what a failure was. There is one
  construction of it now and one process path to it, still run from the
  checkout the work is about
  ([§FS-005-dispatch.3](../../functional-spec/FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)),
  and a runner that exits `75` is parked rather than failed. Reading a plan
  in your editor goes through the same path, so the TUI has no hand-rolled
  spawn left.
