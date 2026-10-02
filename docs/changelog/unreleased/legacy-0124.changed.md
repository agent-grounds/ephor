- **Checks are verbs, and which ones run stays your decision**
  ([§FS-006-project-interface.5](../../functional-spec/FS-006-project-interface.md#5-checks-are-verbs-and-every-script-is-self-contained)).
  ephor binds three: `check`, `style`, `smoke` — probed as `./check.sh`,
  `./check-style.sh`, `./smoke-test.sh` at the forest root, or declared in the
  manifest under whatever paths a project prefers, or bound by you, in that
  precedence. Each runs as a summons and answers through the envelope, so a
  verb's `summary` and `failures[]` land in the verify dossier instead of a
  wall of log. Smoke may enumerate **features** — a static list in the
  manifest, or `--list` printing one id per line, which is a complete answer
  and not something ephor asks to be JSON — and a feature id runs that
  feature's smoke alone. What ephor does **not** decide is which verbs run or
  in what order: `$EPHOR_CHECKS` hands the bound ones to the verify step,
  newline-separated, and `config/verify.example.sh` sequences what it is
  given, falling back to its old guessing only when nothing was handed over.
  The *checkable* rung now counts a bound verb, not just a well-known filename.
