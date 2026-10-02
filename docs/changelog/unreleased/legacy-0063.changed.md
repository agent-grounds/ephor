- **One live run per checkout, enforced where runs start**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)). The autorun sweep skipped a work *root* a run
  already held, so two roots over one checkout — a second panta beside the
  first, or a hand-started run beside a swept one — could both put an agent in
  the same working tree. The guard is now over the checkout, at every place a
  run starts: the sweep passes over a root whose tree a live run holds from
  any root — naming that run, rather than reporting an empty sweep — a tree a
  launch takes mid-sweep is passed over for the rest of that sweep with the
  run that took it, one `ephor work run` over two roots in one tree starts one
  of them and refuses the rest by the id of the run it just made, and `R` on
  the work screen is refused the same way. The refusal is `a run is live in
  this checkout: <id>`; `--force` lifts it for a run asked for by name and
  does nothing with `--due`. Queueing is untouched — laying or dispatching a
  ticket into a busy checkout's plan still succeeds, which is what makes
  handing work down to a tree somebody is in safe. (PR #62)
