- **`--registry` was parsed and dropped by most of the commands that offer
  it.** The manual has always spelled the resolution `--registry` →
  `$EPHOR_REGISTRY` → the configured file, and one branch of `main` did that;
  every subcommand that returns early and resolves the registry for itself —
  `status`, `feed`, `refresh`, `checkout`, `rebase`, `work`, `capabilities`,
  `doctor`, `tui` — went to the configured one regardless. So
  `ephor capabilities --registry <other>` answered about a file the reader had
  not named while wearing the label of the one they had, which is worse than
  not offering the flag: it makes "try the change against a copy first"
  quietly impossible. The flag is recorded once, before anything dispatches,
  where all of them already look. Every test in the tree drives ephor through
  `EPHOR_REGISTRY`, which is why nothing caught it; the new one uses the flag
  and points the environment elsewhere.
