- **Optional workflow execution targets can resolve to nobody**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
  An empty target answer now passes to the runtime as written without being
  parsed, narrowed, or rendered as a hand, so optional tiers can retain the
  execution-target format and its policy for every non-empty choice. Empty
  positions in a list of targets have the same reading. (PR #50)
