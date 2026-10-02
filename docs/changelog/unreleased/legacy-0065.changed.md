- **File-size limits have one authority.** The duplicated numeric limits table
  is removed from [§FS-012-file-size](../../functional-spec/FS-012-file-size.md#fs-012-file-size-every-file-is-measured-against-a-budget-set-by-how-it-is-read), so
  `.agents/fissile.toml` alone holds the configured numbers and its hard-limit
  findings now say that each rule sets its own ceiling. (PR #57)
