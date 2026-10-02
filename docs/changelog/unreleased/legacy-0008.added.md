- **A recipe can ask for its own sweep, and say how often.** `"dispatch": "6h"`
  on a recipe says both that the sweep which *finds* its matters needs nobody
  and at what rhythm — the shape `autorun` already has, one step earlier. Its
  presence is the opt-in: a recipe that says nothing is swept by nobody, exactly
  as before. `"0h"` is every time ephor is asked, and the map form
  `{"every": "6h", "limit": 3}` bounds what one sweep of that recipe may open.
  The sweep is `ephor work sync`, which the shipped work-sync timer already
  runs, so adopting the field needs no unit file edited: sync already walks the
  feed asking whether ephor has work about each matter, and the *no* answer
  stops being unconditional. The interval paces the looking rather than
  triggering it — ephor has no daemon, so the caller's own rate is the ceiling
  on every value — which is how `ephor work run --due` has always worked. When
  each recipe last swept is ephor's own record, kept in
  `$XDG_STATE_HOME/ephor/sweeps.json` beside the feed cache rather than in the
  ledger; missing or unreadable means *due now*, and a sweep the reader typed
  marks the same clock
  ([§FS-005-dispatch.32](../../functional-spec/FS-005-dispatch.md#32-a-recipe-can-ask-for-its-own-sweep-and-say-how-often)).
  (PR #104)
