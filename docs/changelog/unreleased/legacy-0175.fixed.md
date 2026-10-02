- **A finished task in a local store is no longer a matter**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).
  The plan reader pushed every task heading whatever its state, on the belief
  that a finished one would be hidden by the recency window anyway — but a
  local task has no activity time of its own beyond its plan file's, so every
  finished task in a plan resurfaced each time the plan was touched. A project
  keeping its own work in a store showed its whole record as open issues.
  Finality is now the store's own word: the machine its `states.yaml`
  declares, or — where it declares none — the runtime's built-in default, which
  is what such a store's tasks actually run under. A task in a final state is
  history the store keeps and is not read; a store whose machine cannot be read
  reports as a source that did not answer, exactly like a plan ephor cannot
  read ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
