- **Every file is measured against a budget set by how it is read**
  ([§FS-012-file-size](../../functional-spec/FS-012-file-size.md#fs-012-file-size-every-file-is-measured-against-a-budget-set-by-how-it-is-read), PR #23). `.agents/fissile.toml` gives this tree its first
  file-size budgets and `fissile check` enforces them, in the pre-commit hook
  against the files a commit touches, in `just check`, and in a `fissile` job in
  CI against the whole tree. The budget follows the reader rather than the file
  extension: a `§`-declared spec is reached by an ID and fetched a section at a
  time, so it gets 750 soft and 2000 hard lines; an entrypoint is addressed by
  nothing and loaded whole into every session, so it gets 250 and 500;
  `docs/manual.md` is neither, and gets a rule of its own; and
  `docs/changelog.md` is append-only and is not line-measured at all. Four files
  are over a hard limit today, and each is recorded in
  `docs/file-size-human-exceptions.toml` with the boundary it is missing rather
  than trimmed to fit.
