- **A ticket store that could not be read answered as an empty store**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live),
  [§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
  An unreadable plan directory, and a plan the reader could not parse, both
  became "no tickets" — the one thing an empty section must never mean. A
  store is now reported like any other source that did not answer.
