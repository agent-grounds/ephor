- `config/ci-green.example.states.yaml` gained the half that closes the loop:
  a program authors a **join ticket** whose `**Prior:**` names every cause
  ticket, so it runs only once all of them have finished, and a `land` state
  then pushes and lets the gate run again. Two things make it safe — the
  give-up state is named `cancelled`, which is the only final state that does
  *not* satisfy a prerequisite, so nothing is pushed while a cause is
  unresolved; and `land` commits nothing, because every fix commits its own
  work and `git add -A` before a push sweeps up whatever else was in the tree.
