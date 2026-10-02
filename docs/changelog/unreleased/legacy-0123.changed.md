- **The gate is the project's, in three verbs**
  ([§FS-006-project-interface.6](../../functional-spec/FS-006-project-interface.md#6-the-gate-is-the-projects-in-three-verbs)).
  `status` answers what the gate is doing per repository of the forest,
  `failures` answers what actually failed, and `restart` re-runs the failing
  gate and everything downstream of it, committing nothing. Each is bound site
  over manifest over **the forge itself** — a forge-hosted gate needs no
  manifest at all, because the provider's own gate capability is the shipped
  default, and above the seam nothing can tell that apart from a project that
  binds three commands. A `status` answer carries the per-repository breakdown
  through the envelope, so a change gating across a tree can still say which
  repository went red. Restart follows the dispatch semantics it was given:
  exit `75` is "still running, ask again later" rather than a failure, and
  restarting is bounded — past a few, the infrastructure is what is wrong and
  no amount of retrying is the fix. `ci-failures` and `restart-gate` stop being
  examples and become the shipped bindings: `ci-failures` now writes the
  envelope alongside its report, so the same run answers a reader and a
  program. The *gated* rung counts a bound verb, not only a source that
  happens to report one.
