- **Symlinked work roots keep their recorded placement provenance**
  ([§FS-005-dispatch.3](../../functional-spec/FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch),
  [§FS-005-dispatch.4](../../functional-spec/FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  Directory aliases now match ledger recipes and checkout placements by
  identity, while an unrecorded root keeps the caller's spelling when its
  checkout is inferred. (PR #101)
