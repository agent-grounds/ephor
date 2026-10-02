- **A refresh that lost any provider now exits non-zero** (`4`; `3` still means
  every provider failed) and reports each failure as `error:` naming the
  project and provider. A partial refresh used to exit 0, so a source could
  stay dark indefinitely behind a timer that saw nothing wrong
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
