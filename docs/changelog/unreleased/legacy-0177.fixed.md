- **A dead run's leavings are not a question**
  ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).
  A ticket parked for a person and a ticket a crashed run was holding
  mid-slot both rendered *waiting on you*, and they ask different things:
  one is a question about the work, the other a run that wants starting
  again. The artifacts already told them apart — parked is the machine's
  gating word on the ticket's own state, a dead run's hold is the journal's
  unreleased slot under a lock nobody holds — so the board now says
  **dropped by a run that died** for the second, listed right after what
  waits on you.
