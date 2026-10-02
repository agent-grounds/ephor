- **Work can stop for a person, and say so where you are looking**
  ([§FS-005-dispatch.9](../../functional-spec/FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)). Where a ticket sits in a state the runtime will not
  leave on its own — a gating state — ephor reads that out of the machine and
  leads the item's badge with `⚠ waiting on you`, ahead of anything else the
  work is doing, since it is the one part nobody else will move. The work
  screen prints the ticket and the `rhei transition` that resumes it. The
  question and its answer stay in the plan: an agent writes `NEEDS-HUMAN:` as
  the first line of its artifact, a program routes on it, and the person
  answers beside the question rather than in a chat window the next round
  cannot read. `config/ci-green.example.states.yaml` is a worked machine that
  uses it — collect, triage on a cheap model, then analyze → propose →
  critique → fix → verify per failure, with the escalation available at every
  step.
