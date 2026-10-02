- **A finished job says so under the branch it ran on, not at the top of the
  screen**
  ([§FS-005-dispatch.17](../../functional-spec/FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)).
  A replay started beneath the screen used to announce itself in the
  header — `⤴ rebase onto main (level as of Aug 23): ok` above `ephor stream`
  — which names no project and no branch, so a reader with three of them going
  had to guess which row had just moved, and the line was gone at the next
  keypress. The line now lands **under the subject the job ran on**: the branch
  row it replayed, or the matter it was started about, beside the distance it
  just changed. Where that row is not on the screen at all — a branch with no
  item filed under it, the projects summary — the project's own row carries it,
  because news with nowhere to land is news that is lost. It stays there until
  the row is opened (`enter`, `o`, `x`, `w`, `c`) or a later job about the same
  subject replaces it. Only what has **ended** lands there: a job still going
  is already marked running where it could be started again
  ([§FS-005-dispatch.21](../../functional-spec/FS-005-dispatch.md#21-what-is-already-going-is-shown-where-it-could-be-started-again))
  and holds a row among the operations
  ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).
