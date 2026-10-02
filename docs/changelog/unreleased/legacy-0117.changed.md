- **Three CI steps ship, and they run from your repository alone**
  ([§FS-009-shipped-actions](../../functional-spec/FS-009-shipped-actions.md#fs-009-shipped-actions-what-ephor-ships-for-ci-runs-from-the-repository-alone),
  manual §9.3). `setup` installs a pinned ephor release, checksum-verified,
  and puts it on `PATH`; `validate` holds a repository's `ephor.json` — and a
  committed registry where it keeps one — to the published schemas; `check`
  runs the check verbs the repository declares, one job per feature where its
  smoke enumerates them. Each ships twice: as a composite action to compose
  with, and as a `workflow_call` workflow that is a whole job. What selects
  them is the rule that a shipped step reads repository-committed material and
  workflow inputs and nothing else — no registry, no bindings, no credentials
  for anybody's sources — so the watch-and-work loop stays on machines that
  have a site. ephor's own CI is the first consumer, running them against
  ephor's own `ephor.json`.
