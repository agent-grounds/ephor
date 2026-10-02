- **One executor for everything ephor asks of the world**
  ([§AR-002-summons](../../architecture/AR-002-summons.md#ar-002-summons-one-executor-runs-everything-ephor-asks-of-the-world),
  [§FS-006-project-interface.3](../../functional-spec/FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)).
  A new `seams` layer holds the summons executor: it resolves where a command
  runs (the branch workspace where one resolves, the forest root otherwise, or
  a named repository of the forest) and refuses with the reason rather than
  running somewhere surprising, exports the dossier as the one `EPHOR_*`
  vocabulary, spawns through `sh -c`, and reads the exit code uniformly —
  `0` done, non-zero failed, `75` *parked*, meaning not applicable now, ask
  again later. Whether the person watches is the call site's property, not the
  binding's. Alongside it, `$EPHOR_ANSWER`: the executor names a fresh file
  before spawning, and a command that writes one gets it validated against the
  published envelope schema
  ([§FS-006-project-interface.4](../../functional-spec/FS-006-project-interface.md#4-the-answer-envelope))
  — now embedded in the binary — with its `failures` and `gate` conveniences
  normalized into events, its `features` into facts, and its answer paths
  resolved against where the command ran. No answer file is a complete answer:
  the exit code stands alone, and standard output is never parsed for
  structure.
