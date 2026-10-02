- **A forge can report the review you gave**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)). Only the
  forge knows it: a reviewer list says who was asked and a conversation says
  who spoke, and neither says who answered — an approval leaves no message
  behind, and a reviewer who commented at length has still not approved. So a
  reviewing row showing nothing but `open` could not tell a change you had
  dealt with from one you had not, which is the same gap the reasons beside it
  exist to close, seen from the other side. `review` is a declared capability
  carrying one of `approved`, `changes-requested`, `commented`; `github-prs`
  reads it off the call it was already making, and an implementation that does
  not report one loses nothing. It leads a reviewing row — `[open:approved]` —
  except where a review is being asked for again, since a re-request is the
  forge saying the old verdict is no longer the answer. What a verdict
  *retires* is not decided here: this reports what you did, not what is left.
