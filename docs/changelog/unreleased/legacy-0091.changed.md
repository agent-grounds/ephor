- **A run of the runtime starts beneath the screen, and is watched by
  attaching**
  ([§FS-005-dispatch.20](../../functional-spec/FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching),
  [§AR-007-runtime](../../architecture/AR-007-runtime.md#ar-007-runtime-the-runtime-adapter-and-rhei-as-its-shipped-binding), PR #4). Pressing `R` used
  to hand the whole interface to one run for as long as the work took — and the
  work was handed over precisely so that nobody had to stay. `R` and
  `ephor work run` now start the run **detached**, in a session of its own that
  outlives the terminal, and answer with one line saying the run began and what
  it is called. The root turns live on the board from the lock as it always
  did; nothing new is watched. `ephor work run --watch` keeps the old
  behaviour, and a runner with no detached shape gets it unasked, saying so.

  **A run has an identity, and it is the binding's.** A live run names itself —
  an id, and the address of its control while it serves one — read from the
  descriptor the runtime leaves beside its lock and never from anything ephor
  remembers having started, so a run somebody began in another terminal is
  named and reached exactly like one ephor dispatched. The board says the id on
  the row, the work screen says it on the operation, and
  `ephor operations --json` prints it beside the control address, the runner's
  own attach command, and the runner's own **stop** command — shown, never run:
  a key that stopped a run would be a channel to the run ephor promised never
  to hold.

  **Watching is attaching.** `a` on the operations board, and
  `ephor operations attach <run>`, open the binding's own surface on a live
  run; leaving it detaches and never stops the run.
