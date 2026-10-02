- **Every seam has an executable scenario, and the scenario cites its spec
  point** ([§FS-006-project-interface](../../functional-spec/FS-006-project-interface.md#fs-006-project-interface-a-project-and-ephor-meet-over-one-interface-in-three-homes),
  `e2e/cases`). Six cases under `e2e/cases/`, each one a story run against the
  real binary in a temporary world: a forge nobody built in, reached by a bash
  script on `PATH`; a repository checking itself with `ephor check`; a project
  whose CI answers `status`, `failures`, and `restart` from its own commands; a
  plan directory in a checkout arriving as matters; a pull request dispatched
  to tickets that sit on disk until a runtime is bound, then run by one that
  hands back a verdict and a drafted reply; and a project's menu offer refused
  in the ladder's own sentence when a rung it needs is missing. The file *is*
  the `E2E-NNN` declaration, so `grund check` holds each scenario to the
  E2E→FS rule and `grund refs` finds, for a spec point, the scenario that runs
  it. They are cargo test targets declared by path, which puts them in
  `just check` and CI and keeps them out of the published crate; `just e2e`
  runs them alone.
