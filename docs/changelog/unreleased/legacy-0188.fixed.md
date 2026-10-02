- **Moving the cursor in the inbox redrew at the price of a full attribution
  pass.** Two counts a row shows — how many items a branch holds, and a
  project's visible/unread/awaiting totals — were worked out while drawing,
  and a cursor move redraws without rebuilding, so each keystroke paid for
  them again. The branch count matched every item against every branch, and
  the project totals walked the feed three times over; each walk rebuilds
  every matter into a row, and each match joins an item's whole recorded
  conversation into one string. Measured on a 46-item feed with 26 branches:
  27 ms per branch row, about 273 ms for a screen holding ten of them. Both
  are settled once when the view is rebuilt now, and a draw does no matching
  and no counting
  ([§AR-008-pipeline.2](../../architecture/AR-008-pipeline.md#2-attribute-and-merge)).
  Placing an item also asks the matching engine once for the whole branch
  table rather than once per branch, which is the engine's own ranking rather
  than list order, and 46 items across 26 branches in 5.7 ms rather than 27.
