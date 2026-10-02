- **A failure that was never the change's fault is restarted, not fixed**
  ([§FS-005-dispatch.11](../../functional-spec/FS-005-dispatch.md#11-a-failure-that-is-not-the-changes-fault-is-restarted-not-fixed)). The loop could recognize a dead runner or a flake in
  two places and act on it in neither: triage was told to open no ticket, which
  ended the plan with the gate still red and nothing to make it run again, and
  an analysis that concluded "not ours" carried no marker `route.sh` knew, so
  it exited `0` and the ticket walked into propose → critique → fix and spent
  two passes fixing something that was never broken. There is now a
  `restart-gate` state and a `NOT-OURS:` marker that reaches it. It is a
  program, not an agent: it is handed the list of jobs as a declared input —
  `<repo> <job-key>` per line, exactly, or `<repo> -` for a whole gate — and it
  restarts those plus every gate the forge still reports red underneath them,
  which is what a gate spanning several repositories needs, since it fails
  downward and the gates below never start. Nothing is committed; the change
  was not the problem. A job that went green while the ticket waited is left
  alone rather than re-redding a passing gate, the restart gets a ticket of its
  own so the next round can see the failure was already retried, and the budget
  is counted out of the plan — past two restarts on one item the work stops for
  a person, because at that point the infrastructure is what is wrong.
  `config/restart-gate.example.sh` is the worked script; `GATE_RESTART` is
  where a forge's own re-run command goes, since there is no neutral one.
