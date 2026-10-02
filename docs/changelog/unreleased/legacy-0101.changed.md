- **An agent-only hand actually binds**
  ([§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).
  A hand naming an agent and no model — which is every hand on a machine whose
  runtime settings declare no model profiles — was chosen and then did
  nothing: the plan language pins a model, so the ticket carried no line and
  the runtime picked as if nobody had spoken. Now the choice binds in one of
  two spellings, never both. A hand carrying a model is written on its ticket,
  as before; a hand that cannot be rides the run instead, as the runtime's own
  `--agent` / `--agent-mode` flags on `ephor work run`, resolved when the run
  is invoked. An effort-less choice is settled by what the hand declares —
  asked plainly where it declares no efforts, completed with a note where it
  declares exactly one, refused with the list where several — so neither
  spelling ever travels effort-less against an agent that has efforts: a bare
  selector would run without any mode, and a bare `--agent` flag would fail
  the run outright where the state machine's mode is not the agent's. The
  flags ride only where they can re-aim nothing: a ticket with a full target
  line is resolved from that line alone and rides beside them, one pinning a
  bare model would take its carrier from them and runs the plan unflagged
  with the reason said, and a claimed ticket is not the run's to advance at
  all. **On a machine with no model profiles, `work.hands` now bites with no
  further configuration — name a hand and run through `ephor work run`; or
  declare a model profile in the runtime's own settings, which pins the hand
  per ticket everywhere and needs none of this.**
