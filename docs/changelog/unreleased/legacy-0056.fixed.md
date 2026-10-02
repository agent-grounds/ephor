- **GitHub issue searches no longer silently hide matching work beyond their
  per-question limit**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities),
  [§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
  An authored or participating search that returned the newest 30 issues used
  to be accepted as the whole answer even when older matching issues remained,
  leaving those issues absent from the feed and unavailable to work hand-off.
  Every authored, participating, and followed-label question now fails the
  source when it reaches its configured `limit`, naming the question and ways
  to obtain a complete answer. The default limit, query scope, recent closed
  issues, and below-limit answers are unchanged. (PR #82)
