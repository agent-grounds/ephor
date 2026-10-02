- **A project says who does which action, and which hands may be used on it at
  all** ([§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions),
  [§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).
  Every ticket ephor wrote went to whoever the runtime would have picked
  unasked, so a trivial replay and the conflict that needed judgment were the
  same request — and the only way to change that was to point the runtime
  itself somewhere else, for everything. `work.hands` now maps an action's id
  to a hand from the roster (`{ "default": "sonnet", "rebase": "luna:high" }`),
  at site level and under `projects.<id>.work`, and the seven steps that answer
  *who does this* run narrow before broad: what you picked for this dispatch,
  the `hand` the recipe carries, the project's entry for this action, the
  project's default, the site's entry for this action, the site's default,
  then the runtime's own unasked pick. It is the runtime's resolution order mirrored, so the two
  cannot disagree about one configuration. The long form
  `{ "agent", "model", "effort" }` stays legal for a pair the runtime's
  registry never listed — a proxy serving a model it does not know — and is
  accepted with a note, since ephor cannot prove it invalid; a name the roster
  does have is checked, and a typo or an undeclared effort is refused before
  anything is written. `permitted_hands` narrows a project to the hands that
  may work on it, for a repository under a policy about which models may see
  its code: anything outside it is refused with that reason wherever it was
  named, never quietly replaced. With no table anywhere nothing changes, and
  with no runtime on `PATH` a configured hand resolves to nothing, says so in
  the workable rung's own words, and the ticket is written all the same.
