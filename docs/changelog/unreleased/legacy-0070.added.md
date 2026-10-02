- **Which scope a plan belongs in is a rule, not a configuration choice**
  ([§FS-014-work-root-scopes](../../functional-spec/FS-014-work-root-scopes.md#fs-014-work-root-scopes-a-plan-lives-in-the-smallest-scope-that-can-see-everything-it-touches)).
  A work root exists at three scopes — organization, project, checkout — and
  the mechanism for pointing one anywhere has been configurable for a while
  ([§FS-005-dispatch.6.1](../../functional-spec/FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)),
  with nothing saying which of the three a given plan belongs in. The rule is
  now written down: **a plan goes in the smallest scope that can see everything
  the work touches**, and nothing else places it — not who filed the matter,
  not how long it will take, not the directory the reader was standing in. What
  follows from it is written down with it: upper scopes decide and hand a
  ticket *down* into the checkout that must change rather than editing it, so
  the one-live-run-per-checkout guard still means something; the handed-down
  ticket names the work that laid it and that work's result names the ids it
  spawned; nothing durable is left in a root that dies with its branch; and
  three near-misses are refused by name, including nesting the runtime's work
  roots and keeping a tree alive to keep its record. The declaration says which
  three parts of itself nothing yet enforces, and
  [§RM-005-scopes](../../roadmap.md#rm-005-scopes-a-plans-scope-is-enforced-not-merely-written-down)
  carries what each is waiting on. Documentation only: no command, code path or
  output changes. (PR #67)
