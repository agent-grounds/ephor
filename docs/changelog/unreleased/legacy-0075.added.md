- **`ephor work lay` loads repeatable workflow values files**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
  `--values <file>` accepts YAML or JSON mappings relative to the invocation
  directory, merges repeated files left to right, preserves structured values,
  and lets explicit `--set` answers win. File answers are shown with their
  provenance, execution targets remain under Ephor's hand policy, and invalid
  or runtime-rejected input leaves no partial workflow workspace. (PR #49)
