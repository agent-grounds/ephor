- **A parked subtask is visible on an idle root**
  ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
  [§FS-005-dispatch.9](../../functional-spec/FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)).
  The plan floor read only `###` headings and refused the dots in the
  runtime's subtask ids, so a subtask existed to the board only while a live
  run's own listing named it — a run that split a ticket, parked the
  question, and exited left a root that showed nothing at all. The floor now
  reads the runtime's whole heading grammar, taken from its parser rather
  than assumed: `###` through `######`, a dotted id with exactly one segment
  per heading level — names or canonical numbers — and a kind word matched by
  shape, since Title Case is the runtime's convention, not its grammar. A
  parked subtask keeps its row with no run and no runner, like any other
  ticket, and a new dispatch still follows the last dispatch: a subtask is
  never the prior of a top-level ticket.
