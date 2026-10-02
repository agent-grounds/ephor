- **A lost shared source let the whole run report success**
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
  `ephor refresh` counted per-project failures and printed the shared ones
  without counting them, so losing the leg that reads the forge's own notice
  list — the completeness capability — exited `0`. It is a refreshed unit like
  a project now, and its failure is exit `4`.
