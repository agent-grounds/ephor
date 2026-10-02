- **Workflow enumeration reports runtime invocation failures instead of an
  empty registry**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
  A bound runtime that cannot reserve its answer file or whose workflow
  listing exits non-zero now fails the CLI, API, menu, laying, dispatch, and
  autorun request that asked for it, preserving the runtime's refusal where it
  ran. Only successful workflow listings enter the dispatcher cache, so a
  summons failure can no longer become an authoritative empty offer set.
  Successful empty and populated listings, and the explicitly unbound runtime,
  keep their existing behavior. (PR #83)
