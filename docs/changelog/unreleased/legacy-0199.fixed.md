- **The `rebase` recipe asked a model to run the rebase**
  ([§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
  Only `ephor rebase --dispatch` made the deterministic move first; the inbox
  key, `work dispatch --recipe rebase` and `work sync` wrote a ticket whose
  brief said "run `ephor rebase` first" — a pass paid to have two commands
  typed, and a ticket even where the replay would have been clean. A recipe
  may now declare its deterministic opening move, `dispatch` makes it before
  anything is written, a clean replay opens no plan at all, and a conflict is
  handed over as the situation it stopped in.
