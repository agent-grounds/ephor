- **Two matters whose ids differ only in punctuation are two plans, and no
  ticket waits on work about another matter**
  ([§FS-005-dispatch.3](../../functional-spec/FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch),
  [§FS-005-dispatch.5](../../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)).
  A plan's file stem was reduced from the matter's id without the digest
  `{id_slug}` carries, so a task `retry-1` in a plan called `window` and a task
  `1` in a plan called `window-retry` named one file — and the second matter's
  ticket was written into the first matter's record and ordered *after* it, so a
  run would not reach it until work about something else finished. Both
  dispatches exited saying they had opened the work. The stem now carries the
  same digest, and a ticket's `**Prior:**` is the last uncancelled ticket that is
  about the same matter, read from the `id` the ticket itself records — so a plan
  an older ephor or a hand left holding two matters still orders each matter's
  work after its own. A ledger a newer ephor wrote is refused by name rather than
  read forward, since a recorded plan name is now something ephor rewrites in
  place. (PR #137)
