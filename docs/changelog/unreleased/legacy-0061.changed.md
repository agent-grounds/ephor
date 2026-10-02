- **CI's grund pin moves 0.12.3 → 0.13.0, and the entrypoints are re-rendered
  by that version.** `CLAUDE.md` and `.claude/CLAUDE.md` carry the v8 init
  block in place of v7. Under 0.13.0, `grund fmt --check` also caught a stray
  bare `§FS-013-burn.8` citation that 0.12.3 let through uncaught; it now reads
  as a Markdown link like its neighbors. (PR #76)
