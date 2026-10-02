- **Every idle checkout is replayed onto main on a timer, and a reused one says
  how far behind it is**
  ([§FS-004-quick-actions.6.1](../../functional-spec/FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding),
  [§FS-004-quick-actions.7.1](../../functional-spec/FS-004-quick-actions.md#71-a-workspace-that-is-there-is-still-owed-its-store),
  [§FS-005-dispatch.3](../../functional-spec/FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  A branch checkout that sits for a day drifts far enough from main that work
  done in it is work against a layout that is gone, and nothing noticed until
  `ship` — after the whole run had been paid for. Two things now do. `ephor
  rebase` given `--workspace`, `--tag` or `--org` sweeps every branch checkout
  of the projects that selector names through the same per-checkout replay,
  under `--act`, and `systemd/ephor-rebase-sweep.{service,timer}` runs it
  hourly with nobody watching. It passes over — and says it passed over, with
  the reason — the project's main-branch checkout, which is `ephor update`'s, a
  checkout a live run holds, a branch with an open pull request that is not a
  draft, and a checkout an earlier sweep already wrote a conflict ticket about;
  the review answer comes from what the watch already fetched, freshened once
  per project rather than once per branch and not at all by a run held at the
  gate, and a project whose reading cannot be had has **none** of its checkouts
  replayed and makes the sweep exit non-zero, because *could not tell* is not
  *no* — asked only of the sources that could have carried a pull request, so a
  failed status or message source stops no rebase. A conflict puts the tree
  back on the commit it was on, reports it, and — where the project's work
  configuration names a `rebase-sweep` recipe, which is a reserved id offered
  on no matter — writes it up as a ticket in one plan named after the sweep,
  under an id carrying the branch's own fingerprint, closing with what became
  of the tree: restored, or left exactly as found where the checkout was
  already stopped in somebody else's rebase, which nothing here touches. A
  write-up that could not be made is on the row and in the reading as `note`.
  Forty checkouts become one exit code, conflict winning: `3` where any
  conflicted, `1` where any project was not reached, `0` otherwise — a checkout
  with uncommitted work is reported, left alone, and a good end, since a tree
  somebody works in is dirty most of the time and a unit that went red for it
  would read failed always. And `ephor
  checkout` on a directory that is already a whole workspace no longer says
  only `already checked out`: it says `115 behind main (as of Sep 10)` too,
  from the same fold the branch row for that directory renders, which is the
  line that would have stopped the run behind all of this before it started.
  The pull-request reading now carries the forge's draft flag, absent where the
  forge has no notion of one. A report embedded in a ticket's body now has its
  headings flattened to emphasis before it is written
  ([§FS-005-dispatch.3](../../functional-spec/FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)),
  which also repairs the plan `ephor rebase --dispatch` has been writing: a
  heading inside a body is a node to the runtime, and a plan it cannot parse is
  a ticket nobody can work. (PR #89)
