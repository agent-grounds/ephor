- **A workflow entry can ask to run itself, and what it lays down is due like
  any other work**
  ([§FS-005-dispatch.28](../../functional-spec/FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  `"autorun": true` may now be written on an entry that lays a workflow down,
  in all three of its homes — beside the workflow, in the project's manifest,
  in your own `status.json` — and is refused on an entry that runs a command
  or asks for a ticket, where it would be a second spelling of a fact the
  recipe already carries. `ephor work dispatch` lays the first matching
  entry that said it about a matter no recipe covers and that has no work,
  in the ranking's order — finished work never matches, exactly as it never
  matches a recipe — counted against `--limit`, honest under
  `--dry-run`, reported as `laid` / `would-lay` rows and a `laid` count in
  prose and `--json` alike; a refusal is named with nothing written, and a
  second sweep does not lay it again. `ephor work run --due` then treats the
  laid plan as ordinary work: its tasks are read wherever the runtime wrote
  them — including the `tasks/*.md` of a plan rendered as a directory — and
  every surface that reads them judges them by the state machine in force for
  that plan — the one beside it where it declares one, and the work root's
  where it declares none, which is what the runtime itself resolves such a
  plan against: the due sweep, the operations board, and the menu row all
  call a task its own machine parks *waiting on you* rather than queued, and
  a board that had to withhold judgment for want of a machine names the plans
  it withheld it for. A plan nothing in the record laid is still nobody's to
  start, whatever its tasks are named. It runs with capacity ceilings, cross-process
  reservation, ranking, failed-start back-off and the branch guard applying
  exactly as they do to a root a recipe wrote. Recipes keep their priority, and an entry that says
  nothing behaves exactly as before: a menu row, laid by you and started by
  you. (PR #28)
