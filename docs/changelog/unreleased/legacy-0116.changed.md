- **The boundary is checked by the build, not observed by reviewers**
  ([§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter),
  [§AR-001-layers.2](../../architecture/AR-001-layers.md#2-where-literals-live)).
  `scripts/check_boundary.py` runs in `just check` and in CI, and it fails the
  build on two things: a product name — `gh`, `rhei`, `beads`, a chat vendor —
  spelled anywhere but the adapter that owns it, and a core module reaching
  the filesystem, a process, or a module above it. Documentation and test
  fixtures are exempt because the law exempts them, so the check reads Rust
  properly rather than grepping: comments out, strings kept, `#[cfg(test)]`
  bodies skipped. Making it pass moved real code — GitHub's reaction and reply
  writes into the GitHub adapter, the plan file's shape and the runner's
  words into the runtime module, ticket-key parsing out of the registry and
  into core — so nothing above a seam names a product any more. The four sites
  that remain are one pinned ledger entry each, and an entry that stops
  matching fails the check too: the list only shrinks.
