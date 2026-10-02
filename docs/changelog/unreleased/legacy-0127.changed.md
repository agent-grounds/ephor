- **A row that comes back says what brought it back**
  ([§FS-007-matters.2](../../functional-spec/FS-007-matters.md#2-same-subject-one-matter-related-subjects-linked-matters),
  [§FS-007-matters.5](../../functional-spec/FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)).
  Merging is the model's now rather than a pass over rendered rows: reports of
  one subject fold into one matter, the fuller one surviving and the thinner
  one handing over the conversation, gate and reasons it alone saw. Matters
  that *reference* each other — the change implementing a ticket, the ticket
  it names — stay two matters and are linked on both sides, because merging
  what is one thing and linking what is related is the difference between a
  readable pile and a lossy one. And `seen.json` now remembers what a matter
  looked like when you read it, so when it returns the row says **⟳ the
  conversation moved** or **⟳ the gate moved** instead of only reappearing.
  A row marked read before this shows no reason rather than a guessed one.
