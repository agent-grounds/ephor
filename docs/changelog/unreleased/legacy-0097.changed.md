- **A move that needs nobody runs beneath the screen**
  ([§FS-005-dispatch.17](../../functional-spec/FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen),
  [§AR-002-summons.5](../../architecture/AR-002-summons.md#5-detached-the-job)).
  Pressing `⤴ rebase onto master` handed the whole interface to a replay that
  asks nothing: minutes of output nobody has to read while it is produced, no
  watch underneath it, and a keypress at the end to get the screen back. The
  rebase entries now run as **jobs** — a process of their own, in a process
  group of their own, so quitting ephor or closing the terminal does not take
  one down — and the interface stays exactly where it was. A live job is a row
  on the operations board (`;`) saying what its log last said, `e` or `Enter`
  there reads the log with `less +F` following it, and when the job ends its
  one line lands on whatever screen is being read. Afterwards the record stays
  with the item: the work screen (`w`) lists what ephor ran there and `L`
  reads the newest one. From outside the interface, `ephor job list` and
  `ephor job log <id>` answer the same questions — a job outlives the ephor
  that started it, so it is answerable without one. Liveness is a lock, as it
  is for a run: a job that died holds none and wrote no outcome, and is
  reported as died rather than as running. The chain travels with the job — an
  entry needing the branch workspace runs its checkout as the job's first
  step, with the directory verified rather than trusted — and a conflict is
  still handed over as a ticket
  ([§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
  Configured actions and project offers keep the terminal unless they say
  `"background": true`: an entry may be `lazygit`, an editor, or a pager, and
  one of those started beneath the screen is a program nobody can type into.
