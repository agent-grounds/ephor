- **Every operation is visible in one place, and parked work resurfaces on
  its own** ([§FS-005-dispatch.15](../../functional-spec/FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
  [§FS-005-dispatch.9](../../functional-spec/FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)).
  Answering "what is ephor doing right now" meant visiting the work screen of
  every item that might have an answer. `;` now opens a watch-only operations
  board from any screen: rows are execution roots — the runtime locks one per
  root, so two items in one branch workspace are one operation — with
  liveness read from that lock by a non-blocking probe, never from a process
  table, the held ticket from the run's own journal, the rest queued, a
  `quiet` badge on a run that has written nothing for a while (silence is not
  death: the OS releases the lock when a run dies), and a ticket claimed with
  no run behind it shown as *claimed, not scheduled* beside the runner's own
  release command. Work parked for a person keeps its row after the run
  exits — the usual end of a parked ticket's run, since nothing else was
  schedulable and parking writes no claim — and a run that died mid-slot
  leaves its held ticket *waiting on you* rather than vanishing with the
  lock; within one operation what waits on the reader lists ahead of what
  runs, then claims, then the queue
  ([§FS-005-dispatch.9](../../functional-spec/FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)).
  The journal outlives every run and is believed accordingly: an assignment
  no run released is held per invocation — fanout cannot mark a task free
  while a sibling still runs it — and stops counting the moment the ticket's
  own state moved on, so a crashed run's ticket never reads running under a
  later run. A root whose `states.yaml` cannot be read says less instead of
  guessing: running and claimed still show, nothing is called queued or
  counted finished on a machine that is not there. `Enter` goes to the
  matter, `o` opens a live run's
  dashboard, `Esc` returns exactly where the reader was; the background
  refresh reports on the board additionally to the header line it keeps
  ([§FS-001-forge-interface.7](../../functional-spec/FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)).
  Plan state and assignee read straight off the plan files as always — the
  floor that stays with no runner bound, when the board is the refresh row
  alone — and the runner's own `list --json` sharpens them where the binary
  is there: it also surfaces a subtask a live run holds, which the plan read
  alone cannot see, and it is asked only about roots that hold an operation
  — an idle root costs stats, never a fork. The tick's change gate is the
  journal and the lock, one stat each — never a sweep of the agent logs,
  which grow for the life of a project and are read only to clock the
  `quiet` badge on a live row. The side effect reaches past the board: work state used to be
  re-read only on refresh landings and dispatch, and now an mtime-gated tick
  between key reads re-reads what actually moved — so a ticket the runtime
  parks for you resurfaces the moment it parks
  ([§FS-005-dispatch.15.1](../../functional-spec/FS-005-dispatch.md#151-the-board-keeps-itself-current)),
  which is [§FS-005-dispatch.9](../../functional-spec/FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking) finally holding without a refresh.
