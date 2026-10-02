- **The reader picks the hand, once**
  ([§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).
  The first of the seven steps — what the reader picked for this dispatch
  alone — existed only as an unfed slot in the resolution. `t` on a menu
  entry that hands work over now opens the picker: the roster's hands in one
  column and the selected hand's declared efforts in a second, absent where
  it declares none — the common shape on a machine with no model profiles.
  An unavailable hand is listed with its reason and cannot be chosen; a hand
  the project's `permitted_hands` excludes is not listed at all; with an
  empty roster there is no picker and the entry dispatches unchanged. The
  same pick rides the command line as `--hand <hand>[:<effort>]` on `ephor
  work dispatch` and on `ephor rebase --dispatch` (env spelling `HAND`), so
  the key and the command are one operation. A pick lives exactly one
  dispatch: nothing records it, and the next dispatch of the same action
  resolves from the tables again. The work screen's offers now name the hand
  each would go to, in the same sentence the menu's entries carry.
