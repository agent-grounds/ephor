- **A ticket carries the item as data** ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), not only as
  prose: every ticket's identifiers — project and source, kind and id, repo and
  number, branch and ticket, url and state, and the checkout — are written into
  the plan's frontmatter under the same names a shell action gets in its
  environment. A state machine hands them to a program as `{meta.*}`, which is
  what lets a **script run in front of the agent**: ask the forge what actually
  failed, write it as an artifact, and let the agent state declare that
  artifact as an input. The metadata is merged rather than rewritten, because
  the runtime keeps its own per-task bookkeeping in the same block.
  `config/ci-failures.example.sh` and
  `config/ephor-work-ci.example.states.yaml` are a working pair, including the
  exit codes that pick the next state — `75` when the gate is still running,
  so a ticket waits on a poll instead of spending an agent on a half-finished
  gate.
