- **`failures`, a forge capability**: what actually failed under a red gate —
  each failure as a job, a link to its log, and the error text where the forge
  can extract one. Asked on demand rather than during a refresh, since it is
  the expensive question and nobody asks it of a green gate. `ephor failures`
  is the command behind the menu entry; it collapses jobs that failed
  identically into one entry that says how many, because a gate fans one
  compile error across every job that built the file.
