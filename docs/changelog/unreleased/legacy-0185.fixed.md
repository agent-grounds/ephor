- **The operations board lost your place, and the cursor could leave the
  screen**
  ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
  [§FS-005-dispatch.15.1](../../functional-spec/FS-005-dispatch.md#151-the-board-keeps-itself-current)).
  A rebuild fires from the tick and from every refresh landing, and it kept
  only the cursor's index — so a row appearing above it silently changed what
  `Enter` and `o` acted on. The cursor now belongs to the execution root, which
  is what a row of this board is. An operation is several lines, and `j`/`k`
  moved the selection without moving the view, so the selected row could sit
  off screen; the view follows it. A run *starting* on a root the board had no
  row for writes no file the glance watches — the OS takes its lock and that is
  the whole event — so the locks of roots without rows are probed too, and one
  that came alive gets its row. The tick asked for a frame every two seconds
  whether or not anything had moved; it now asks only when something did. `e`
  on a live root whose tickets had all been filtered answered *No plan behind
  this row*, and falls back to the plan the ledger knows for that root. And
  finding each row's matter walked every project's feed once per row, rebuilding
  every matter into a row each time — one walk answers them all.
