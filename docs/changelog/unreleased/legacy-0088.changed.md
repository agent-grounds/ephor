- **What a live run is doing is read from the run's own stream, not from a
  journal that outlives every run**
  ([§FS-005-dispatch.15.2](../../functional-spec/FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream),
  [§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
  PR #6). The runtime writes a record of each run — truncated when that run
  starts, one line per structural move, numbered and terminated — and ephor
  now reads it wherever it asks which tickets a run has in hand: the board,
  the rows beneath a matter, and the check a cancel makes. It removes a whole
  class of inference rather than adding a source: the transition journal is
  append-only *across* runs, so an assignment a crashed run never released had
  to be argued down from the ticket's own state and the age of a log against
  the birth of the lock, and a witness to one run needs none of that. A run
  that died mid-slot is now read as **dropped** from its own unreleased
  assignment and its missing end record; a run that finished leaves nothing
  open at all. The journal stays the floor and is read unchanged where a
  runner writes no stream, so nothing here becomes a requirement on the
  binding, and liveness is still the lock and only the lock. The stream joins
  the change gate's fixed handful of timestamps, so a run that has written
  only there still surfaces within moments
  ([§FS-005-dispatch.15.1](../../functional-spec/FS-005-dispatch.md#151-the-board-keeps-itself-current)).
