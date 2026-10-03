- **A registry the schema refuses names every violation at once.** `ephor
  validate`, `ephor refresh` and every other command that loads the registry
  used to name only the first place it failed the schema, so a hand-written
  registry missing five fields took five runs to fix. The refusal now lists
  each violation on its own line under a header that counts them, and closes
  by naming `config/workspaces.example.json` as a complete registry to start
  from; under `--json` the list rides in `says`
  ([§FS-006-project-interface.11.1](../../functional-spec/FS-006-project-interface.md#111-a-registry-the-schema-refuses-is-refused-whole)). (PR #177)
