- **Recipes and actions are one menu**
  ([§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
  [§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions)).
  What you could do about a row depended on which key you knew: `x` listed the
  commands and `w` listed the work, and neither mentioned the other. `x` now
  carries both — the recipes that apply to the item stand after the quick
  actions, the project's offers and your own, each with the recipe's own icon
  and a `→` naming the hand that would get it, resolved through the same six
  steps the dispatch resolves, so an unavailable hand says why and a refused
  one refuses the entry rather than being found out at the keystroke. Pressing
  it hands the work over through the one path the work screen uses: same plan,
  same ticket, same ledger entry. A configured action may now carry an
  `agent` block — `brief`, and optionally `state` and `hand` — instead of a
  `command`, which is a recipe under another name and lets a project write one
  agent entry rather than an entry and a recipe that have to agree; an entry
  with both, with neither, or asking for work under no `id` is refused when
  the file is read. Work is offered only where it would work: never about a
  finished item, and — where it edits the change — only where the change is on
  this machine. The shipped `rebase` recipe drops its `kinds`/`roles`
  selector, because the entry that dispatches it asks about a branch on disk
  that trails its base and the two cannot be gated differently; a recipe whose
  id the menu already carries is dropped from it, so a stale branch still
  shows one rebase row and the recipe is what that row hands its conflict to.
  The menu footer is built from the selected entry, and with no runner bound
  the work is still offered — the ticket is written either way — with the
  *workable* rung's own sentence where the hand would be.
