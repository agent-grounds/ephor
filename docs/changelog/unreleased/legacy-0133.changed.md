- **The local runners now go through the one executor**
  ([§AR-002-summons](../../architecture/AR-002-summons.md#ar-002-summons-one-executor-runs-everything-ephor-asks-of-the-world)).
  Configured actions, quick actions, the one-off command, the configured
  checkout, and `custom-status` were four spawn sites with four ideas about
  what an exit code means; they are one now. Two things follow for anything
  already configured: `75` means *parked* everywhere — for a status source
  that is "nothing to report just now" rather than a failed refresh — and
  every command may write a structured answer to `$EPHOR_ANSWER`, so
  `custom-status` gains `format: "answer"` (manual §5.1) and speaks the same
  envelope as every other verb, with its stdout-reading `text` and `json`
  forms kept and marked as the legacy they are. `custom-status` is also told
  which project it is running for now, in the same `EPHOR_*` vocabulary a menu
  action receives ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)).
