- **Repeating a workflow action no longer lays a second plan**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here),
  [§FS-011-command-line.1](../../functional-spec/FS-011-command-line.md#1-what-may-be-done-here-listed-and-run)).
  When the selected entry's newest workflow dispatch records the unchanged
  matter and its exact plan is still present, the shared action move now
  refuses before resolving inputs or asking the runtime to instantiate. It
  names that plan and points to `ephor work run --item <id>` to start the
  existing work, while `ephor work lay <entry> --item <id>` remains the
  deliberate route to another plan. Different entries, changed matters, and
  missing recorded plans retain their existing laying behavior. Prose exits
  `1` on standard error; JSON uses the existing refusal outcome with the same
  sentence and no new plan field. (PR #96)
