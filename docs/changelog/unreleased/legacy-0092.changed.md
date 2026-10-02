- **What is already going is shown where it could be started again**
  ([§FS-005-dispatch.21](../../functional-spec/FS-005-dispatch.md#21-what-is-already-going-is-shown-where-it-could-be-started-again),
  [§FS-011-command-line.8](../../functional-spec/FS-011-command-line.md#8-what-is-going-is-said-and-the-way-in-is-printed),
  PR #4). The menu said what could be done and the board said what was being
  done, and neither said the other: opening the menu on an item whose rebase was
  already replaying showed the rebase as something to start. Every entry with
  work going about its subject is now **marked running and set apart** — first,
  under a line that says so, a step further in, in one colour used for nothing
  else on that screen — with how long it has been going and what it is at right
  now: the job's own last line, the ticket a run holds and the state it is in,
  *waiting on you* where the ticket it opened is parked — with a run still at
  the gate or without one — and *queued* where the root's run will reach it.

  Found by looking, never remembered from the keypress: a job is a held lock
  and a record naming the entry it came from (a job now records that, and the
  branch on a branch row), a run is a held lock and the descriptor beside it,
  and the row that would make a branch workspace is running while the job whose
  first step is making it holds its own. A second ephor sees the same rows, and
  a job that died is not running whatever started it — and the whole reading is
  taken once for a menu rather than once for each of its rows.

  **Pressing a running entry opens it.** `Enter` (or `l`) goes to the thing
  that is running — a job's log followed as it writes, a run attached, a window
  brought forward — and the footer says *open* rather than *run*.
  `ephor actions [--json]` carries the same mark with the same facts and prints
  **the way in**, and `ephor actions open <id>` is that key as a command,
  refusing by name where the entry has nothing going. A parked question opens
  where the answer belongs: the run still standing at its gate, and the plan the
  question is written in where none is.
