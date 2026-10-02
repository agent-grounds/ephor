- **A project can offer menu entries, and yours still win**
  ([§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions),
  manual §7.6). The `actions` of a project's `ephor.json` now arrive in the
  action menu, between what ephor recognized and what you configured. All three
  are one shape: an `id`, a `when` selector in the language work recipes use —
  roles, gate, `needs_response`, sources, `behind`, not just `kinds` — a
  `requires` list of capability rungs, a `cwd` saying which repository of the
  forest it runs in, and `confirm` for an entry that should be asked about
  twice. Where two entries share an `id`, the later provenance replaces the
  earlier **where it already sits**, so yours beats the project's beats the
  shipped one without renumbering the menu under your fingers. A `requires`
  rung you do not hold leaves the row visible with the ladder's own sentence,
  and a requirement that is not a rung at all is refused by name rather than
  quietly met. The manifest's trust switch needs no second thought: a checkout
  trusted for descriptions only never carries offers in the first place.
