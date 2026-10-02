- **GitHub issue dependencies now govern work hand-off**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities),
  [§FS-003-feed-categories.4](../../functional-spec/FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it),
  [§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
  [§FS-005-dispatch.5](../../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)).
  The GitHub issue source fetches first-class **blocked by** relationships in
  its existing batched search. Feed rows and dossiers name every open
  prerequisite, while recipe, ad-hoc, and workflow hand-offs are withheld
  until they close; non-work actions remain available. The work ledger
  fingerprints those open prerequisites, so closing the final blocker is
  observable movement even when the dependent issue's own timestamp does not
  change. Labels and issue prose are never interpreted as dependencies.
  (PR #80)
