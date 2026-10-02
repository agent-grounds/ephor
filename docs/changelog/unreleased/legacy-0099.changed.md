- **A ticket can be taken back**
  ([§FS-005-dispatch.16](../../functional-spec/FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so),
  [§DA-005-cancel-is-the-runtimes-move](../../decisions/architectural/DA-005-cancel-is-the-runtimes-move.md#da-005-cancel-is-the-runtimes-move-cancelling-a-ticket-asks-the-runtime-and-never-rewrites-the-state-line)).
  The same recipe pressed twice was two runs of one fix in one checkout, and
  the only way back was an editor on the plan. `c` on the work screen now
  picks an open ticket — numbered, `j`/`k` or a digit — and asks why in one
  line; `ephor work cancel --item ID TICKET… [--why …] [--dry-run]` is the
  same from the shell, several at once. Cancelling is the runtime's own move:
  ephor asks the bound runner for the transition into `cancelled` — the plan
  language's abandonment state, the one final state that satisfies no
  `**Prior:**` — carrying the reason as the ticket's result, and relays what
  the runner answered. It never rewrites a `**State:**` line by hand, since
  the plan language reserves a written ticket's state to the runtime's verbs
  and their checks and trail. The plan keeps the ticket, marked `⊘` with the
  reason beneath it, and the row's badge says `⊘ recipe · cancelled` where
  that is the last word; the operations board counts cancelled apart from
  finished. Refused before the runner is asked, one sentence each: a ticket a
  live run holds (the run's to finish; the lock and the journal say so, as
  the board reads them), one already over, a machine declaring no final
  `cancelled` state — named with what to add — and, with no runtime bound,
  the workable rung's own sentence, exactly as `R`. Cancelling names the open
  tickets ordered after the one taken back, which will not start while it
  stands cancelled; and a ticket ephor appends afterwards — a reopen, an ask
  — is ordered after the last one *not* cancelled, so ephor's own chain never
  hangs off abandoned work. The shipped machine and both example machines
  declare `cancelled` and a `from: "*"` transition into it; a `states.yaml`
  already installed is never rewritten and has to gain both by hand.
