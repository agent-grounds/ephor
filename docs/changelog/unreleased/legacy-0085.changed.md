- **`src/work/mod.rs` takes the first seam its size budget names, and the
  boundary check learns that shape**
  ([§FS-012-file-size.1](../../functional-spec/FS-012-file-size.md#1-the-budget-follows-the-reader),
  [§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).
  The dispatch module's inline `#[cfg(test)] mod tests` moves whole to
  `src/work/mod_tests.rs`, attached by `#[cfg(test)] #[path =
  "mod_tests.rs"] mod tests;` — a pure move, no test edited, the same 891
  tests — which brings the file back inside the ceiling
  `docs/file-size-human-exceptions.toml` records for it. Because the
  `#[cfg(test)]` now sits on the attachment rather than in the file,
  `scripts/check_boundary.py` reads a `<name>_tests.rs` sibling as a test
  body in its entirety, so fixtures that name a product stay the examples
  the law already permits. (PR #28)
