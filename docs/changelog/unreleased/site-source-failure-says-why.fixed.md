- **A site source's rows stay where it placed them until it answers again.**
  Refreshing one project — the refetch behind `ephor status` and `ephor feed`
  once the cache has aged, or the TUI's refresh — dropped every row a site
  source had placed in it until the next full `ephor refresh`, and a site
  source that failed lost its rows instead of keeping them stale. A refresh of
  one project now leaves them as they were, and a site source that stopped
  answering keeps its last rows in each project, marked stale, with what failed
  and whether it was the network beside them, as a project's own source does
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
  A source taken out of `sources` takes its rows with it at the next refresh.
  (PR #155)
