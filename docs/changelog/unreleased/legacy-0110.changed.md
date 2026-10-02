- **A released binary is self-checked by doing its job, and the profile is
  trained on the same walk**
  ([§FS-002-release.3](../../functional-spec/FS-002-release.md#3-artifacts)). Both "self-checks"
  ran `--version` and `--help`, which test the argument parser: a binary that
  linked, printed its version and could not refresh a source would have
  shipped. Every artifact is now held to `doctor --self-only`, which builds a
  project of its own and walks the seams against it.

  The PGO training run had the worse version of the same problem. It ran
  `list`, `validate`, `status --cached` and `feed` under `EPHOR_HOME="$repo"` —
  but `EPHOR_HOME` does not redirect the configuration, which resolves
  `~/.config/ephor` first. On a developer's machine that trained on their own
  private registry; on a release runner there is no such file, so every
  command died at "Cannot read workspaces.json" and the profile was gathered
  entirely from error paths. The training workload is the self pass now:
  hermetic, and the only workload that is also what the release verifies.
