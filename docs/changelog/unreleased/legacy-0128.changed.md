- **One subject, one row: the CI and review-thread rows dissolve into the
  change they are about**
  ([§FS-007-matters.3](../../functional-spec/FS-007-matters.md#3-a-discussion-is-messages-grouped-in-a-channel),
  [§FS-007-matters.5](../../functional-spec/FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)).
  A pull request with a red gate and two unresolved review threads was four
  rows: the pull request, a CI row for the same change, and one row per thread.
  It is one row now. `github-ci` reports the pull request carrying its gate —
  a gate is an observation of a change, not a subject — and `github-threads`
  reports the pull request carrying its unresolved threads as discussions.
  Merging keeps the fuller report as before, and now carries over what only
  the thinner one saw: a conversation it alone read (deduplicated, so the same
  thread reported by two sources is shown once) and a gate it alone fetched.
  Two consequences. The **CI category** now holds what it says it holds and
  nothing else — periodic build results, not gates that belong to a change —
  so it is empty until something reports one. And the **Failing** column
  counts matters whose gate is red wherever they sit, rather than rows of one
  kind, so it means the same thing it always did.
