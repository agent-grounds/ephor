- **`ephor check` runs a project's own verbs from its checkout**
  ([§FS-006-project-interface.5](../../functional-spec/FS-006-project-interface.md#5-checks-are-verbs-and-every-script-is-self-contained),
  manual §9.3). The seam had no command line; now it has one, and it is what
  the shipped step stands on. With no verb named it runs the aggregate where a
  project declares one and whatever else it declares where it does not; the
  project's output streams to your log rather than being swallowed and
  summarized; a verb that exits `75` is parked, not failed; and
  `--list-features --json` prints what a matrix fans out over.
  `ephor validate --schema-only` is the registry half — the schema is what a
  repository can check, since the checkouts its rows name are on somebody's
  machine.
