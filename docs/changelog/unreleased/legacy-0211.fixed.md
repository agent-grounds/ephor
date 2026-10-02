- A failed command was quoted by the *first* line of its stderr, which is the
  stream tools narrate their progress on: opening a red gate reported
  `Requesting RCA for pull request …` and dropped the error four lines below
  it. The line reported is now the one that reads as a diagnosis, stripped of
  its colour codes, and the command is named once rather than three times
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
