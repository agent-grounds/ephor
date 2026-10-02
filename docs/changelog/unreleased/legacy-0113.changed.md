- **The tree carries nobody's employer any more, and the roadmap says what the
  law still owes**
  ([§FS-001-forge-interface.5](../../functional-spec/FS-001-forge-interface.md#5-no-site-specific-data-in-the-repository),
  [§RM-003-boundary](../../roadmap.md#rm-003-boundary-the-seams-the-law-still-owes)).
  `scripts/check-no-site-specific.sh` passes both halves for the first time: a
  private CLI in a shipped example, a source name in two test fixtures, and the
  worked example in `docs/design.md` — an employer's project, its ticket
  prefix, its build command — are all neutral now, and the packaged crate was
  already clean. `RM-001-forge-interface` is rewritten to what it still owes
  (a run against a public repository from the examples alone, and a first tag)
  rather than to a violation that has been fixed, and the new
  `RM-003-boundary` names the four places where the boundary law is still
  observed by intention rather than by construction: the `defaults.github_user`
  ledger entries, `forest` reaching the git prober, the gate seam having no
  surface, and the `beads` reader nobody has written.
