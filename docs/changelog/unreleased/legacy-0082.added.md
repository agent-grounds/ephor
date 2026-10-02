- **Work nobody has to start starts itself**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
  [§DA-008-a-run-follows-the-ticket](../../decisions/architectural/DA-008-a-run-follows-the-ticket.md#da-008-a-run-follows-the-ticket-autorun-is-a-sweep-that-starts-a-run-never-a-runner-kept-alive),
  PR #6). A recipe may now say `"autorun": true`, and a ticket written from it
  gets its run without anyone pressing a key: the reader's deliberate act
  moves one step earlier and is made once, when the recipe is adopted. What
  starts runs is a **sweep** — `ephor work run --due`, run by dispatch in the
  same breath as the ticket, by `work sync`, and by the work-sync timer —
  which reads the world rather than the ledger: every work root, the plans in
  it, the machine's own words about their states, and the runtime's lock. A
  root is due when it holds an open, unclaimed, unparked ticket from such a
  recipe and no run is live on it, so the sweep is idempotent by construction
  (a root a run already holds gets nothing, because a second run there would
  only wait for the first) and safe to invoke as often as anything cares to.
  A ticket a hand appended counts exactly as a dispatched one: the recipe is a
  fact about the ticket, read from the ledger where ephor wrote it and from
  the id's own `<recipe>-<n>` shape where it did not. Everything dispatch
  refuses before writing a ticket, starting refuses before running one —
  including a working tree standing on another branch, which the ledger now
  records the branch to check. A start that fails is remembered as ephor's own
  act (never as work state) and that root rests before it is tried again,
  doubling to a two-hour cap, so a runner that refuses cannot become a spawn
  loop. Where the binding has no detached shape the sweep starts nothing and
  says so: a run nobody asked for must not take a terminal. Silence still
  means the key — nothing autoruns unasked.
