- **Tests split into an integration home, and e2e moves under `tests/`.**
  (PR #18) `.agents/grund.toml`'s deprecated `[[kinds]] prefix` key is renamed
  `kind` throughout — required before grund 0.13.0, which stops loading it —
  and the citable `E2E` kind gives way to two non-citable homes: `e2e` at
  `tests/e2e` (the corpus, moved from `e2e/`) and `integration` at
  `tests/integration` (the ten cross-part Rust tests and three Python
  repo-hygiene tests, moved out of `tests/`). `[citations.E2E]` becomes
  `[citations.e2e]` (must cite FS, should not cite AR) and
  `[citations.integration]` (should cite AR). CI's grund pin moves 0.9.0 →
  0.12.3 so the new config loads, and the entrypoints are re-rendered by that
  version. Cargo's `[[test]]` blocks and `exclude`, CI's and the pre-commit
  hook's Python discovery path, and the justfile's `e2e` recipe all follow the
  two moves.
