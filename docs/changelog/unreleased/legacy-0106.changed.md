- **`ephor capabilities`: the ladder is answerable on its own**
  ([§FS-010-doctor.2](../../functional-spec/FS-010-doctor.md#2-the-ladder-is-answerable-on-its-own)).
  Every rung of [§FS-006-project-interface.10](../../functional-spec/FS-006-project-interface.md#10-capability-rung-by-rung) was computed with a sentence
  saying why it was missing, and the only thing that ever printed it was the
  interactive inbox — so "why is this action not offered here" cost a TUI
  session. `capabilities [PROJECT] [--json]` prints it, reading the last
  refresh rather than running one.
