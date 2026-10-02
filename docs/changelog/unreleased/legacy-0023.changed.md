- **A due sweep stops restarting a root whose runs advance nothing**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
  [§FS-005-dispatch.15.2](../../functional-spec/FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream),
  [§AR-007-runtime.1](../../architecture/AR-007-runtime.md#1-what-the-module-owns),
  [§AR-007-runtime.3](../../architecture/AR-007-runtime.md#3-degrade)).
  The sweep now reads what the last run on each root actually did, out of that
  run's own event stream: a run advanced something if it recorded a pass that
  progressed, or released a slot in a completing outcome, either being enough
  on its own. A root whose last run advanced nothing is **passed over** with
  the reason in the row where it used to say `started` — five minutes, then
  ten, on the same doubling arithmetic and the same two-hour cap a failed start
  rests under, though the hold below arrives before that cap can — and past
  three consecutive such runs it stops being started at all until
  one advances there or somebody starts one by hand. A run that moves
  something drops the memory whole. This changes the default for every
  existing `--due` caller, which is the point: the previous behaviour restarted
  a stalled root on every sweep, held a slot, and printed `started` each time.
  An outcome word this reader does not recognize, a runner writing no stream,
  or one declaring a layout it cannot read leaves the whole rule inert rather
  than resting a healthy root. The rest binds the sweep only: `work run
  --item` on such a root still runs it, and `--force` neither lifts this nor
  needs to. (PR #88)
