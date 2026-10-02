- **The parity gate inspects Cargo's configured build and reports setup
  failures without a traceback.** It resolves the effective target directory
  through Cargo, prefers its debug then release executable, and keeps the
  standalone locked-build fallback in that same directory. Resolution, build,
  missing-output, and launch failures now name the known paths and the recovery
  command as operational exit 2 errors
  ([§AR-009-surfaces.5.1](../../architecture/AR-009-surfaces.md#51-cargo-chooses-the-executable-the-parity-gate-inspects),
  [§REQ-002-parity.5](../../requirements/REQ-002-parity.md#5-the-parity-list-is-checked-not-remembered)).
  (PR #110)
