- **`ephor work lay --dry-run` leaves the ordinary work root untouched**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
  A dry run now asks the runtime to validate and report the resolved workflow
  without creating the work root or the dossier, item, and values files a real
  laying would carry. (PR #29)
