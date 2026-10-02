- **Tool configuration has a home, and `.agents/` is its deprecated one**
  ([§FS-006-project-interface.12](../../functional-spec/FS-006-project-interface.md#12-what-the-toolchain-keeps-in-a-checkout-has-a-home-and-a-deprecated-one),
  [§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project),
  [§FS-010-doctor.1](../../functional-spec/FS-010-doctor.md#1-it-reports-what-is-already-judged-and-judges-nothing-itself)).
  `.agents/` is where an agent's instructions live and a sandboxed runtime
  mounts it read-only, so what the toolchain maintains moved out from under
  it: the runtime settings overlay a work root may carry is now read from
  `.agent-grounds/<runner>/settings.json`, and this repository's own
  `grund.toml` moved to its root and `fissile.toml` to `.agent-grounds/`.
  Reading one of these files is one probe in one fixed order — the home first,
  the deprecated `.agents/` name second — so both names go on working and a
  root carrying both is answered by the home with the other passed over rather
  than merged. Finding the old name produces one sentence naming the file and
  where it belongs now, and nothing else: the roster keeps every hand it had,
  no command exits differently, and the sentence travels as a dispatch note in
  prose and in `--json` alike. `ephor doctor` and `ephor capabilities` print
  that same sentence per project for a checkout still on the old name, as
  `deprecated` in the machine form, moving neither health nor the exit code.
  ephor reads both names and writes neither — it creates no tool configuration
  in any checkout. CI's pinned `fissile` moves to `0.8.3`, which is the first
  version that discovers `.agent-grounds/` on its own; no `--config` flag is
  added anywhere. (PR #75)
