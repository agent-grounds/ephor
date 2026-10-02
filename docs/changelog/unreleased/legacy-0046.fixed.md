- **Captured summons cleanup is observed without a scheduler race on Linux**
  ([§AR-002-summons.2](../../architecture/AR-002-summons.md#2-the-invocation),
  [§FS-016-browser-opening.2](../../functional-spec/FS-016-browser-opening.md#2-automatic-selection-and-truthful-outcomes)).
  The regression proof now waits under explicit bounds for direct-shell exit,
  capture release, and eventual descendant termination instead of sampling the
  descendant once as soon as capture returns. It validates process observations
  after each read and timestamps capture completion before queuing the result,
  so delayed observation cannot hide an expired bound. (PR #109)
